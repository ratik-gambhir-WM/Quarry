use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Company {
    pub company_id: String,
    pub user_id: i64,
    pub name: String,
    pub website: Option<String>,
    pub industry: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompanyInput {
    #[serde(skip_deserializing)]
    pub user_email: String,
    pub name: String,
    pub website: Option<String>,
    pub industry: Option<String>,
    pub description: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyPage {
    pub companies: Vec<Company>,
    pub has_more: bool,
}
