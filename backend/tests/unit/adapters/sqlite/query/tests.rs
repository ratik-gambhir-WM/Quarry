use super::*;
use crate::adapters::sqlite::client::SqliteClientError;

fn build_error(result: Result<SqlQuery, SqliteClientError>) -> SqlBuilderError {
    match result.unwrap_err() {
        SqliteClientError::QueryBuild(error) => error,
        error => panic!("expected query construction error, got {error}"),
    }
}

#[test]
fn all_builders_preserve_construction_error_sources() {
    use std::error::Error;

    let cases = [
        (
            SqlBuilder::select("").build(),
            SqlBuilderError::EmptyIdentifier,
        ),
        (
            SqlBuilder::insert_into("users").build(),
            SqlBuilderError::MissingInsertValues,
        ),
        (
            SqlBuilder::update("users").build(),
            SqlBuilderError::MissingUpdateAssignments,
        ),
        (
            SqlBuilder::delete_from("users").build(),
            SqlBuilderError::MissingCondition("DELETE"),
        ),
    ];

    for (result, expected) in cases {
        let error = result.unwrap_err();
        assert_eq!(
            error.source().unwrap().downcast_ref::<SqlBuilderError>(),
            Some(&expected)
        );
        assert!(matches!(error, SqliteClientError::QueryBuild(_)));
    }
}

#[test]
fn builds_parameterized_selects() {
    let query = SqlBuilder::select("users")
        .columns(["id", "email"])
        .and_where(Condition::equal("status", "active"))
        .and_where(Condition::in_values("role", ["admin", "analyst"]))
        .order_by("updated_at", SortDirection::Descending)
        .limit(25)
        .offset(50)
        .build()
        .unwrap();

    assert_eq!(
        query.sql(),
        "SELECT \"id\", \"email\" FROM \"users\" WHERE (\"status\" = ?) AND (\"role\" IN (?, ?)) ORDER BY \"updated_at\" DESC LIMIT 25 OFFSET 50"
    );
    assert_eq!(
        query.parameters(),
        &[
            SqlValue::Text("active".to_string()),
            SqlValue::Text("admin".to_string()),
            SqlValue::Text("analyst".to_string()),
        ]
    );
}

#[test]
fn keeps_untrusted_values_out_of_sql() {
    let untrusted = "' OR 1 = 1 --";
    let query = SqlBuilder::select("users")
        .and_where(Condition::equal("email", untrusted))
        .build()
        .unwrap();

    assert!(!query.sql().contains(untrusted));
    assert_eq!(query.parameters(), &[SqlValue::Text(untrusted.to_string())]);
}

#[test]
fn builds_insert_update_and_delete_queries() {
    let insert = SqlBuilder::insert_into("users")
        .value("email", "ada@example.com")
        .value("active", true)
        .build()
        .unwrap();
    assert_eq!(
        insert.sql(),
        "INSERT INTO \"users\" (\"email\", \"active\") VALUES (?, ?)"
    );

    let update = SqlBuilder::update("users")
        .set("active", false)
        .and_where(Condition::equal("id", 7_i64))
        .build()
        .unwrap();
    assert_eq!(
        update.sql(),
        "UPDATE \"users\" SET \"active\" = ? WHERE (\"id\" = ?)"
    );

    let delete = SqlBuilder::delete_from("users")
        .and_where(Condition::equal("id", 7_i64))
        .build()
        .unwrap();
    assert_eq!(delete.sql(), "DELETE FROM \"users\" WHERE (\"id\" = ?)");
}

