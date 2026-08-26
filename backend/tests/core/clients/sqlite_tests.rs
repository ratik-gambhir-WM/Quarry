use crate::core::sqlbuilder::{Condition, ConflictUpdate, SortDirection, SqlBuilder, SqlValue};

use super::*;

fn test_client() -> SqliteClient {
    let client = SqliteClient::open_in_memory().unwrap();
    client
        .with_connection(|connection| {
            connection.execute_batch(
                r#"
                CREATE TABLE users (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    role TEXT,
                    active INTEGER NOT NULL
                );

                CREATE TABLE profiles (
                    user_id INTEGER PRIMARY KEY,
                    bio TEXT NOT NULL,
                    FOREIGN KEY (user_id) REFERENCES users(id)
                );
                "#,
            )
        })
        .unwrap();
    client
}

#[test]
fn writes_and_reads_dynamic_rows() {
    let client = test_client();
    let insert = SqlBuilder::insert_into("users")
        .value("name", "Ada")
        .value("role", "admin")
        .value("active", true)
        .build()
        .unwrap();

    let result = client.write(&insert).unwrap();
    assert_eq!(result.rows_affected, 1);
    assert_eq!(result.last_insert_row_id, 1);

    let select = SqlBuilder::select("users")
        .columns(["id", "name", "role", "active"])
        .and_where(Condition::equal("active", true))
        .order_by("id", SortDirection::Ascending)
        .build()
        .unwrap();
    let rows = client.read(&select).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("id"), Some(&SqlValue::Integer(1)));
    assert_eq!(
        rows[0].get("name"),
        Some(&SqlValue::Text("Ada".to_string()))
    );
}

#[test]
fn reads_rows_into_typed_values() {
    let client = test_client();
    let insert = SqlBuilder::insert_into("users")
        .value("name", "Grace")
        .value("role", Option::<String>::None)
        .value("active", true)
        .build()
        .unwrap();
    client.write(&insert).unwrap();

    let select = SqlBuilder::select("users")
        .columns(["id", "name"])
        .and_where(Condition::equal("name", "Grace"))
        .build()
        .unwrap();
    let user = client
        .read_one_with(&select, |row| {
            Ok((row.get::<_, i64>("id")?, row.get::<_, String>("name")?))
        })
        .unwrap();

    assert_eq!(user, Some((1, "Grace".to_string())));
}

#[tokio::test]
async fn async_methods_run_crud_queries() {
    let client = test_client();
    let insert = SqlBuilder::insert_into("users")
        .value("name", "Lin")
        .value("role", "analyst")
        .value("active", true)
        .build()
        .unwrap();
    client.write_async(insert).await.unwrap();

    let select = SqlBuilder::select("users")
        .and_where(Condition::equal("name", "Lin"))
        .build()
        .unwrap();
    let row = client.read_one_async(select).await.unwrap().unwrap();

    assert_eq!(
        row.get("role"),
        Some(&SqlValue::Text("analyst".to_string()))
    );
}

#[test]
fn rejects_using_queries_for_the_wrong_operation() {
    let client = test_client();
    let select = SqlBuilder::select("users").build().unwrap();
    assert!(matches!(
        client.write(&select),
        Err(SqliteClientError::InvalidOperation {
            operation: "write",
            ..
        })
    ));

    let insert = SqlBuilder::insert_into("users")
        .value("name", "Ada")
        .value("role", "admin")
        .value("active", true)
        .build()
        .unwrap();
    assert!(matches!(
        client.read(&insert),
        Err(SqliteClientError::InvalidOperation {
            operation: "read",
            ..
        })
    ));
}

