//! Isolated SharePoint / Microsoft Graph client.
//!
//! This module is exposed under `core::clients` but is not assembled by backend
//! bootstrap or wired into product routes and services.

mod auth;
pub mod cache;
pub mod client;
pub mod error;
pub mod graph;
pub mod services;
pub mod types;
pub mod utils;

pub use cache::InMemoryCache;
pub use client::SharePointClient;
pub use error::SharePointClientError;
pub use types::*;
