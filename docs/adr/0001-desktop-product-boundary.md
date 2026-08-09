# ADR 0001: Desktop product boundary and persistence ownership

Status: Accepted

Quarry’s React UI talks to one typed product adapter. Tauri commands are the trust boundary; React components do not issue SQL, Helix queries, or unrestricted filesystem operations. SQLite owns local account, deal, deal-metadata, archive state, and stored data-room roots. Helix owns user-partitioned document graphs, completion state, and chunk search.

Deal archive is an idempotent soft archive. Persisted metadata is flattened into the deal response while legacy investment-thesis data remains available as optional desktop briefing content. A stored data-room root is canonicalized before persistence and all later relative paths must remain beneath that canonical root.

Document ingestion starts only from paths granted by a Rust native picker. Commands validate count, type, size, canonical path, and the grant before starting retained UUID jobs. File bytes never cross IPC. Document identity is derived from user ID plus SHA-256 content hash; a Helix document is eligible for deduplication only after its ingestion-complete marker is set. Same-content work is serialized per user-scoped document identity.

West Monroe research clients remain optional. They are not exposed until their product requirement, service ownership, and secret provider are approved.
