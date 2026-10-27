# npm dependency health

Checked on 2026-10-19 using Node 24.14.0 and npm 11.9.0.

## Result

Both npm projects have consistent manifests and lockfiles, installed direct
dependencies, passing typechecks and test suites. Both frontend targets build.
The security audits still have unresolved findings; this is not a clean audit.

The repository root has no npm package. Run npm commands from `frontend/` or, for the optional
legacy service, its separate checkout at `$HOME/diligence-studio-server/`.

## Changes made

- Added `@embedpdf/plugin-i18n` 2.15.1 to the frontend's declared dependencies.
  The existing PDF editor imports `@embedpdf/plugin-stamp`, which declares this
  required peer. It was the missing peer relevant to the active React editor.
- Applied `npm audit fix` in both projects without `--force`. This installed
  compatible fixes and updated the lockfiles within declared dependency ranges.
- Frontend `shadcn` changed from 4.19.0 to 4.22.1; its dependency tree includes
  security updates. React and React DOM remain on the same 19.3 canary, and the
  frontend's Vite 7 and Vitest 3 versions remain unchanged.
- Diligence Studio's Vitest changed from 4.1.10 to 4.1.12. Its supported transitive
  Vite dependency resolved from 7.3.2 to 8.3.4; the server's typecheck and complete
  test suite pass with this tooling tree.
- Preserved the frontend `.npmrc`, existing package ranges, and unrelated work.
  No application source, API contract, Rust dependency, or environment file changed.

## Remaining security findings

Counts are npm's affected-package counts, including dependent packages, rather
than counts of distinct exploits or proof that every advisory is reachable.

| Project | Before | After |
| --- | --- | --- |
| Frontend | 20: 3 critical, 12 high, 5 moderate | 10: 2 critical, 7 high, 1 moderate |
| Diligence Studio | 10: 1 critical, 6 high, 2 moderate, 1 low | 2 high |

The remaining chains need deliberate follow-up:

- **Frontend test tooling:** Vitest 3.2.7 and its mocker/tinypool dependencies
  remain flagged, including critical npm audit findings. npm proposes Vitest
  5.0.5, which is outside the declared major. A test-runner migration and its
  configuration/API checks should precede that change. See the maintainers'
  [Vitest advisory](https://github.com/vitest-dev/vitest/security/advisories/GHSA-82fw-gwwq-j7x9)
  and [tinypool advisory](https://github.com/tinylibs/tinypool/security/advisories/GHSA-85c8-ppgw-ccpr).
- **Frontend shadcn CLI tooling:** the updated shadcn tree still includes
  `braces` through `micromatch`/`fast-glob`, registry tooling and `ts-morph`.
  npm proposes downgrading shadcn to 1.0.0. That was not applied. Track a fixed
  upstream tree or review a separately scoped tooling change; do not force a
  downgrade merely to reduce the audit count. See the
  [braces advisory](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm).
- **Diligence Studio PowerPoint conversion:** `pptxgenjs` 4.0.1 requires
  `image-size` `^1.2.1`, which remains flagged for denial of service. npm proposes
  downgrading `pptxgenjs` to 4.0.0. The patched image-size 2.x releases are outside
  the declared range, so substituting them needs an API compatibility review and
  conversion/parity tests. No override or conversion-library downgrade was made.
  See the [image-size advisory](https://github.com/advisories/GHSA-5p2g-fcmc-qvqq).

Other newer package versions are available, including major releases. They were
checked with `npm outdated` but were not applied as a blanket update.

## Peer dependency warnings

`npm ls --depth=0` passes for the frontend, and `npm ls --all` passes for Diligence
Studio. The frontend's full `npm ls --all` still exits with peer warnings:

- the intentionally retained React/React DOM/scheduler canary does not satisfy
  some libraries' declared stable-release peer ranges;
- EmbedPDF declares Preact, Svelte and Vue peers for its alternate framework
  bindings, which this React application does not install or use.

The existing `legacy-peer-deps=true` configuration remains in place. These
warnings are not a claim that the application supports those other frameworks;
they also mean the full frontend dependency tree cannot be reported as free of
peer warnings. The missing stamp/i18n peer no longer appears.

## Verification performed

From both npm project roots:

```sh
npm ci --dry-run --ignore-scripts --no-audit --no-fund --offline
npm run typecheck
npm test -- --reporter=dot
```

The `npm ci` commands were dry runs, not clean reinstalls; the preceding install
and audit-fix commands performed the dependency installation. A temporary npm
cache was supplied to these checks. Frontend tests: **271 passed across 68
files**. Diligence Studio tests: **134 passed across 16 files**. The first server
test run failed because the sandbox blocked Supertest's temporary HTTP listeners;
rerunning with local socket access passed.

From `frontend/`, all passed:

```sh
npm run check:boundaries
npm run build:web
npm run check:web-bundle
npm run check:bundle-size:web
npm run build:desktop-ui
npm run check:bundle-size:desktop
```

Both web and desktop typechecks are included in `npm run typecheck`. Each
bundle-size check followed its matching build. Vite's generic large-chunk warning
does not fail the repository's application-chunk budget check.

Live browser/Tauri interaction, real PowerPoint preview imports, Chromium
provisioning, native packaging, and backend Rust suites were not exercised during
this npm-only maintenance. These automated checks do not establish full runtime
or production security readiness. `npm audit` still exits nonzero for the
findings above; `npm audit fix` likewise returned nonzero after installing its
compatible fixes because findings remain.
