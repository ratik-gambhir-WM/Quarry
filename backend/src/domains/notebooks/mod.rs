//! Authenticated personal notebooks and durable, revision-scoped save jobs.
mod content;
mod handler;
mod job_repository;
mod markdown;
mod model;
mod pipeline;
mod repository;
pub(crate) mod route;
mod save_jobs;
mod search_repository;
mod service;

pub(crate) use pipeline::NotebookPipeline;
pub(crate) use repository::NotebookRepository;
pub(crate) use service::NotebookService;

#[cfg(test)]
#[path = "../../../tests/integration/notebook_support.rs"]
pub(crate) mod test_support;
