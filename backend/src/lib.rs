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

#[cfg(test)]
#[path = "../tests/integration/deal_company_http_tests.rs"]
mod deal_company_http_tests;

#[cfg(test)]
#[path = "../tests/integration/slide_templates/support.rs"]
mod slide_template_test_support;

#[cfg(test)]
#[path = "../tests/integration/slide_templates/pptx_parity_tests.rs"]
mod slide_templates_pptx_parity_tests;

#[cfg(test)]
#[path = "../tests/integration/slide_templates/repository_lifecycle_tests.rs"]
mod slide_templates_repository_lifecycle_tests;

#[cfg(test)]
#[path = "../tests/integration/slide_templates/repository_parity_tests.rs"]
mod slide_templates_repository_parity_tests;

#[cfg(test)]
#[path = "../tests/integration/slide_templates/api_tests.rs"]
mod slide_templates_api_tests;

#[cfg(test)]
#[path = "../tests/integration/slide_templates/integration_tests.rs"]
mod slide_templates_integration_tests;

#[cfg(test)]
#[path = "../tests/integration/slide_templates/headless_preview_tests.rs"]
mod slide_templates_headless_preview_tests;

#[cfg(test)]
#[path = "../tests/integration/authentication_tests.rs"]
mod authentication_tests;

#[cfg(test)]
#[path = "../tests/integration/notebook_http_tests.rs"]
mod notebook_http_tests;
