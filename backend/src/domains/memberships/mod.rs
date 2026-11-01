//! Deal membership is the canonical shared-project authorization boundary.
pub mod model;
pub(crate) mod repository;
pub(crate) mod route;
pub mod service;

pub(crate) use repository::{
    event as record_activity, initialize as initialize_deal, require as require_deal,
};
