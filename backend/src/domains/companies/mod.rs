pub(crate) mod handler;
pub(crate) mod model;
pub(crate) mod repository;
pub(crate) mod route;
pub(crate) mod service;
pub use model::Company;
// Persistence row mapping is shared with deal aggregate snapshot assembly.
pub(crate) use repository::company_from_row;
