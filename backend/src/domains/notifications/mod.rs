//! Planned owner of notification preferences, delivery requests, and delivery status.
//!
//! This comment-only module does not send notifications or provide a durable queue. Channel
//! adapters and credentials must remain outside the domain and be injected at bootstrap.

// TODO: Define notification, preference, delivery-attempt, and idempotency models.
// TODO: Add durable enqueue/retry policy with bounded attempts and observable failure states.
// TODO: Add authorized preference and delivery APIs plus adapter-independent service tests.
