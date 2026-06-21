# Freshness Audit — Dependencies and Tooling

> Snapshot as of **2026-06-21**. Goal: shiny-but-not-bleeding-edge. Lean into latest *stable* where the cost is low; defer breaking-major migrations to focused follow-up PRs; flag deprecated paths explicitly so AI agents do not accidentally re-pick them.
>
> This document lives alongside `ARCHITECTURE_PLAN.md` §14 (Guardrails Inventory). Every "deferred" row is a follow-up PR slot.

---

## How to re-run this audit

```bash
# Rust — current resolved vs. latest published
cd backend
cargo update --dry-run
cargo tree --depth 0 --prefix none

# crates.io latest per direct dep
UA='vibe-coding-template (your-email-or-repo-url)'
for c in axum tower-http tokio reqwest validator async-openai qdrant-client; do
  curl -sS -A "$UA" "https://crates.io/api/v1/crates/$c" \
    | python3 -c 'import sys,json; print(json.load(sys.stdin)["crate"]["max_stable_version"])'
done

# Frontend
cd frontend
npm outdated --long
npm audit                          # security advisories
```

---

## Tier A — landed in this PR (low-risk: patches, semver-compatible bumps, narrow opt-in migrations)

### Rust backend

| Crate | Before | After | Notes |
|---|---|---|---|
| `cargo update` patch refresh | various | latest patches within bounds | tokio 1.52.1 → 1.52.3, serde_json 1.0.149 → 1.0.150, uuid 1.23.1 → 1.23.3, qdrant-client 1.17.0 → 1.18.0, plus ~30 transitive patches. Standard hygiene. |
| `tower-http` | 0.6 | **0.7** | Pre-1.0 minor bump = nominally breaking, but our usage (`cors`, `trace`, `request-id`) compiled clean with zero code changes. |
| `reqwest` | 0.12 | **0.13** | Feature `rustls-tls` was renamed to `rustls`. One-line `Cargo.toml` change; no code touched. `default-features = false` preserved so we still ship rustls-only (no OpenSSL link). |
| `async-openai` | 0.27 | **0.41** | SDK was restructured into per-endpoint feature flags (only opt into what you use). Two small source changes: types moved from `async_openai::types::*` to `async_openai::types::chat::*`, and `max_tokens(...)` builder call switched to `max_completion_tokens(...)` (OpenAI's new field for reasoning-token-aware accounting). |
| `deny.toml` ignores | 4 entries | **2 entries** | `RUSTSEC-2025-0012` (backoff) and `RUSTSEC-2024-0384` (instant) are gone — async-openai 0.41 dropped them. Two ignores remain (`proc-macro-error2` via validator, `rustls-pemfile` via tonic via qdrant-client); both are documented with the path and a removal trigger. |

### Frontend

| Package | Before | After | Notes |
|---|---|---|---|
| `@supabase/supabase-js` | ^2.39.3 | **^2.108.2** | Patch within the 2.x line; no API changes affecting us. |
| `@supabase/auth-helpers-nextjs` | ^0.8.0 | **^0.8.7** | Patch only — this whole package is **deprecated** (see Tier B); pin to the latest 0.8.x to stop minor-range drift until we migrate. |
| `react` / `react-dom` | ^18.2.0 | **^18.3.1** | Last 18.x — no API changes for us; pre-stages the React 19 migration. |
| `@types/react` / `@types/react-dom` | ^18.2.0 | **^18.3.27 / ^18.3.7** | Match the runtime. |
| `@types/node` | ^20.11.0 | **^20.19.20** | Stays on the Node-20 line that matches `engines.node`. |
| `zod` | ^3.24.0 | **^3.25.76** | Within-3.x. Zod 4 is deferred (Tier B). |
| `tailwindcss` | ^3.4.1 | **^3.4.18** | Within-3.x. Tailwind 4 is deferred (Tier B). |
| `eslint` | ^8.56.0 | **^8.57.1** | Last 8.x (8 is EOL — see Tier B). |
| `eslint-config-next` | 15.5.15 | **15.5.18** | Match `next` exactly (always 1:1). |
| `typescript` | ^5.3.3 | **^5.9.3** | Within-5.x. TS 6 is deferred (Tier B). |
| `autoprefixer` | ^10.4.16 | **^10.5.0** | Minor. |
| `postcss` | ^8.5.10 | **^8.5.15** | Patch. (Note: `npm audit` still flags a postcss advisory in Next 15's bundled deps — only fixed by Next 16.) |

### What this gets us

- ✅ Backend: `cargo deny` ignore-list cut in half. `cargo machete` clean. `make agent-verify` green end-to-end.
- ✅ Frontend: latest within the current majors. ESLint, React, Next, Zod, Tailwind, TypeScript stage for the major migrations below without doing them yet.
- ✅ No deprecated *first-party* code paths introduced; the one deprecated dep we still ship (`@supabase/auth-helpers-nextjs`) is pinned to its last patch and is on the Tier-B list to be replaced.

---

## Tier B — deferred (major version, real migration cost)

Each row is a candidate for its own focused PR. Ordering is recommended; see "Suggested sequence" below.

| # | Migration | Cost | Risk | Why it matters in <1yr |
|---|---|---|---|---|
| B1 | `@supabase/auth-helpers-nextjs` → `@supabase/ssr` | Small (15-line file: `frontend/app/auth/callback/route.ts`) | Low | Already deprecated by Supabase. Will rot. |
| B2 | ESLint 8 → 9 + `@typescript-eslint` 6 → 8 + flat config | Medium (rewrite `.eslintrc.json` as `eslint.config.js`; ESLint 9 only accepts flat config) | Medium | ESLint 8 EOL Oct 2024. We are already 8 months past EOL; 6 high-severity `npm audit` findings in `@typescript-eslint` 6.x. |
| B3 | Next 15 → 16 (paired with `eslint-config-next` 15 → 16) | Medium (Next 16 deprecated `next.config.js` defaults around caching/dynamic-IO; the codebase only uses defaults so should be light, but verify the OAuth callback route) | Medium | Next 15 hits EOL when 17 ships (~Oct 2026). Also fixes the postcss advisory transitively. |
| B4 | React 18 → 19 (+ types) | Medium (no breaking change in our usage that I can see; verify `LoginForm`, `Dashboard`, `TextGenerator` and the new ref-as-prop) | Low–medium | RSC actions, `use()`, improved transitions are genuine wins for any Next 16 work. |
| B5 | Zod 3 → 4 | Small–medium (renames at `frontend/lib/api-types.ts`; `.merge()` → `.extend()`, error format change) | Medium | Smaller bundle, better TS inference perf; Zod 3 will be supported but bug-fix-only. |
| B6 | Tailwind 3 → 4 | Medium–large (CSS-first config: rewrite `tailwind.config.js` as inline `@theme` in `globals.css`; PostCSS plugin chain change) | Medium | Tailwind 4 is the active line; 3.x is in maintenance. Faster compiler is a real DX win on a template you fork a lot. |
| B7 | TypeScript 5 → 6 | Small (most code is already strict) | Low–medium | TS 6 just shipped (~weeks old). Wait one minor for ecosystem catch-up — `@typescript-eslint` 8 needs to officially support it. Do this *after* B2. |
| B8 | `validator` → `garde` (Rust) | Medium (rewrite request struct annotations on `models/auth.rs`, `models/vectordb.rs`, `models/llm.rs`) | Low | Drops the `proc-macro-error2` advisory ignore. Pure win once you accept the API change. |
| B9 | `qdrant-client` upstream fix to drop `rustls-pemfile` | Watch upstream | Low | Not actionable on our side until qdrant-client migrates to `rustls-pki-types`. The advisory ignore in `deny.toml` calls this out; the trigger to remove it is documented. |

### Suggested sequence (one PR each, in this order)

1. **B1 — `@supabase/ssr` migration.** Smallest, biggest deprecation signal, no other deps depend on it.
2. **B2 — ESLint 9 + ts-eslint 8 + flat config.** Closes 6 high-severity `npm audit` findings. Required for B3 (Next 16's `eslint-config-next` expects flat config).
3. **B3 — Next 16 + eslint-config-next 16.** Closes the remaining postcss advisory. Pair with B4 since they share types.
4. **B4 — React 19.** Pair with B3.
5. **B7 — TypeScript 6.** Now ts-eslint 8 supports it.
6. **B5 — Zod 4.** Touches contract types; do after the framework upgrades are stable so the diff is small.
7. **B6 — Tailwind 4.** Touches styling but not logic; safe to do whenever.
8. **B8 — `garde` Rust migration.** Independent of frontend.
9. **B9 — qdrant-client / rustls-pemfile.** Watch upstream; no action today.

Doing the first three back-to-back wipes every `npm audit` finding the current install has. The rest is shiny.

---

## Tier C — verified-still-active (no action, just confirmed not abandoned)

| Crate / package | Last meaningful release | Active? | Replacement if not |
|---|---|---|---|
| `dotenvy` 0.15 | maintained as the canonical `dotenv` successor | ✅ | n/a — original `dotenv` is the unmaintained one this replaced |
| `urlencoding` 2.1 | small, stable, no churn expected | ✅ | n/a |
| `async-trait` 0.1 | Rust-team maintained; will be deprecated once native `async fn` in traits covers our use case fully | ✅ | drop entirely once we move to native AFIT (Rust 1.75+ supports it; the limitation is dyn-trait object safety which 1.75 + RTN-syntax addresses) |
| `tower` 0.5 | actively maintained, current major | ✅ | n/a |
| `chrono`, `jsonwebtoken` | **removed** in this PR series (caught by `cargo machete` after the `clippy::disallowed_methods` work) | n/a | n/a |
| `cargo-deny`, `cargo-machete` | actively maintained Embark Studios tooling | ✅ | n/a |

---

## Tooling & runtime

| Item | Current | Latest | Note |
|---|---|---|---|
| Rust toolchain (channel) | `stable` (resolves to 1.96.0 on the CI VM) | 1.96.0 | `rust-toolchain.toml` floats with stable. ✅ |
| `rust-version` (MSRV) | 1.85 | n/a — this is a *minimum*, not a target | Keeps the template compatible with older installations. ✅ |
| Node.js engines | `>=20.9.0` | LTS = 22 | Bump to `>=22.0.0` when comfortable; 20 LTS active maintenance ends Apr 2026 and is *security-only* now. **Update worth scheduling soon.** |
| npm engines | `>=10.0.0` | 11.17.0 | Auto-resolved by the Node bump. |
| GitHub Actions (`actions/checkout` @ `v4.3.1`, `actions/setup-node` @ `v4.4.0`, `actions/cache` @ `v4.3.0`) | SHA-pinned in this PR | currently latest | Workflow comment documents the bump procedure. The runners' Node-20 → Node-24 forced upgrade is a separate GH-side concern; will require these actions to ship Node-24-native releases. |
| `cargo-deny` (in CI) | 0.19.9 | 0.19.9 | ✅ |
| `cargo-machete` (in CI) | 0.9.2 | 0.9.2 | ✅ |

---

## Standing rules for keeping this current

Adopt as part of `AGENTS.md` / `.cursor/rules/` (Tier-4 guardrail):

1. **No new dependency without a `Cargo.toml` / `package.json` comment** explaining why and when it should be revisited.
2. **`cargo update` runs on every PR** (it's part of `make agent-verify` indirectly via the build) — drift is visible in PRs immediately.
3. **`npm audit` findings are PR-blocking** unless explicitly waived in the PR description with rationale.
4. **Tier-B migrations are scheduled, not improvised** — pick from the list above and the next agent does *only that one*.
5. **The audit doc gets refreshed quarterly.** Last refresh: 2026-06-21. Next: 2026-09-21 (or sooner if a Tier-B migration ships).

