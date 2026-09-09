---
name: code-review-and-quality
description: Conducts multi-axis code review. Use before merging any change, after completing an implementation, or when reviewing code written by yourself, another agent, or a human.
---

# Code Review and Quality

Review changes across correctness, readability, architecture, security, and performance. Do not
rubber-stamp code because it works or because the author is an agent. Approve when the change
clearly improves overall code health and no required finding remains; perfection and personal
style preference are not the standard.

## Review process

1. Read the governing project instructions, the request or spec, and the change description.
2. Identify the intended behavior, affected runtime boundaries, and claimed verification.
3. Review tests first. Confirm they express the intended behavior, cover meaningful edge/error
   paths, and would fail for a real regression.
4. Review every changed file across the five axes below. Inspect nearby canonical helpers and
   architecture when ownership or dependency direction matters.
5. Verify the verification story: distinguish checks actually run from checks merely available,
   and inspect manual evidence for UI or integration changes when applicable.
6. Report findings first, ordered by severity and leverage. End with a clear verdict and concise
   verification assessment.

## Five review axes

### Correctness

- Does the implementation match the request and its observable contracts?
- Are null, empty, boundary, cancellation, stale, and error states handled where relevant?
- Are there race conditions, off-by-one errors, state inconsistencies, or silent fallbacks?
- Do tests validate behavior rather than implementation details?
- Would failures surface clearly rather than being mistaken for success?

### Readability and simplicity

- Are names specific and consistent with project conventions?
- Is control flow direct, or does it rely on deep nesting, clever tricks, or scattered conditions?
- Could the behavior be expressed with materially fewer concepts or moving parts?
- Does each abstraction earn its complexity? Avoid generalizing before repeated use justifies it.
- Did the change leave dead code, no-op variables, compatibility shims, or historical comments?
- Is a feature-specific conditional bolted onto an unrelated flow? If so, propose moving the
  policy into its owning helper, model, or state rather than normalizing the tangle.

### Architecture

- Does the change follow the repository's dependency direction and ownership boundaries?
- Is feature-specific logic leaking into a shared or general-purpose module?
- Does it reuse the canonical helper rather than introduce a near-duplicate?
- Are type boundaries explicit, without gratuitous `any`, `unknown`, casts, optionals, or fallbacks
  that hide an unclear invariant?
- Does a refactor remove complexity, or merely relocate the same branches and concepts?
- Has a small diff pushed an already-large file toward an unhealthy boundary? Around 1,000 total
  lines is an inspection signal, not an automatic blocker.

When flagging a structural issue, name the remedy: collapse duplicate branches, introduce a typed
model or dispatcher, separate orchestration from business logic, move feature logic to its owner,
reuse the canonical helper, make the type boundary explicit, delete a pass-through wrapper, or
split the file into focused modules. Prefer remedies that remove moving pieces.

### Security

- Is input validated at the real trust boundary and output encoded for its destination?
- Are authentication and authorization enforced server-side where required?
- Are queries parameterized and external data treated as untrusted?
- Are secrets absent from code, client bundles, logs, fixtures, screenshots, and version control?
- Are dependency sources, licenses, and known vulnerabilities acceptable?

### Performance

- Are there N+1 queries, unbounded loops, fetches, queues, or collections?
- Are blocking operations placed on an async worker or request path?
- Are list endpoints paginated where scale requires it?
- Does UI code introduce avoidable rendering, allocation, or subscription work?
- Is a claimed optimization supported by measurement or a concrete cost model?

## Change sizing and separation

Use size as a reviewability signal:

- About 100 changed lines is usually easy to review.
- About 300 changed lines can be reasonable for one coherent change.
- About 1,000 changed lines usually warrants splitting unless it is a deletion or mechanical
  refactor whose intent can be verified independently.

Separate feature behavior from substantial refactoring. Split by dependency stack, file group,
horizontal layer, or vertical feature slice while keeping every submitted change functional.

## Dependency discipline

Before accepting a new dependency, check whether the existing stack or standard library already
solves the need, then assess size, maintenance, license, vulnerabilities, and transitive impact.

For upgrades:

1. Read the changelog and migration notes rather than trusting semver alone.
2. Prefer one dependency, or one tightly related group, per change.
3. Verify relevant behavior before and after the upgrade.
4. Review the lockfile diff and transitive graph; never hand-edit a lockfile.

## Dead-code hygiene

After a refactor or implementation, identify newly unreachable or unused elements and list them
explicitly. Do not silently delete uncertain or pre-existing code during a review. Ask before
removing anything whose ownership or continued use is unclear.

## Findings and severity

Use these labels consistently:

| Severity | Meaning | Author action |
| --- | --- | --- |
| **Critical** | Security vulnerability, data loss, or broken core behavior | Blocks merge |
| **Required** | Correctness, architecture, or maintainability issue introduced by the change | Fix before merge or explicitly justify deferral |
| **Optional** / **Consider** | Improvement that is worthwhile but not required | Author discretion |
| **Nit** | Minor preference or polish | May be ignored |
| **FYI** | Context with no requested change | No action |

Attach findings to the narrowest useful file and line range. Explain the concrete failure mode,
when it occurs, and the smallest credible remedy. A few high-confidence findings are better than a
long list of speculative nits.

If there are no findings, say so directly, but still report residual risks or unverified areas.
Do not output an evidence-free `LGTM`.

## Review verdict

Choose one:

- **Approve** — the change improves code health and has no unresolved Critical or Required issue.
- **Request changes** — at least one Critical or Required issue remains.

Technical facts and measurements override opinions. Project style guides control style. Evaluate
design using engineering principles and repository consistency, and accept an informed author
override gracefully when it does not leave a correctness or safety defect.

Finish with:

- findings, ordered Critical → Required → Optional/Nit;
- verdict;
- verification performed or inspected;
- remaining uncertainty;
- newly identified dead code, if any.
