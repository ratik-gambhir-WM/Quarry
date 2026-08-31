pub mod bootstrap;
pub mod config;
pub mod core;
pub mod document_jobs;
pub mod errors;
pub mod handlers;
pub mod repository;
pub mod routes;
pub mod services;
pub mod state;
pub mod utils;

mod events;

#[cfg(test)]
#[path = "../tests/architecture_tests.rs"]
mod architecture_tests;

pub use routes::create_router;
