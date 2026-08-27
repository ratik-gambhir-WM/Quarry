# ADR 0002: Clear and reindex Helix for the versioned file graph

Status: accepted  
Date: 2026-08-26

## Decision

Quarry will roll out the versioned file graph with a controlled Helix clear and SQLite-backed
reindex. The old `QuarryFile(document_id, user_id) -[:HAS_CHUNK]-> Chunk` graph is not compatible
with the new `QuarryFile -[:HAS_VERSION|CURRENT_VERSION]-> FileVersion -[:HAS_CHUNK]-> FileChunk`
schema, so the two schemas must not be served together.

The clear is an explicit deployment operation. Normal API startup creates the required indexes but
does not delete graph data. SQLite remains the source for logical files, immutable versions, and
blobs during the reindex.

## Rollout procedure

1. Stop or drain document ingestion and record the SQLite database and Helix deployment being
   migrated.
2. Back up the SQLite database and take the environment's supported Helix snapshot/export.
3. Run `cargo run --bin clear_helix` from `backend/` against the selected Helix environment. This
   command is destructive and verifies that no nodes remain.
4. Start the updated API once to create the `QuarryFile`, `FileVersion`, and `FileChunk` indexes.
5. Reindex every current SQLite file version through the versioned ingestion path. If historical
   versions must be searchable immediately, enqueue them from oldest to newest and finish by
   indexing the SQLite version whose `is_current` value is true.
6. Verify node identities, `HAS_VERSION`, `CURRENT_VERSION`, `HAS_CHUNK`, vector search, and keyword
   search for each workspace before restoring document ingestion traffic.

If the clear or reindex fails, keep ingestion drained, retain the valid SQLite data, repair Helix,
and retry with the same SQLite `file_id` and `version_id` values. The graph write is idempotent for
those identities.

## Consequences

Legacy Helix-only data is not preserved automatically. A future deployment that cannot tolerate a
clear must implement and validate a dedicated one-time graph migration before changing this
decision.
