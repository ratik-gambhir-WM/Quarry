//! Authenticated personal notebooks and durable, revision-scoped save jobs.
mod content;
mod graph;
mod handler;
mod job_repository;
mod markdown;
mod model;
mod repository;
pub(crate) mod route;
mod save_jobs;
mod service;

pub(crate) use graph::NotebookPipeline;
pub(crate) use repository::NotebookRepository;
pub(crate) use service::NotebookService;

#[cfg(test)]
#[path = "../../../tests/integration/notebook_support.rs"]
pub(crate) mod test_support;
