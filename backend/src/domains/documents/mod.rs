pub mod formats;
pub mod index;
pub mod ingestion;
pub mod model;
pub mod search;
pub mod store;
pub mod viewing;

pub use formats::docx::parse_docx_from_path;
