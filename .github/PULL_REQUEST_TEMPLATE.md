<!--
Tier-4 AI-guardrail (see ARCHITECTURE_PLAN.md §1.3, §14). This template
mirrors the AGENTS.md NEVER/ALWAYS lists so AI-authored PRs cannot quietly
skip them. Fill out every section; CI runs the same checks the boxes ask you
to confirm, so a green CI plus a complete template gives a reviewer a fast
path to confidence.
-->

## Summary

<!-- One paragraph. What changes, why now. Avoid restating the diff. -->

## Linked issue / context

<!-- Issue link, design doc, Slack/Linear thread, or "n/a" with a reason. -->

## Type of change

- [ ] Bug fix (non-breaking)
- [ ] Feature (non-breaking)
- [ ] Refactor / cleanup (no behavior change)
- [ ] Breaking change (API, schema, env var)
- [ ] Docs / infrastructure only

## Code quality checklist (AGENTS.md / CODE_STANDARDS.md)

Confirm each item. Do not check a box you have not actually verified.

### Rust backend

- [ ] `cd backend && cargo fmt -- --check` passes
- [ ] `cd backend && cargo clippy --all-targets -- -D warnings` passes
- [ ] `cd backend && cargo test` passes
- [ ] `cd backend && cargo deny check` passes
- [ ] `cd backend && cargo machete` passes
- [ ] No `.unwrap()` / `.expect()` outside `#[test]` / `tests/`
- [ ] No `Box<dyn Error>` (use `AppError` or a context-specific domain error)
- [ ] No `chrono::Local::now()` (use `chrono::Utc::now()`)
- [ ] No `todo!()`, `unimplemented!()`, `dbg!()`, `println!()`
- [ ] Every new public item has a doc comment
- [ ] Every fallible function returns `Result<T, AppError>` (or a context-specific domain error mapped into `AppError`)
- [ ] Important operations log structured fields via `tracing::info!` / `warn!` / `error!`

### TypeScript frontend

- [ ] `cd frontend && npm run lint` passes
- [ ] `cd frontend && npm run build` passes
- [ ] No `any` types and no `// @ts-ignore` / `// @ts-nocheck`
- [ ] All components have typed props
- [ ] Loading + error states handled in UI
- [ ] No hard-coded API URLs (read from `NEXT_PUBLIC_API_URL`)

### API / data

- [ ] Request bodies validated with `validator` (Rust) and `zod` (TS)
- [ ] Response shape matches `frontend/lib/api-types.ts` (no contract drift)
- [ ] Auth enforced on every protected endpoint
- [ ] No secrets in code or logs
- [ ] Database migrations (if any) are reversible or document why not
- [ ] Did **not** delete unused-but-capable template services/ports to shrink LOC (promote-and-wire instead; see `ARCHITECTURE_PLAN.md`)

## Manual testing

<!--
Describe how you verified the change end-to-end. For UI changes: include
screenshots or a screen recording. For backend changes: paste the curl/output
or a test log excerpt. "It compiles" is not testing.
-->

## Notes for reviewers

<!--
Anything you want surfaced: a known limitation, a follow-up issue, a place
you'd appreciate extra eyes, or "none — straightforward".
-->
