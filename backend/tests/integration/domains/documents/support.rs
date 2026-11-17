use crate::{
    adapters::postgres::client::PostgresClient,
    domains::documents::{
        model::{Document, DocumentChunk},
        store::{
            repository::DocumentStore,
            search_repository::{DocumentSearchRepository, EmbeddingProfile},
        },
    },
    shared::ids::{document_id_from_content, sha256_hex},
};
pub(crate) async fn database() -> PostgresClient {
    seed_database(PostgresClient::test_database().await.unwrap()).await
}
pub(crate) async fn database_at(path: &str) -> PostgresClient {
    seed_database(PostgresClient::test_database_at(path).await.unwrap()).await
}
async fn seed_database(db: PostgresClient) -> PostgresClient {
    crate::app::migrations::migrate(&db, Default::default())
        .await
        .unwrap();
    crate::integration_support::seed_identity(&db, "document-owner@example.invalid", 'a').await;
    crate::integration_support::seed_identity(&db, "document-other@example.invalid", 'b').await;
    for (deal, actor) in [("DEAL-SQL", 1i64), ("DEAL-FOREIGN", 2)] {
        sqlx::query("INSERT INTO deals(deal_id,user_id,deal_name,engagement_type,transaction_type,status,created_at,updated_at) VALUES($1,$2,'Synthetic','buy_side','unknown','pipeline','now','now')").bind(deal).bind(actor).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at) VALUES($1,$2,'owner','now','now')").bind(deal).bind(actor).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO deal_access_revisions(deal_id) VALUES($1)")
            .bind(deal)
            .execute(db.pool())
            .await
            .unwrap();
    }
    db
}
pub(crate) fn sample(
    file: &str,
    deal: &str,
    content: &str,
    count: u32,
) -> (Document, Vec<DocumentChunk>, Vec<u8>) {
    let bytes = content.as_bytes().to_vec();
    let hash = sha256_hex(&bytes);
    let document_id = document_id_from_content(deal, &hash);
    let document = Document {
        file_id: file.into(),
        document_id: document_id.clone(),
        deal_id: deal.into(),
        file_name: "evidence.docx".into(),
        source_type: "docx".into(),
        local_path: None,
        file_size_bytes: u64::try_from(bytes.len()).unwrap(),
        token_count: u64::from(count) * 4,
        content_hash: hash,
        rendered_pdf_path: None,
    };
    let chunks = (1..=count)
        .map(|index| {
            let text = format!("Architecture technical evidence {content} chunk {index}");
            let start = usize::try_from(index).unwrap() * 100;
            DocumentChunk {
                chunk_id: format!("random-parser-{index}"),
                document_id: document_id.clone(),
                deal_id: deal.into(),
                content_hash: sha256_hex(text.as_bytes()),
                text,
                embedding: Some(vec![0.5; 1536]),
                sequence_number: index,
                page_numbers: Some(vec![index]),
                start_offset: start,
                end_offset: start + 50,
                token_count: 4,
                section_title: Some("Architecture".into()),
            }
        })
        .collect();
    (document, chunks, bytes)
}
pub(crate) fn search(db: PostgresClient) -> DocumentSearchRepository {
    DocumentSearchRepository::new(
        db,
        EmbeddingProfile::new("text-embedding-3-small".into(), 1536),
    )
}
pub(crate) async fn save(db: &PostgresClient, file: &str, deal: &str, content: &str, count: u32) {
    let (doc, chunks, bytes) = sample(file, deal, content, count);
    crate::domains::documents::ingestion::persistence::persist_document_and_chunks(
        &DocumentStore::new(db.clone()),
        deal,
        if deal == "DEAL-SQL" { 1 } else { 2 },
        doc,
        chunks,
        bytes,
        None,
    )
    .await
    .unwrap();
}
