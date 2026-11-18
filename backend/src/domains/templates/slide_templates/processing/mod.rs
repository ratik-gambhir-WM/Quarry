//! Classification/embedding lifecycle and durable work scheduling.
mod service;
mod worker;
pub use service::{SlideClassificationService, SlideClassificationServiceOptions};
pub use worker::{SlideClassificationWorker, SlideClassificationWorkerOptions};
