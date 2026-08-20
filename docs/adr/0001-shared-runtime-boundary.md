# ADR 0001: One frontend with a build-time runtime boundary

Status: accepted for the initial consolidation  
Date: 2026-08-18

## Context

Quarry previously had a browser React application backed by Axum and a second React application
backed by an in-process Tauri product backend. The duplicate UI and business-logic paths had
drifted, and desktop-created data was not the same product data seen in the browser.

## Decision

- `frontend/src` is the only maintained React source tree.
- Vite mode `web` aliases `@quarry/router` to `BrowserRouter` and `@quarry/runtime` to the web
  platform adapter.
- Vite mode `desktop` aliases those modules to `HashRouter` and the Tauri platform adapter.
- Both adapters compose the same versioned HTTPS `QuarryApi` client.
- Platform code owns only capabilities that truly differ. The initial native boundary contains one
  validated save/export command.
- The Tauri shell bundles the frontend and does not contain users, deals, SQLite, OpenAI, Helix,
  parsers, repositories, or document jobs.

## Consequences

The normal web bundle has no reachable Tauri import. Desktop and web releases share product data
through the Axum API. Native capabilities must be added deliberately, with a transport-neutral
contract, narrow Tauri permission/configuration, input validation, origin checks, and tests.

The inherited Axum service is still a development backend. Identity, tenancy, durable production
storage/jobs, deployment providers, signing, and updates remain release gates in the deployment
plan.