#[test]
fn executes_join_queries() {
    let client = test_client();
    let insert_user = SqlBuilder::insert_into("users")
        .value("name", "Ada")
        .value("role", "admin")
        .value("active", true)
        .build()
        .unwrap();
    let user_id = client.write(&insert_user).unwrap().last_insert_row_id;
    let insert_profile = SqlBuilder::insert_into("profiles")
        .value("user_id", user_id)
        .value("bio", "Mathematician")
        .build()
        .unwrap();
    client.write(&insert_profile).unwrap();

    let select = SqlBuilder::select("users")
        .columns(["users.name", "profiles.bio"])
        .left_join("profiles", "users.id", "profiles.user_id")
        .and_where(Condition::equal("users.id", user_id))
        .build()
        .unwrap();
    let row = client.read_one(&select).unwrap().unwrap();

    assert_eq!(row.get("name"), Some(&SqlValue::Text("Ada".to_string())));
    assert_eq!(
        row.get("bio"),
        Some(&SqlValue::Text("Mathematician".to_string()))
    );
}

#[test]
fn executes_insert_on_conflict_updates_with_bound_parameters() {
    let client = test_client();
    let insert_user = SqlBuilder::insert_into("users")
        .value("name", "Ada")
        .value("role", "admin")
        .value("active", true)
        .build()
        .unwrap();
    let user_id = client.write(&insert_user).unwrap().last_insert_row_id;

    for bio in ["Mathematician", "Programmer"] {
        let upsert_profile = SqlBuilder::insert_into("profiles")
            .value("user_id", user_id)
            .value("bio", bio)
            .on_conflict_update(ConflictUpdate::new(["user_id"]).set_excluded("bio"))
            .build()
            .unwrap();
        client.write(&upsert_profile).unwrap();
    }

    let select = SqlBuilder::select("profiles")
        .column("bio")
        .and_where(Condition::equal("user_id", user_id))
        .build()
        .unwrap();
    let profile = client.read_one(&select).unwrap().unwrap();
    assert_eq!(
        profile.get("bio"),
        Some(&SqlValue::Text("Programmer".to_string()))
    );
}

#[test]
fn transaction_commits_multiple_writes_together() {
    let client = test_client();
    let insert_user = SqlBuilder::insert_into("users")
        .value("name", "Katherine")
        .value("role", "analyst")
        .value("active", true)
        .build()
        .unwrap();

    let user_id = client
        .transaction(|transaction| {
            let user_id = transaction.write(&insert_user)?.last_insert_row_id;
            let insert_profile = SqlBuilder::insert_into("profiles")
                .value("user_id", user_id)
                .value("bio", "Orbital mechanics")
                .build()
                .map_err(|error| SqliteClientError::transaction_aborted(error.to_string()))?;
            transaction.write(&insert_profile)?;
            Ok(user_id)
        })
        .unwrap();

    let profile = SqlBuilder::select("profiles")
        .column("bio")
        .and_where(Condition::equal("user_id", user_id))
        .build()
        .unwrap();
    assert_eq!(
        client.read_one(&profile).unwrap().unwrap().get("bio"),
        Some(&SqlValue::Text("Orbital mechanics".to_string()))
    );
}

#[test]
fn transaction_rolls_back_prior_writes_when_a_later_write_fails() {
    let client = test_client();
    let insert_user = SqlBuilder::insert_into("users")
        .value("name", "Dorothy")
        .value("role", "admin")
        .value("active", true)
        .build()
        .unwrap();

    let error = client
        .transaction(|transaction| {
            transaction.write(&insert_user)?;
            let invalid_profile = SqlBuilder::insert_into("profiles")
                .value("user_id", 999_i64)
                .value("bio", "must fail")
                .build()
                .map_err(|error| SqliteClientError::transaction_aborted(error.to_string()))?;
            transaction.write(&invalid_profile)?;
            Ok(())
        })
        .unwrap_err();
    assert!(error.to_string().contains("FOREIGN KEY"));

    let users = SqlBuilder::select("users").build().unwrap();
    assert!(client.read(&users).unwrap().is_empty());
}

