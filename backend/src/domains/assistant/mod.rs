//! Assistant interaction ownership boundary.
//!
//! `chat` owns ephemeral textual multi-turn chat. A future `agent` sibling will own tool and
//! reasoning lifecycle; it must not widen the chat transport with provider-specific state.

pub mod chat;
