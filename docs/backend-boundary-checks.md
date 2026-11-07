# Backend validation and authorization boundaries

The 2026-11-07 audit separates checks that can reject an HTTP request before product work from
checks that require loaded state or must remain current during an operation. This document
describes the boundary refactor; it is not a claim that all validation or authorization can be
performed without reading canonical state.

## Placement decision for every new or changed check

Identify the check's inputs and earliest safe execution point before implementing it. Use this
decision table during implementation and review:

| What the check needs | Owner and timing |
| --- | --- |
| Only caller-supplied fields: presence, syntax, ranges, combinations, duplicate IDs, declared scope, or filenames | Handler-owned `handler/validate.rs`, before service invocation; prefer cheap checks before costly content processing when their inputs are already available |
| Bearer/header syntax and verified identity | Shared HTTP authentication boundary, before protected handlers; check syntax before session lookup |
| Authorization for a request-selected resource | HTTP handler or handler-owned `auth.rs`, using a service/policy interface to resolve canonical state |
| Loaded ownership, current membership/version, revisions, canonical paths/bytes, or results | Keep the necessary check after that state exists and at the protected read/write/result boundary |
| Authorization during queued or streaming work | Recheck in the running worker/stream; initial request authorization does not replace it |
| Configuration, upstream responses, or outbound protocol invariants | Configuration/integration boundary; shared parsers remain HTTP-independent |

A check retained later needs a concrete state or lifecycle reason in the change description.
Calling it business logic is not sufficient if all its inputs were available from the request.
Move its early portion to the handler when a check combines request facts with loaded-state
constraints, while preserving the necessary later guard.

Reviewers must check that:

- Invalid input and denied initial access stop before protected product work or side effects.
- Models, services, and adapters do not acquire HTTP extraction/error dependencies or duplicate
  caller-field policy. Routes compose handlers and routing/body-limit policy.
- Dedicated validation files use `validate.rs`; handler-owned multipart extraction can use
  `upload.rs` and handler-owned authorization can use `auth.rs`.
- Required state-dependent and transaction guards are preserved, including access changes between
  preflight and use. Handlers do not query repositories directly to perform these checks.
- Relevant route-level regression tests demonstrate ordering. A helper unit test alone does not
  prove that the handler calls it before invoking a provider or starting a mutation.
- The architecture gate passes: from `backend/`, run
  `cargo test --locked --lib architecture_tests`, or the full backend suite that includes it.

The architecture gate catches forbidden dependencies; it cannot prove the semantics or timing of
every check. The decision review and behavioral tests are required alongside those static guards.

## Early request checks

| Area | Handler-owned checks and changes |
| --- | --- |
| Assistant chat | Prompt/model/context rules moved out of the model into `handler/validate.rs`; file reference identity, scope, names, duplicates, and combined count are checked before canonical resolution. The redundant request-scope branch was removed from the file loader. |
| Multipart inputs | Assistant, deal metadata, research, and summaries use handler-owned `upload.rs` modules. Streaming byte/count limits remain beside extraction; summaries reject an empty accepted upload before service invocation. |
| Deals | Acquisition target requirements depend only on submitted status/transaction/parties, so create and update handlers enforce them alongside existing ID/date/link/field rules. |
| Research / WM AI | Empty document text/files, empty questions, resource IDs, and path-segment rules moved from the adapter to handler-owned `validate.rs`. The adapter safely encodes outbound path segments. |
| Identity | Bearer syntax moved out of the identity service into the HTTP authentication boundary. Both token and supplied user-ID syntax are checked before session lookup; the identity match is checked after lookup. Registration/login input checks remain handler-owned. |
| Memberships | HTTP handlers and their validation were split out of routing. `route.rs` now composes endpoints; `handler/validate.rs` owns paging, IDs, revision, name, role, and email checks. |
| Companies and notebooks | Existing request validators were collected into `handler/validate.rs`; owner-scoped persistence and note-content integrity remain enforced. |
| Documents | Search, ingestion, and viewing request validators use `handler/validate.rs`. Job access, result filtering, document identity, parser, and version integrity remain close to their data. |
| Data rooms | Relative-path syntax is checked before looking up/opening the configured source. The same pure parser also supports the canonical filesystem guard; symlink containment is still checked after canonicalization. |
| Templates | Existing HTTP ID/header/query/document/upload-name validators use `handler/validate.rs`. User scoping/authentication and admission moved out of both template routers into handler-owned `auth.rs`; rendering, package, persisted asset, and output-size guards retain their existing owners. |

Dedicated validation modules are named `validate.rs`. Data types stay in `model.rs`. Pure
format/path parsers can be shared with integrity checks but do not import Axum or `AppError`.
Handlers invoke services and policy interfaces rather than repositories or infrastructure clients.

