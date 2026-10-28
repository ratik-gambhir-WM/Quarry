//! Password authentication, revocable bearer sessions, and authenticated request identity.
pub mod access;
pub(crate) mod handler;
pub mod model;
pub(crate) mod repository;
pub(crate) mod route;
pub mod service;
