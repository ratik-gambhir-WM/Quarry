//! Isolated SharePoint / Microsoft Graph client.
//!
//! This module mirrors the standalone TypeScript SharePoint client. It is
//! intentionally not registered in `core::clients` yet, so consumers can wire
//! it into the backend when they are ready.

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
