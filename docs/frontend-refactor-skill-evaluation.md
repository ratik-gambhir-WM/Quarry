# Frontend refactor skill evaluation

Evaluated 2026-11-07 for global Codex installation and Quarry's frontend review workflow.

## Source and installation

The user supplied [MCP Market's Frontend Refactor & Modernization listing](https://mcpmarket.com/tools/skills/frontend-refactor-modernization).
Its SKILL.md tab identifies LerianStudio's `ring:dev-refactor-frontend`, revision
`dd5386662f8c6d7b58273644b83cdef9f1c11faa`, at
`dev-team/skills/dev-refactor-frontend/SKILL.md`.

The requested `npx skillfish add lerianstudio/ring dev-refactor-frontend` installation could not
resolve the original repository. The exact listed revision was retrieved from the available
[mirror](https://raw.githubusercontent.com/LucasMalessa/TheRing/dd5386662f8c6d7b58273644b83cdef9f1c11faa/dev-team/skills/dev-refactor-frontend/SKILL.md).
The installed `dev-refactor-frontend` is an evaluated Codex adaptation, not an unchanged upstream
skill or a Skillfish-managed installation. Its global folder retains the original text, source
revision, checksum, adaptation notes, and Apache-2.0 license.

Both `refactor` and `dev-refactor-frontend` live globally in Codex, currently under
`~/.codex/skills/`. Quarry does not vendor either skill under `.agents/skills/`.
Agents resolve global skill paths from their current skill catalog. Quarry's project-specific
development and verification skills remain local.

## Quarry baseline

The comparison used [AGENTS.md](../AGENTS.md), the
[frontend development skill](../.agents/skills/react-vite-development/SKILL.md), the
[review skill](../.agents/skills/code-review-and-quality/SKILL.md), the
[verification skill](../.agents/skills/code-verification/SKILL.md), and the live
[frontend manifest](../frontend/package.json), lockfile, and
[Vite configuration](../frontend/vite.config.ts).

Quarry uses a shared client-rendered React/Vite application with React Router, separate web and
desktop runtime aliases, existing UI primitives and theme tokens, Vitest, and handwritten API
adapters. It does not use Next.js App Router, React Server Components, or a frontend server-side
BFF. A browser adapter directory named `api` does not establish a BFF. Existing instructions
already cover state ownership, accessibility, async states, dependency direction, user-work
preservation, and both runtime targets.

## Decisions

| Upstream recommendation | Decision | Reason and resulting guidance |
| --- | --- | --- |
| Inspect actual architecture before making recommendations | Adopt | Establish the build root, framework capabilities, callers, and contracts before flagging a gap. |
| Explicit review coverage across architecture, UI, accessibility, performance, and testing | Adopt with scope | A short coverage record exposes missed areas in ordinary frontend changes; a larger requested audit can use a table. Uninspected areas remain explicit limitations. |
| Findings include source locations, impact, and concrete remedies | Adopt | Reviewers can assess and verify a proposed change. This strengthens the handoff without requiring a separate report for every feature. |
| Trace findings through implementation and verification | Adopt with scope | Accepted audit findings map to tasks and acceptance criteria. Routine reviews record fixed, deferred, or outside-scope dispositions; distinct issues remain visible. |
| Identify the check that missed a regression | Adopt | A demonstrated escaped defect should inform the smallest useful regression check, rather than a ceremonial gate count. |
| Every difference from Ring standards is mandatory work | Reject as a blanket rule | Differences can be intentional and correct. Require evidence of a real defect or measurable improvement; separate required changes from optional improvements. |
| Missing server components or App Router is a high-severity problem in a React application | Reject for Quarry | These are Next.js-specific capabilities. Adding them would change Quarry's runtime architecture without solving a demonstrated problem. |
| Replace libraries that differ from Ring's preferred stack | Reject as a blanket rule | Migration cost, compatibility, and user benefit determine whether a change is justified. Existing primitives and contracts remain the starting point. |
| Always dispatch five to seven named Ring agents and invoke Ring-only tools/cycles | Adapt | The standalone Codex skill uses available tools and does not require an uninstalled agent ecosystem. Delegation follows the session's existing authorization. |
| Stop without remote CLAUDE.md, PROJECT_RULES.md, or shared Ring files | Adapt | Quarry already has maintained governing instructions and architecture. No duplicate rules document or unavailable remote dependency is necessary. |
| Always generate separate reports, a visual HTML artifact, and one task per duplicate finding | Adapt | Keep task traceability for substantial requested audits; consolidate duplicate root causes and scale artifacts to the request. |
| Always require new approval before implementation | Adapt | Honor analysis-only requests and ask for genuinely new scope; existing authorization covers coherent, reversible work already requested. |
| Impose coverage percentages, snapshot systems, Docker, Makefiles, or CI as universal requirements | Reject as a blanket rule | Check actual behavior and real scripts. New tooling requires a concrete need and belongs in its own justified scope. |

## Guidance changes

The post-feature section of `AGENTS.md` now references the global refactoring skills, requires
frontend review coverage and finding dispositions, and explains how to evaluate and explicitly
adopt a better recommendation. The project frontend skill incorporates the same review workflow.
These changes improve the previous guidance; installing a third-party skill does not silently
replace it. Governing instructions are updated when an evaluated improvement is accepted.

The adaptation retains automatic skill discovery. It has no remote standards fetch, Ring agent,
framework migration, new approval ceremony, or project-local refactor installation requirement.
This evaluation covers the skill and instruction changes; it is not a new audit of Quarry's
application code or evidence that every UI route was inspected.

## Verification

Validate the global adaptation and changed project frontend skill with the skill-creator
`quick_validate.py` helper. Check local links and source checksums, verify both global skill
folders exist and both project-local refactor folders are absent, and inspect `git diff --check`
and final status. Application builds and tests are unnecessary for instruction-only changes.
The source archive is retained for comparison and is not an active instruction dependency.