Malformed WM AI input now returns 400 before optional provider lookup, including when WM AI is
unconfigured; valid requests still report the existing unconfigured capability response. Dot path
segments are rejected as resource IDs, and query/fragment/percent characters in otherwise accepted
IDs are encoded as literal outbound path data. No route, JSON shape, SSE event, or schema changes.

## Authorization that needs current state

Common bearer, verified actor, header consistency, and path-capability checks already run at the
HTTP boundary before protected handlers. Caller-controlled body scope is checked there as well.
Policy methods may load canonical state through service interfaces; their data lookup is not
replaced by trusting caller-supplied ownership or role fields.

The following checks remain after the initial boundary because moving them exclusively to an
earlier point would lose information or introduce a stale authorization window:

- Canonical file reference resolution checks current membership before loading each stored blob.
  Its size/type checks and aggregate byte budget require actual canonical bytes. Client display
  names never choose storage objects.
- A persisted assistant run checks its scope against the loaded owned thread. Existing completed
  runs retain their replay path before resolving file bytes.
- Membership, deal, notebook, document, and template persistence keeps owner/capability/revision
  predicates and atomic transaction checks with the rows being read or mutated.
- Search authorizes current canonical candidates and final results, including cached work.
- Queued ingestion/notebook jobs and live SSE streams recheck access/session revocation while
  running; initial handler authorization does not grant permanent access.
- Filesystem access verifies canonical containment after resolving the real path. Parsers,
  providers, stored identity checkpoints, and generated output retain integrity and safety bounds.
- Configuration and outbound protocol checks remain at their own configuration/integration
  boundaries. They are not validation of an incoming product HTTP request.

## Verification

The architecture suite covers HTTP dependencies outside handlers, service imports of the handler
layer, source/test mirroring, and existing dependency boundaries. Regression tests exercise invalid
research requests before provider lookup, assistant scope rejection before file/provider access,
relative-path rejection before source lookup, safe outbound ID encoding, and reference-count
overflow. Existing authentication, membership, notebook, stored-file, replay, and search suites
cover the retained state-dependent protections.

Use the backend's normal formatting, all-target compilation, Clippy, and test gates. Live provider,
Chromium, and disposable-Helix opt-in fixtures are not needed for this ownership refactor; they
remain separately invoked integration checks. No real configuration, database, or graph is used
as a verification shortcut.

Verification completed from `backend/` on 2026-11-07:

| Command | Outcome |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --locked --all-targets` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `cargo test --locked --all-targets` | Final run: 524 passed, 0 failed, 10 opt-in tests ignored |

The architecture suite passed all 11 checks. Backend skill frontmatter, guidance links, final
status, and `git diff --check` were also checked. The relocated acquisition-rule fixture needed
its original required nullable fields restored. A full run overlapping compilation work also
failed the unchanged two-second QuickLook buffer fixture; that fixture passed in isolation,
and the subsequent complete default-parallelism run passed without concurrent compilation.
No QuickLook code or timeout was changed.

The ten live/browser/interactive opt-in checks were subsequently exercised at the user's request,
as recorded below. Frontend, Tauri, and product browser walkthroughs were not rerun because their
code and contracts were unchanged.
The audit retained the existing uncommitted frontend, file-reference, and SQLite-pool work.

### Requested opt-in rerun

Rerun completed on 2026-11-07 against the current boundary-refactor code. A fresh pinned-image
Helix container used port 16969 without host data mounts; full Chrome used temporary test profiles.

From `backend/`:

```sh
QUARRY_HELIX_COMPAT_URL=http://127.0.0.1:16969 \
QUARRY_SLIDE_TEMPLATES_CHROMIUM_PATH='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' \
cargo test --locked --all-targets -- --ignored --test-threads=1 \
  --skip notebook_visual_fixture --skip serve_disposable_slide_template_validation_api
```

Result: **8 passed, 0 failed, 0 ignored**: four browser tests and four live Helix tests.

The two skipped names are interactive server fixtures. They were launched separately with their
exact test names, `--exact --ignored --nocapture`, using test ports 3002 and 39001:

- `app::bootstrap::tests::serve_disposable_slide_template_validation_api`: health/capability
  checks, unauthorized access, registration, and authenticated access through both template
  route families passed. Graceful shutdown returned a passing Cargo result.
- `notebook_http_tests::notebook_visual_fixture`: health/capability checks, unauthorized access,
  authenticated notebook listing, and notebook create/delete passed. This fixture runs
  indefinitely and was intentionally interrupted after its checks; it is not counted as a
  completed Cargo test.

Both fixture processes were stopped, the owned test container was removed, and all three test
ports were verified free. The existing API process and normal Helix container remained running.
No application code changed for this rerun.
