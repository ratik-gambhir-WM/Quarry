use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedFileIdentity {
    pub file_id: String,
    pub deal_id: String,
    pub display_name: String,
    pub version_id: String,
}

/// Fully validated values used to persist one logical file aggregate to PostgreSQL.
///
/// The repository supplies the transaction-derived version number; every other
/// value needed by `quarry_files`, `quarry_file_versions`, and
/// `quarry_file_blobs` is represented here before the transaction begins.
pub(crate) struct FilePersistenceInput {
    pub(crate) deal_id: String,
    pub(crate) file_id: String,
    pub(crate) actor_id: i64,
    pub(crate) display_name: String,
    pub(crate) source_uri: Option<String>,
    pub(crate) metadata_json: String,
    pub(crate) timestamp: String,
    pub(crate) version_id: String,
    pub(crate) mime_type: String,
    pub(crate) content_sha256: String,
    pub(crate) byte_size: i64,
    pub(crate) file_bytes: Vec<u8>,
    pub(crate) manifest: Option<super::identity::IdentityManifest>,
    pub(crate) image_assembly_json: Option<String>,
}

/// PostgreSQL projection used to verify or reactivate an existing file version.
pub(crate) struct ExistingFileVersion {
    pub(crate) version_id: String,
    pub(crate) byte_size: i64,
    pub(crate) is_current: bool,
}
