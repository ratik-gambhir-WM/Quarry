//! AI classification of ingested documents with OpenAI Decisions.
//!
//! Each background document worker samples its parsed chunks, classifies them beside
//! embedding and saves the outcome in the same transaction as the document. A failed
//! classification is saved as `failed`; it never fails the document.
pub(crate) mod handler;
pub mod model;
pub(crate) mod policy;
pub(crate) mod repository;
pub(crate) mod route;
pub(crate) mod sample;
pub mod service;
pub mod taxonomy;
