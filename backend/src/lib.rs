pub mod adapters;
pub mod app;
pub mod domains;
pub mod shared;

#[cfg(test)]
#[path = "../tests/integration/architecture_tests.rs"]
mod architecture_tests;

#[cfg(test)]
#[path = "../tests/integration/http_tests.rs"]
mod http_integration_tests;

#[cfg(test)]
#[path = "../tests/integration/support.rs"]
mod integration_support;

#[cfg(test)]
#[path = "../tests/integration/helix_v3_compatibility_tests.rs"]
mod helix_v3_compatibility_tests;

pub use app::bootstrap::{bootstrap, Application};
pub use app::config::AppConfig;
pub use app::http::create_router;
