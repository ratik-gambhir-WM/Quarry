use super::*;
use crate::{adapters::postgres::query::SqlValue, app::migrations};
#[tokio::test]
async fn lexical_order_matches_captured_fts5_results_with_owner_and_app_filters() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../fixtures/template-lexical-baseline.json"
    ))
    .unwrap();
    let client = PostgresClient::test_database().await.unwrap();
    migrations::migrate(&client).await.unwrap();
    crate::integration_support::seed_identity(&client, "lexical-one@example.invalid", 'a').await;
    crate::integration_support::seed_identity(&client, "lexical-two@example.invalid", 'b').await;
    client
        .transaction_async(|tx| async move {
            for app in ["A", "B"] {
                tx.write(
                    SqlBuilder::insert_into("apps")
                        .value("app_id", app)
                        .value("display_name", app)
                        .value("metadata_json", "{}")
                        .value("created_at", "now")
                        .value("last_seen_at", "now")
                        .build(),
                )
                .await?;
            }
            for row in fixture["rows"].as_array().unwrap() {
                let id = row["id"].as_str().unwrap();
                tx.write(
                    SqlBuilder::insert_into("templates")
                        .value("template_id", id)
                        .value("template_json", "{}")
                        .value("owner_user_id", row["owner"].as_i64().unwrap())
                        .build(),
                )
                .await?;
                tx.write(
                    SqlBuilder::insert_into("app_templates")
                        .value("app_id", row["app"].as_str().unwrap())
                        .value("template_id", id)
                        .value("created_at", "now")
                        .build(),
                )
                .await?;
                tx.write(
                    SqlBuilder::insert_into("template_classification_status")
                        .value("template_id", id)
                        .value("status", "ready")
                        .value("metadata_json", "{}")
                        .value("input_fingerprint", "fixture")
                        .value("schema_version", 2)
                        .value("prompt_version", "fixture")
                        .build(),
                )
                .await?;
                let fields = row["fields"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect::<Vec<_>>();
                let mut insert =
                    SqlBuilder::insert_into("slide_classifications_fts").value("template_id", id);
                for (column, field) in fixture["columns"].as_array().unwrap().iter().zip(&fields) {
                    insert = insert.value(column.as_str().unwrap(), field);
                }
                tx.write(insert.build()).await?;
                index(&tx, id, &fields).await?;
            }
            for i in 0..fixture["emptyDocuments"].as_i64().unwrap() {
                let id = format!("empty-{i}");
                tx.write(
                    SqlBuilder::insert_into("templates")
                        .value("template_id", &id)
                        .value("template_json", "{}")
                        .value("owner_user_id", 1)
                        .build(),
                )
                .await?;
                let mut insert =
                    SqlBuilder::insert_into("slide_classifications_fts").value("template_id", &id);
                for column in fixture["columns"].as_array().unwrap() {
                    insert = insert.value(column.as_str().unwrap(), "");
                }
                tx.write(insert.build()).await?;
            }
            Ok(())
        })
        .await
        .unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../fixtures/template-lexical-baseline.json"
    ))
    .unwrap();
    for query in fixture["queries"].as_array().unwrap() {
        let result = search(
            &client,
            Some(query["owner"].as_i64().unwrap()),
            query["app"].as_str().unwrap(),
            query["expression"].as_str().unwrap(),
            200,
        )
        .await
        .unwrap();
        let expected = query["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(result, expected, "{query}");
    }
    // A selective prefix must keep using the term index as the unrelated corpus grows.
    sqlx::query("INSERT INTO template_lexical_terms(template_id,term,field_index,frequency) SELECT 'empty-0','noise' || n,0,1 FROM generate_series(1,10000) n")
        .execute(client.pool()).await.unwrap();
    sqlx::query("UPDATE slide_classifications_fts SET title=(SELECT string_agg('noise' || n,' ') FROM generate_series(1,10000) n),token_count=10000 WHERE template_id='empty-0'")
        .execute(client.pool()).await.unwrap();
    sqlx::query("ANALYZE template_lexical_terms")
        .execute(client.pool())
        .await
        .unwrap();
    let query = search_query(Some(1), "A", "\"cloud\"*", 200).unwrap();
    let explain = SqlQuery::new(
        format!("EXPLAIN (COSTS OFF) {}", query.sql()),
        query.parameters().to_vec(),
        QueryKind::Select,
    );
    let plan = client
        .read_with_async(Ok(explain), |r| r.get::<_, String>(0))
        .await
        .unwrap()
        .join("\n");
    assert!(plan.contains("template_lexical_terms_pkey"), "{plan}");
    eprintln!("Indexed lexical plan after 10,000 unrelated terms:\n{plan}");
    // Query text is bound, and syntax outside the established safe grammar is rejected.
    assert!(search_query(Some(1), "A", "cloud');DROP TABLE users;--", 10).is_err());
    assert!(search_query(Some(1), "A", "\"cloud\"*", 10)
        .unwrap()
        .parameters()
        .contains(&SqlValue::from("cloud")));
}

#[test]
fn tokenizer_matches_captured_unicode61_categories_folding_and_diacritics() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../fixtures/template-lexical-baseline.json"
    ))
    .unwrap();
    for case in fixture["tokenization"].as_array().unwrap() {
        let expected = case["tokens"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(tokens(case["input"].as_str().unwrap()), expected, "{case}");
    }
}
