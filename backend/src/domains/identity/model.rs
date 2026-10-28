use crate::domains::users::User;
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegisterInput {
    pub username: String,
    pub password: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub job_title: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub user: User,
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_at: String,
}
#[derive(Clone)]
pub struct AuthenticatedUser {
    pub user: User,
    pub expires_at: i64,
    pub(crate) token_hash: String,
}