#[test]
fn transaction_rolls_back_when_commit_fails() {
    let client = SqliteClient::open_in_memory().unwrap();
    client
        .with_connection(|connection| {
            connection.execute_batch(
                r#"
                CREATE TABLE parents (id INTEGER PRIMARY KEY);
                CREATE TABLE children (
                    id INTEGER PRIMARY KEY,
                    parent_id INTEGER NOT NULL,
                    FOREIGN KEY (parent_id) REFERENCES parents(id)
                        DEFERRABLE INITIALLY DEFERRED
                );
                "#,
            )
        })
        .unwrap();
    let invalid_child = SqlBuilder::insert_into("children")
        .value("id", 1_i64)
        .value("parent_id", 999_i64)
        .build()
        .unwrap();

    let error = client
        .transaction(|transaction| {
            transaction.write(&invalid_child)?;
            Ok(())
        })
        .unwrap_err();

    assert!(error.to_string().contains("failed to commit"));
    let children = SqlBuilder::select("children").build().unwrap();
    assert!(client.read(&children).unwrap().is_empty());
}

#[test]
fn transaction_reads_then_builds_a_dependent_write_on_the_same_connection() {
    let client = test_client();
    let first = SqlBuilder::insert_into("users")
        .value("name", "First")
        .value("role", "analyst")
        .value("active", true)
        .build()
        .unwrap();
    client.write(&first).unwrap();

    client
        .transaction(|transaction| {
            let latest = SqlBuilder::select("users")
                .column("id")
                .order_by("id", SortDirection::Descending)
                .limit(1)
                .build()
                .map_err(|error| SqliteClientError::transaction_aborted(error.to_string()))?;
            let latest_id = match transaction.read_one(&latest)?.unwrap().get("id") {
                Some(SqlValue::Integer(value)) => *value,
                value => panic!("unexpected id value: {value:?}"),
            };
            let dependent = SqlBuilder::insert_into("profiles")
                .value("user_id", latest_id)
                .value("bio", format!("profile-{latest_id}"))
                .build()
                .map_err(|error| SqliteClientError::transaction_aborted(error.to_string()))?;
            transaction.write(&dependent)?;
            Ok(())
        })
        .unwrap();

    let profiles = SqlBuilder::select("profiles").build().unwrap();
    assert_eq!(client.read(&profiles).unwrap().len(), 1);
}

#[test]
fn transaction_preserves_query_kind_validation() {
    let client = test_client();
    let select = SqlBuilder::select("users").build().unwrap();
    let insert = SqlBuilder::insert_into("users")
        .value("name", "Wrong operation")
        .value("role", "analyst")
        .value("active", true)
        .build()
        .unwrap();

    client
        .transaction(|transaction| {
            assert!(matches!(
                transaction.write(&select),
                Err(SqliteClientError::InvalidOperation {
                    operation: "write",
                    ..
                })
            ));
            assert!(matches!(
                transaction.read_one(&insert),
                Err(SqliteClientError::InvalidOperation {
                    operation: "read",
                    ..
                })
            ));
            Ok(())
        })
        .unwrap();
}

#[tokio::test]
async fn immediate_transactions_serialize_dependent_version_writers() {
    let client = SqliteClient::open_in_memory().unwrap();
    client
        .with_connection(|connection| {
            connection.execute_batch(
                "CREATE TABLE versions (version_number INTEGER PRIMARY KEY NOT NULL);",
            )
        })
        .unwrap();

    let mut tasks = Vec::new();
    for _ in 0..2 {
        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            client
                .transaction_async(|transaction| {
                    let latest = SqlBuilder::select("versions")
                        .column("version_number")
                        .order_by("version_number", SortDirection::Descending)
                        .limit(1)
                        .build()
                        .map_err(|error| {
                            SqliteClientError::transaction_aborted(error.to_string())
                        })?;
                    let version_number = transaction
                        .read_one(&latest)?
                        .map(|row| match row.get("version_number") {
                            Some(SqlValue::Integer(value)) => *value + 1,
                            value => panic!("unexpected version value: {value:?}"),
                        })
                        .unwrap_or(1);
                    let insert = SqlBuilder::insert_into("versions")
                        .value("version_number", version_number)
                        .build()
                        .map_err(|error| {
                            SqliteClientError::transaction_aborted(error.to_string())
                        })?;
                    transaction.write(&insert)?;
                    Ok(version_number)
                })
                .await
        }));
    }

    let mut versions = Vec::new();
    for task in tasks {
        versions.push(task.await.unwrap().unwrap());
    }
    versions.sort_unstable();
    assert_eq!(versions, vec![1, 2]);
}
