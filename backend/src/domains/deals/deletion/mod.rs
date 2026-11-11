//! Durable permanent-deletion workflow. Receipts outlive the removed deal aggregate.
pub(crate) mod handler;
pub(crate) mod lifecycle;
pub mod model;
pub(super) mod repository;
pub(crate) mod route;
pub(crate) mod service;
mod worker;
pub(crate) use repository::barrier;
pub(crate) use repository::DeletionRepository;
