use std::{io::Cursor, num::NonZeroUsize, sync::Arc};

use docx_rust::{document::Paragraph, Docx};
use helix_db::dsl::prelude::*;
use serde_json::Value;

use crate::{
    adapters::{helix::client::HelixClient, sqlite::client::SqliteClient},
    app::migrations,
    domains::{
        deals::{
            model::DealFields,
            repository::{CreateDealRecord, DealRepository},
        },
        documents::{
            formats::docx::parse_docx_chunks_from_bytes,
            graph::{
                query::{FileChunkKeywordSearch, FileChunkVectorSearch},
                repository::{DocumentGraphReader, DocumentGraphSearch, DocumentGraphWriter},
            },
            ingestion::persistence::persist_document_and_chunks,
            search::service::DocumentSearchService,
            store::sqlite::DocumentStore,
        },
        identity::access::ResourceAccess,
        memberships::{
            model::{DealRole, Mutation},
            repository::MembershipRepository,
            service::DealAccessService,
        },
    },
};

#[tokio::test]
#[ignore = "requires explicitly selected disposable QUARRY_HELIX_COMPAT_URL"]
async fn v3_shared_search_authorizes_current_candidates_and_exact_byte_retries() {
    let url = std::env::var("QUARRY_HELIX_COMPAT_URL").expect("disposable instance URL");
    let helix = Arc::new(HelixClient::with_config(&url, None).unwrap());
    let writer = DocumentGraphWriter::new(helix.clone());
    writer
        .initialize(NonZeroUsize::new(2).unwrap())
        .await
        .unwrap();
    let sqlite = SqliteClient::open_in_memory().unwrap();
    migrations::migrate(&sqlite).unwrap();
    sqlite.with_connection(|connection| {
        connection.execute("INSERT INTO users(id,username,email,first_name,last_name,password_hash) VALUES(1,'owner','owner@example.test','Test','Owner','unusable'),(2,'collaborator','collaborator@example.test','Test','Collaborator','unusable')",[])?;
        Ok(())
    }).unwrap();
    let deals = DealRepository::new(sqlite.clone());
    let store = DocumentStore::new(sqlite.clone());
    let mut source = Docx::default();
    source
        .document
        .push(Paragraph::default().push_text("Synthetic shared architecture evidence."));
    let bytes = source.write(Cursor::new(Vec::new())).unwrap().into_inner();
    for (deal, file) in [
        ("DEAL-SHARED-SEARCH", "shared-search-file"),
        ("DEAL-OTHER-SEARCH", "other-search-file"),
        ("DEAL-PARTIAL-SEARCH", "partial-search-file"),
    ] {
        let fields: DealFields = serde_json::from_value(serde_json::json!({
            "dealName":"Shared search", "dealCode":null, "startDate":null,
            "expectedCloseDate":null, "actualCloseDate":null,"engagementCompletedDate":null,
            "engagementType":"buy_side","transactionType":"unknown","status":"pipeline",
            "parties":[],"scopes":[]
        }))
        .unwrap();
        deals
            .create(CreateDealRecord {
                deal_id: deal.into(),
                user_id: 1,
                fields,
                local_path: None,
                sharepoint_link: None,
            })
            .await
            .unwrap();
        index(&store, &writer, deal, file, 1, &bytes).await;
    }
    let members = MembershipRepository::new(sqlite.clone());
    members
        .mutate(
            "DEAL-SHARED-SEARCH".into(),
            1,
            Mutation::AddMember {
                username: "collaborator".into(),
                role: DealRole::Contributor,
            },
        )
        .await
        .unwrap();
    let access = ResourceAccess::new(
        sqlite.clone(),
        Arc::new(DealAccessService::new(members.clone())),
    );
    let search =
        DocumentSearchService::new(DocumentGraphSearch::new(helix.clone())).with_access(access);
    let keyword = |deal: &str| FileChunkKeywordSearch {
        deal_id: deal.into(),
        query_text: "architecture".into(),
        limit: 10,
    };
    let scoped = search
        .keyword_owned(keyword("DEAL-SHARED-SEARCH"), 2)
        .await
        .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].chunk.file_id, "shared-search-file");
    let global = search.keyword_owned(keyword(""), 2).await.unwrap();
    assert_eq!(global.len(), 1);
    assert_eq!(global[0].chunk.chunk_id, scoped[0].chunk.chunk_id);
    assert_eq!(search.keyword_owned(keyword(""), 1).await.unwrap().len(), 3);
    assert!(search
        .keyword_owned(keyword("DEAL-OTHER-SEARCH"), 2)
        .await
        .is_err());
    let vectors = search
        .vector_owned(
            FileChunkVectorSearch {
                deal_id: "DEAL-SHARED-SEARCH".into(),
                query_embedding: vec![0.25, 0.75],
                limit: 10,
            },
            2,
        )
        .await
        .unwrap();
    assert_eq!(vectors.len(), 1);
    assert_eq!(vectors[0].chunk.chunk_id, scoped[0].chunk.chunk_id);

    // A current canonical version with missing graph chunks must fail explicitly, then recover by retry.
    let _: Value = helix
        .execute_write_query(
            "test.partial_search_graph",
            remove_partial_chunks().unwrap(),
        )
        .await
        .unwrap();
    assert!(search
        .keyword_owned(keyword("DEAL-PARTIAL-SEARCH"), 1)
        .await
        .is_err());
    assert!(search.keyword_owned(keyword(""), 1).await.is_err());
    index(
        &store,
        &writer,
        "DEAL-PARTIAL-SEARCH",
        "partial-search-file",
        1,
        &bytes,
    )
    .await;
    assert_eq!(search.keyword_owned(keyword(""), 1).await.unwrap().len(), 3);
    let before = store
        .manifest(scoped[0].chunk.version_id.clone())
        .await
        .unwrap()
        .unwrap();
    index(
        &store,
        &writer,
        "DEAL-SHARED-SEARCH",
        "shared-search-file",
        2,
        &bytes,
    )
    .await;
    let after = store
        .manifest(scoped[0].chunk.version_id.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before, after);
    let summary = store.list_for_deal("DEAL-SHARED-SEARCH").await.unwrap();
    assert_eq!(summary.len(), 1);
    assert_eq!(summary[0].uploaded_by_user_id, Some(1));
    let chunks = DocumentGraphReader::new(helix)
        .document_version_chunks(
            "DEAL-SHARED-SEARCH",
            "shared-search-file",
            &scoped[0].chunk.version_id,
        )
        .await
        .unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].chunk_id, scoped[0].chunk.chunk_id);
    members
        .mutate(
            "DEAL-SHARED-SEARCH".into(),
            1,
            Mutation::Transfer {
                user_id: 2,
                revision: 2,
            },
        )
        .await
        .unwrap();
    members
        .mutate(
            "DEAL-SHARED-SEARCH".into(),
            2,
            Mutation::RemoveMember { user_id: 1 },
        )
        .await
        .unwrap();
    assert!(search
        .keyword_owned(keyword("DEAL-SHARED-SEARCH"), 1)
        .await
        .is_err());
    let remaining = search.keyword_owned(keyword(""), 1).await.unwrap();
    assert_eq!(remaining.len(), 2);
    assert!(remaining
        .iter()
        .all(|hit| hit.chunk.deal_id != "DEAL-SHARED-SEARCH"));
}
async fn index(
    store: &DocumentStore,
    writer: &DocumentGraphWriter,
    deal: &str,
    file: &str,
    actor: i64,
    bytes: &[u8],
) {
    let mut assembly = parse_docx_chunks_from_bytes(bytes.to_vec(), None, deal).unwrap();
    assembly.document.file_id = file.into();
    assembly.document.file_name = "shared.docx".into();
    for chunk in &mut assembly.chunks {
        chunk.embedding = Some(vec![0.25, 0.75]);
    }
    persist_document_and_chunks(
        store,
        writer,
        deal,
        actor,
        assembly.document,
        assembly.chunks,
        bytes.to_vec(),
    )
    .await
    .unwrap();
}
#[query]
fn remove_partial_chunks() -> WriteBatch {
    write_batch()
        .var_as(
            "removed",
            g().n_with_label("FileChunk")
                .has("file_id", "partial-search-file")
                .drop()
                .count(),
        )
        .returning(["removed"])
}
