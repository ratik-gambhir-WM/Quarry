//! Planned owner of durable tasks, approvals, and workflow execution state.
//!
//! This comment-only module does not provide a scheduler, worker, or durable queue. Workflow
//! actions must call public domain capabilities instead of importing their private layers.

// TODO: Define workflow definitions, runs, steps, transitions, and idempotency guarantees.
// TODO: Add durable execution and retry/cancellation policy with bounded background work.
// TODO: Add authorized command/query routes and deterministic transition tests before registration.
