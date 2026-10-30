use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

pub(super) const MAX_REVISION: i64 = 9_007_199_254_740_991;
pub(super) const PIPELINE: &str = "notebook-markdown-v1/chunks-v1";

#[derive(Debug, Default, Clone, Serialize, PartialEq)]
#[serde(untagged)]
pub(super) enum PatchField<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}
impl<T> PatchField<T> {
    pub fn missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
    pub fn apply(self, old: Option<T>) -> Option<T> {
        match self {
            Self::Missing => old,
            Self::Null => None,
            Self::Value(value) => Some(value),
        }
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for PatchField<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct NotebookInput {
    #[serde(default)]
    pub title: PatchField<String>,
    #[serde(default)]
    pub deal_id: PatchField<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FolderInput {
    pub name: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SaveInput {
    pub operation_id: String,
    #[serde(default, skip_serializing_if = "PatchField::missing")]
    pub expected_revision: PatchField<i64>,
    #[serde(default, skip_serializing_if = "PatchField::missing")]
    pub title: PatchField<String>,
    #[serde(default, skip_serializing_if = "PatchField::missing")]
    pub folder_id: PatchField<String>,
    #[serde(default, skip_serializing_if = "PatchField::missing")]
    pub blocks: PatchField<Value>,
    #[serde(default, skip_serializing_if = "PatchField::missing")]
    pub content_schema_version: PatchField<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Notebook {
    pub notebook_id: String,
    pub title: String,
    pub deal_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Folder {
    pub folder_id: String,
    pub notebook_id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NoteSummary {
    pub note_id: String,
    pub notebook_id: String,
    pub folder_id: Option<String>,
    pub title: String,
    pub content_schema_version: i64,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Note {
    #[serde(flatten)]
    pub summary: NoteSummary,
    pub blocks: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct AcceptedSave {
    pub pipeline_version: String,
    pub note: Note,
    pub expected_revision: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveJob {
    pub job_id: String,
    pub notebook_id: String,
    pub note_id: String,
    pub status: String,
    pub stage: String,
    pub attempt: i64,
    pub sequence: i64,
    pub committed_revision: Option<i64>,
    pub sqlite_committed: bool,
    pub error_code: Option<String>,
    pub retryable: bool,
    pub status_url: String,
    pub events_url: String,
}
impl SaveJob {
    pub fn terminal(&self) -> bool {
        self.status == "completed" || self.status == "failed"
    }
}
pub(super) struct ClaimedJob {
    pub job: SaveJob,
    pub owner: i64,
    pub token: String,
    pub input: AcceptedSave,
}
#[derive(Default)]
pub(super) struct ListOptions {
    pub limit: usize,
    pub offset: usize,
    pub relationship: Option<String>,
    pub query: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Page<T> {
    #[serde(flatten)]
    pub items: T,
    pub limit: usize,
    pub offset: usize,
    pub has_more: bool,
}
impl<T> Page<T> {
    pub fn from_rows<R>(
        mut rows: Vec<R>,
        options: &ListOptions,
        wrap: impl FnOnce(Vec<R>) -> T,
    ) -> Self {
        let has_more = rows.len() > options.limit;
        rows.truncate(options.limit);
        Self {
            items: wrap(rows),
            limit: options.limit,
            offset: options.offset,
            has_more,
        }
    }
}
#[derive(Serialize)]
pub(super) struct Notebooks {
    pub notebooks: Vec<Notebook>,
}
#[derive(Serialize)]
pub(super) struct Folders {
    pub folders: Vec<Folder>,
}
#[derive(Serialize)]
pub(super) struct Notes {
    pub notes: Vec<NoteSummary>,
}

pub(super) fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
#[path = "../../../tests/unit/domains/notebooks/model_tests.rs"]
mod tests;
