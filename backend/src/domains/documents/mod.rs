pub mod formats;
pub mod ingestion;
pub mod model;
pub mod search;
pub mod store;
pub mod viewing;

pub use formats::docx::parse_docx_from_path;

#[cfg(test)]
#[path = "../../../tests/integration/domains/documents/support.rs"]
pub(crate) mod test_support;
pub(crate) use store::access_repository::DocumentAccessRepository;