#[test]
fn builds_parameterized_insert_on_conflict_update() {
    let query = SqlBuilder::insert_into("deal_metadata")
        .value("deal_id", 42_i64)
        .value("key_questions_json", "[]")
        .value("document_count", 3_i64)
        .value("data_room_size_bytes", 1024_i64)
        .on_conflict_update(
            ConflictUpdate::new(["deal_id"])
                .set_excluded("key_questions_json")
                .set_excluded("document_count")
                .set_excluded("data_room_size_bytes")
                .set_current_timestamp("updated_at"),
        )
        .build()
        .unwrap();

    assert_eq!(
        query.sql(),
        "INSERT INTO \"deal_metadata\" (\"deal_id\", \"key_questions_json\", \"document_count\", \"data_room_size_bytes\") VALUES (?, ?, ?, ?) ON CONFLICT (\"deal_id\") DO UPDATE SET \"key_questions_json\" = excluded.\"key_questions_json\", \"document_count\" = excluded.\"document_count\", \"data_room_size_bytes\" = excluded.\"data_room_size_bytes\", \"updated_at\" = CURRENT_TIMESTAMP"
    );
    assert_eq!(
        query.parameters(),
        &[
            SqlValue::Integer(42),
            SqlValue::Text("[]".to_string()),
            SqlValue::Integer(3),
            SqlValue::Integer(1024),
        ]
    );
}

#[test]
fn supports_bound_conflict_updates_and_do_nothing() {
    let update = SqlBuilder::insert_into("users")
        .value("email", "ada@example.com")
        .on_conflict_update(ConflictUpdate::new(["email"]).set("status", "already_registered"))
        .build()
        .unwrap();
    assert_eq!(
        update.sql(),
        "INSERT INTO \"users\" (\"email\") VALUES (?) ON CONFLICT (\"email\") DO UPDATE SET \"status\" = ?"
    );
    assert_eq!(
        update.parameters(),
        &[
            SqlValue::Text("ada@example.com".to_string()),
            SqlValue::Text("already_registered".to_string()),
        ]
    );

    let do_nothing = SqlBuilder::insert_into("users")
        .value("email", "ada@example.com")
        .on_conflict_do_nothing(["email"])
        .build()
        .unwrap();
    assert_eq!(
        do_nothing.sql(),
        "INSERT INTO \"users\" (\"email\") VALUES (?) ON CONFLICT (\"email\") DO NOTHING"
    );
}

#[test]
fn validates_conflict_updates() {
    assert_eq!(
        build_error(
            SqlBuilder::insert_into("users")
                .value("email", "ada@example.com")
                .on_conflict_update(ConflictUpdate::new(Vec::<String>::new()).set_excluded("email"))
                .build()
        ),
        SqlBuilderError::MissingConflictTarget
    );
    assert_eq!(
        build_error(
            SqlBuilder::insert_into("users")
                .value("email", "ada@example.com")
                .on_conflict_update(ConflictUpdate::new(["email"]))
                .build()
        ),
        SqlBuilderError::MissingConflictAssignments
    );
}

#[test]
fn update_and_delete_require_explicit_scope() {
    assert_eq!(
        build_error(SqlBuilder::update("users").set("active", false).build()),
        SqlBuilderError::MissingCondition("UPDATE")
    );
    assert_eq!(
        build_error(SqlBuilder::delete_from("users").build()),
        SqlBuilderError::MissingCondition("DELETE")
    );

    assert!(SqlBuilder::delete_from("users")
        .allow_all_rows()
        .build()
        .is_ok());
}

#[test]
fn equal_null_uses_sql_null_semantics() {
    let query = SqlBuilder::select("users")
        .and_where(Condition::equal("deleted_at", SqlValue::Null))
        .build()
        .unwrap();

    assert_eq!(
        query.sql(),
        "SELECT * FROM \"users\" WHERE (\"deleted_at\" IS NULL)"
    );
    assert!(query.parameters().is_empty());
}

#[test]
fn builds_simple_joins_with_qualified_wildcards() {
    let query = SqlBuilder::select("users")
        .columns(["users.*", "profiles.bio"])
        .left_join("profiles", "users.id", "profiles.user_id")
        .inner_join("teams", "users.team_id", "teams.id")
        .build()
        .unwrap();

    assert_eq!(
        query.sql(),
        "SELECT \"users\".*, \"profiles\".\"bio\" FROM \"users\" LEFT JOIN \"profiles\" ON (\"users\".\"id\" = \"profiles\".\"user_id\") INNER JOIN \"teams\" ON (\"users\".\"team_id\" = \"teams\".\"id\")"
    );
    assert!(query.parameters().is_empty());
}

