//! Native mechanisms for embedded Slide Templates Studio.
pub mod preview;
pub mod providers;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
#[derive(Clone, Default)]
pub struct RequestCancellation(pub Arc<AtomicBool>);
impl RequestCancellation {
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
}
