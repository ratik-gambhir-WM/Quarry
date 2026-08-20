use helix_db::dsl::prelude::*;

use crate::core::models::user::UserNode;

pub const USER_LABEL: &str = "User";

/// Builds an email-keyed upsert for a Helix `User` node.
pub fn save_user(user: UserNode) -> Result<DynamicQueryRequest, String> {
    let UserNode {
        id,
        first_name,
        last_name,
        email,
        api_key,
        role,
        created_at,
        updated_at,
    } = user;

    if email.trim().is_empty() {
        return Err("user email cannot be empty".to_string());
    }

    Ok(add_user_mutation(
        id, first_name, last_name, email, api_key, role, created_at, updated_at,
    ))
}

/// Builds a Helix lookup for one `User` node by its exact email address.
pub fn get_user_by_email(email: String) -> Result<DynamicQueryRequest, String> {
    if email.trim().is_empty() {
        return Err("user email cannot be empty".to_string());
    }

    Ok(get_user_by_email_query(email))
}

#[register]
fn get_user_by_email_query(email: String) -> ReadBatch {
    let _ = &email;

    read_batch()
        .var_as(
            "user",
            g().n_with_label(USER_LABEL)
                .where_(Predicate::eq_param("email", "email"))
                .limit(1)
                .project(user_projection()),
        )
        .returning(["user"])
}

#[allow(clippy::too_many_arguments)]
#[register]
fn add_user_mutation(
    id: i64,
    first_name: String,
    last_name: String,
    email: String,
    api_key: String,
    role: String,
    created_at: String,
    updated_at: String,
) -> WriteBatch {
    let _ = (
        &id,
        &first_name,
        &last_name,
        &email,
        &api_key,
        &role,
        &created_at,
        &updated_at,
    );

    write_batch()
        .var_as(
            "existing_user",
            g().n_with_label(USER_LABEL)
                .where_(Predicate::eq_param("email", "email")),
        )
        .var_as_if(
            "updated_user",
            BatchCondition::VarNotEmpty("existing_user".to_string()),
            g().n(NodeRef::var("existing_user"))
                .set_property("id", PropertyInput::param("id"))
                .set_property("first_name", PropertyInput::param("first_name"))
                .set_property("last_name", PropertyInput::param("last_name"))
                .set_property("email", PropertyInput::param("email"))
                .set_property("api_key", PropertyInput::param("api_key"))
                .set_property("role", PropertyInput::param("role"))
                .set_property("created_at", PropertyInput::param("created_at"))
                .set_property("updated_at", PropertyInput::param("updated_at"))
                .project(user_projection()),
        )
        .var_as_if(
            "created_user",
            BatchCondition::VarEmpty("existing_user".to_string()),
            g().add_n(
                USER_LABEL,
                vec![
                    ("id", PropertyInput::param("id")),
                    ("first_name", PropertyInput::param("first_name")),
                    ("last_name", PropertyInput::param("last_name")),
                    ("email", PropertyInput::param("email")),
                    ("api_key", PropertyInput::param("api_key")),
                    ("role", PropertyInput::param("role")),
                    ("created_at", PropertyInput::param("created_at")),
                    ("updated_at", PropertyInput::param("updated_at")),
                ],
            )
            .project(user_projection()),
        )
        .returning(["updated_user", "created_user"])
}

#[register]
pub fn create_user_indexes() -> WriteBatch {
    write_batch()
        .var_as(
            "user_id_unique",
            g().create_index_if_not_exists(IndexSpec::node_unique_equality(USER_LABEL, "id")),
        )
        .var_as(
            "user_email_unique",
            g().create_index_if_not_exists(IndexSpec::node_unique_equality(USER_LABEL, "email")),
        )
}

fn user_projection() -> Vec<PropertyProjection> {
    vec![
        PropertyProjection::renamed("$id", "helix_id"),
        PropertyProjection::new("id"),
        PropertyProjection::new("first_name"),
        PropertyProjection::new("last_name"),
        PropertyProjection::new("email"),
        PropertyProjection::new("api_key"),
        PropertyProjection::new("role"),
        PropertyProjection::new("created_at"),
        PropertyProjection::new("updated_at"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(email: &str) -> UserNode {
        UserNode {
            id: 1,
            first_name: "Ada".to_string(),
            last_name: "Lovelace".to_string(),
            email: email.to_string(),
            api_key: "test-key".to_string(),
            role: "Analyst".to_string(),
            created_at: "2026-08-05T00:00:00Z".to_string(),
            updated_at: "2026-08-05T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn save_user_builds_a_query_for_a_valid_user() {
        assert!(save_user(user("ada@example.com")).is_ok());
    }

    #[test]
    fn save_user_rejects_an_empty_email() {
        assert_eq!(
            save_user(user("  ")).unwrap_err(),
            "user email cannot be empty"
        );
    }

    #[test]
    fn get_user_by_email_validates_the_lookup_key() {
        assert!(get_user_by_email("ada@example.com".to_string()).is_ok());
        assert_eq!(
            get_user_by_email(" ".to_string()).unwrap_err(),
            "user email cannot be empty"
        );
    }
}