#[test]
fn builds_aliased_joins_with_composable_on_conditions() {
    let query = SqlBuilder::select("users")
        .alias("u")
        .columns(["u.id", "m.role"])
        .join(
            JoinClause::left("memberships")
                .alias("m")
                .on(JoinCondition::equal("u.id", "m.user_id"))
                .and_on(JoinCondition::value(
                    "m.active",
                    ComparisonOperator::Equal,
                    true,
                )),
        )
        .and_where(Condition::equal("u.status", "active"))
        .build()
        .unwrap();

    assert_eq!(
        query.sql(),
        "SELECT \"u\".\"id\", \"m\".\"role\" FROM \"users\" AS \"u\" LEFT JOIN \"memberships\" AS \"m\" ON (\"u\".\"id\" = \"m\".\"user_id\") AND (\"m\".\"active\" = ?) WHERE (\"u\".\"status\" = ?)"
    );
    assert_eq!(
        query.parameters(),
        &[SqlValue::Integer(1), SqlValue::Text("active".to_string()),]
    );
}

#[test]
fn validates_join_conditions() {
    assert_eq!(
        build_error(
            SqlBuilder::select("users")
                .join(JoinClause::inner("profiles"))
                .build()
        ),
        SqlBuilderError::MissingJoinCondition {
            join_type: "INNER",
            table: "profiles".to_string(),
        }
    );
    assert_eq!(
        build_error(
            SqlBuilder::select("users")
                .join(
                    JoinClause::cross("roles")
                        .on(JoinCondition::equal("users.role_id", "roles.id")),
                )
                .build()
        ),
        SqlBuilderError::UnexpectedCrossJoinCondition("roles".to_string())
    );

    let cross_join = SqlBuilder::select("users")
        .cross_join("roles")
        .build()
        .unwrap();
    assert_eq!(
        cross_join.sql(),
        "SELECT * FROM \"users\" CROSS JOIN \"roles\""
    );
}

#[test]
fn count_rows_and_fts_keep_bound_input() {
    let query = SqlBuilder::select("templates")
        .and_where(Condition::equal("template_id", "value' OR 1=1"))
        .count_rows()
        .build()
        .unwrap();
    assert!(query.sql().starts_with("SELECT COUNT(*)"));
    assert_eq!(query.parameters(), &[SqlValue::from("value' OR 1=1")]);
    let query = SqlBuilder::select("documents_fts")
        .and_where(Condition::full_text_match(
            "documents_fts",
            "cloud OR attack",
        ))
        .order_by_bm25("documents_fts", vec![0.0, 2.0])
        .order_by("id", SortDirection::Ascending)
        .limit(10)
        .build()
        .unwrap();
    assert!(!query.sql().contains("cloud OR attack"));
    assert!(query.sql().contains("bm25("));
    assert_eq!(query.parameters().len(), 3);
}

#[test]
fn fts_ranking_rejects_nonfinite_weights() {
    assert_eq!(
        build_error(
            SqlBuilder::select("documents_fts")
                .order_by_bm25("documents_fts", vec![f64::NAN])
                .build()
        ),
        SqlBuilderError::NonFiniteRankWeight
    );
}

#[test]
fn notebook_query_increment_and_literal_substring_remain_typed_and_bound() {
    let query = SqlBuilder::update("notes")
        .set("folder_id", None::<String>)
        .increment("revision")
        .and_where(Condition::equal("folder_id", "folder"))
        .build()
        .unwrap();
    assert!(query.sql().contains("\"revision\" = \"revision\" + 1"));
    assert_eq!(query.parameters().len(), 2);
    assert!(SqlBuilder::update("notes")
        .set("revision", 1_i64)
        .increment("revision")
        .and_where(Condition::equal("id", "id"))
        .build()
        .is_err());
    let query = SqlBuilder::select("notes")
        .and_where(Condition::literal_substring("title", "%_\\' OR 1=1"))
        .build()
        .unwrap();
    assert!(!query.sql().contains("OR 1=1"));
    assert!(query.sql().contains("LIKE ? ESCAPE"));
    assert_eq!(
        query.parameters(),
        &[SqlValue::Text("%\\%\\_\\\\' OR 1=1%".into())]
    );
}
