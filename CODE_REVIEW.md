# Code Review — Reporting to Uncle Bob

> **Status update (later than the original audit):** the analysis below still stands and remains the evidence base. The *action* plan at the bottom of this document ("Optimization Plan") has been **superseded by [`ARCHITECTURE_PLAN.md`](./ARCHITECTURE_PLAN.md)**, which targets lead-developer-quality Clean Architecture expressed as microservices and **preserves every feature** (including the currently-unused Supabase database/storage code, which becomes the engine of a new `documents-service`). Read the analysis here for the *why*; read `ARCHITECTURE_PLAN.md` for the *what we are actually going to do*.


> Audit of the **Full Stack Vibe Coding Template** (Next.js frontend + Rust Axum backend + Supabase + Qdrant), evaluated through the lens of *Clean Code*, *Clean Architecture*, and the **SOLID** principles.
>
> Audit scope: working tree at `main` (`942d8ed`), `backend/src/**` (4,155 LOC of Rust), `frontend/**` (1,071 LOC of TS/TSX), `supabase/migrations/`.

---

## **Required Inputs (as adapted for this repo)**

### IMPLEMENTATION_PLAN

Per `README.md`, `CHANGELOG.md`, and recent git history, the implementation plan was:

1. Provide a reusable boilerplate ("vibe coding template") with Next.js 15 frontend, Rust Axum 0.8 backend, Supabase auth/DB/storage, and Qdrant vector search.
2. Migrate the original Python/FastAPI backend to Rust (PRs #14, #28, #34 — `cursor/backend-language-migration-33ab`).
3. Layer a *clean architecture* slice on top of the Rust backend for auth (PR #36 — `cursor/auth-clean-architecture-slice-f9a4`) introducing `domain/`, `application/`, `infrastructure/` modules.
4. Enforce guardrails: `clippy::unwrap_used = deny`, `clippy::expect_used = deny`, `unsafe_code = forbid`, request validation with `validator`, structured tracing, request-id middleware.
5. Provide cloud-agent-friendly toolchain (`backend/rust-toolchain.toml`, `scripts/verify-agent-toolchain.sh`).

### TECHNICAL_SPECIFICATION

Derived from `AGENTS.md`, `CODE_STANDARDS.md`, `.cursor/rules/`:

- **Backend (Rust 2024, MSRV 1.85):** Axum 0.8 with `tower-http` (cors, trace, request-id). `reqwest` (rustls-tls) for outbound HTTP, `async-openai` for OpenAI, raw `reqwest` for Anthropic, `qdrant-client 1.x` for vector DB, `jsonwebtoken` (declared but unused — see analysis), `validator` for input validation, `thiserror` for error types, `tracing` for logs.
- **Frontend (Next.js 15, React 18, TS 5):** App Router, `@supabase/supabase-js` for auth/storage from the browser, `@supabase/auth-helpers-nextjs` for the OAuth callback route, Tailwind for styling, `zod` for runtime validation of API boundaries.
- **API surface:** `GET /`, `GET /api/auth/me`, `POST /api/auth/provider-token`, `POST /api/llm/{generate,embedding}`, `POST/DELETE /api/vectordb/documents`, `POST /api/vectordb/search`.
- **Error contract:** `{ "error": "..." }` JSON body, HTTP status from `AppError::status_code()` (4xx → WARN log, 5xx → ERROR log).
- **Auth contract:** Bearer JWT issued by Supabase Auth; backend verifies by calling `GET {SUPABASE_URL}/auth/v1/user` with the service key.
- **Authz contract:** Qdrant documents tagged with `_owner_user_id` payload key, enforced server-side in every read/write filter.

### PROJECT_REQUEST

From `README.md`: "Don't waste your time and tokens on boilerplate code. Use it to build your app." — i.e. a batteries-included, AI-agent-friendly template with maximum compiler-enforced guardrails, suitable to clone and immediately ship a Supabase + LLM + vector-search SaaS on top of.

### PROJECT_RULES

From `AGENTS.md` and `.cursor/rules/ai-guidelines.mdc` (the rules Uncle Bob would actually read):

- Rust: no `unwrap`/`expect` outside tests/main (already enforced by clippy lints); `Result<T, AppError>` everywhere; doc comments on every public item; `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` clean before commit.
- TS: no `any`, no `// @ts-ignore`, typed props on every component, error + loading states everywhere; `npm run lint` + `npm run build` clean before commit.
- API: validate every body, authenticate every protected endpoint, log every important operation with context, return typed errors.
- DB: every table has RLS, every query has an index, every migration is reversible.

### EXISTING_CODE

Audited in place. Notable cross-cutting observations are captured under `<analysis>` below; per-file recommendations live in the Optimization Plan.

---

<analysis>

# Uncle Bob's Audit

> "The only way to make the deadline — the only way to go fast — is to keep the code as clean as possible at all times." — *Robert C. Martin, Clean Code*

## 0. Headline

This is a *good* template — clear domain boundaries, real validation, real tests, real tracing, real lint deny-lists — but it's **half-migrated**. A "clean architecture" slice was bolted onto auth (`domain/`, `application/`, `infrastructure/`), yet the *old* service-layer code (`services/supabase/{auth,database,storage}.rs`) was never deleted, and the new and old layers now coexist with **duplicate models, duplicate bearer-token parsers, and dead public APIs**. The frontend, by contrast, is honest about its layering but leaks small inconsistencies (deprecated Supabase auth-helpers, a misnamed mode-switch copy, a Docker hostname hard-coded into a Next route handler that runs natively in CI).

Below: three lenses — **Structure / Quality / UI-UX** — followed by an atomic, dependency-ordered Optimization Plan.

---

## 1. Code Organization & Structure

### 1.1 Clean Architecture half-migration (the biggest finding)

PR #36 introduced the textbook layout for auth:

```
backend/src/
├── domain/auth.rs              # AuthGateway trait + AuthenticatedUser entity + AuthDomainError
├── application/auth.rs         # AuthUseCase orchestration
├── infrastructure/supabase/    # SupabaseAuthGateway impl + SupabaseUserRecord
└── api/                        # Axum handlers + AppState + map_auth_error
```

That is *exactly* what Uncle Bob draws. The arrows point inward, the gateway is a port, the use-case is the interactor, and the Axum handler is the controller. Good.

The problem is that `backend/src/services/supabase/auth.rs` (`SupabaseAuthService`) — the **previous-generation** implementation — was left intact, with its own copy of:

- `SupabaseUser` / `UserMetadata` structs (duplicating `infrastructure::supabase::models::SupabaseUserRecord` / `SupabaseUserMetadata`).
- `get_user(jwt_token) -> UserProfile` (duplicating `SupabaseAuthGateway::get_user_from_bearer_token`).
- `sign_in_with_provider_token` (duplicating `SupabaseAuthGateway::exchange_provider_token`).
- `extract_bearer_token` (duplicating `http_auth::bearer_token_from_value`).

Grepping for usage:

```
SupabaseAuthService     → defined + re-exported, never instantiated outside its own module
SupabaseDatabaseService → defined + re-exported, never instantiated anywhere
SupabaseStorageService  → defined + re-exported, never instantiated anywhere
```

That is roughly **780 LOC of dead code** (`services/supabase/auth.rs` 164, `database.rs` 302, `storage.rs` 315) plus the redundant model duplicates. By the *Common Closure Principle* and *DRY*, this is the single largest hit to maintainability in the repo. New contributors will copy the wrong pattern, and CodeRabbit-style review bots will keep flagging both copies.

**Verdict:** Either delete `services/supabase/` (and the duplicated models) or — if the intent was that future db/storage gateways migrate the same way — move them under `infrastructure/supabase/` as `database_gateway.rs` / `storage_gateway.rs` *after* introducing `DbGateway` / `StorageGateway` ports in `domain/`. Do **not** leave both shapes living side-by-side.

### 1.2 SRP violations in `AppState::try_new`

`api/state.rs` does three things in one function: build the Qdrant client, build the shared `reqwest::Client`, *and* compose the `SupabaseAuthGateway` + `AuthUseCase`. The clean-architecture intent is that *composition* lives at the edge — `main.rs` — and `AppState` is just a struct of shared handles. Today the wiring lives in `AppState`, the entrypoint is in `app::run`, and the binary in `main.rs` is a one-liner. Acceptable, but the wiring should be split into a tiny `composition.rs` (`build_auth_use_case(client) -> AuthUseCase`) so that integration tests can compose with a mock gateway by swapping one helper.

### 1.3 Per-request factories for LLM / embedding services

`LlmServiceFactory::get_service` and `EmbeddingServiceFactory::get_service` are called **inside every request handler** (`api/llm.rs`, `api/vectordb.rs`). Each call:

- builds a brand-new `reqwest::Client` (for Anthropic and for OpenAI embeddings), or
- builds a brand-new `async_openai::Client` (for OpenAI text generation).

`reqwest::Client` owns its connection pool. Recreating it per request defeats keep-alive and inflates TLS handshake count under load. This is also a *Dependency Inversion* miss: handlers know how to construct dependencies instead of receiving them through `AppState`. Move both factories' outputs onto `AppState` (`Arc<dyn LlmServiceRegistry>`, `Arc<dyn EmbeddingServiceRegistry>`) and select the provider by enum at call time using the **already-shared** `reqwest::Client`.

### 1.4 Two error-mapping islands

`api/auth_handler.rs` has `map_auth_error(AuthDomainError) -> AppError`. That's the right layer (an *adapter*). But error mapping for LLM / Qdrant / Supabase database errors lives directly inside each service via `AppError::ExternalService(format!(...))`. Once we add `LlmService` and `VectorDb` gateway ports (1.3 above), each domain error type should land in `domain/` and `api/` should own a single error-translation table. Otherwise transport leaks into the domain (e.g. `qdrant.rs` already returns `AppError`, which is an *Axum* error type — that's a layering smell flagged by *Dependency Rule*).

### 1.5 `models/` mixes API DTOs and DB rows

`models/auth.rs` holds:

- `UserProfile` — API response DTO ✅
- `TokenResponse` — API response DTO ✅
- `ProviderTokenRequest` — API request DTO + validator ✅
- `SupabaseUser` + `UserMetadata` — *upstream* Supabase wire format ❌ (duplicates the same thing in `infrastructure/supabase/models.rs`)

The `SupabaseUser` family belongs strictly behind the gateway. Re-exporting it from `models::` invites handlers to depend on Supabase shape, which is exactly what the clean-architecture refactor was supposed to prevent.

### 1.6 `frontend/app/api/health/route.ts` hard-codes Docker hostname

```ts
const apiUrl = 'http://backend:8000';
```

This route only resolves inside `docker-compose`. In native runs (and CI) it logs `getaddrinfo ENOTFOUND backend` on every build (AGENTS.md already documents this as "expected"). Documenting a bug as expected is a *Boy Scout Rule* failure — fix it. Read `NEXT_PUBLIC_API_URL` (already in the env contract) and fall back to `http://localhost:8000`.

### 1.7 Frontend service layer is thin and correct

`services/llm.ts`, `services/supabase.ts`, and `lib/api-types.ts` get the boundary right: zod-validated request *and* response, single `apiRequestJson` helper, single `getAuthToken` source. The `getApiUrl()` SSR branch correctly distinguishes browser vs server (same Docker-hostname caveat as 1.6).

---

## 2. Code Quality & Best Practices

### 2.1 Dead code is dead weight

In addition to the three unused services (1.1), the following are unused:

- `EmbeddingServiceFactory::get_default()` — declared, never called.
- `LlmConfig::has_openai()` / `has_anthropic()` — declared, never called.
- `jsonwebtoken` dependency in `Cargo.toml` — declared, never imported (the backend doesn't actually verify JWT signatures locally; it delegates to Supabase via HTTP). Either implement local JWT verification with `SUPABASE_JWT_SECRET` (already plumbed in `SupabaseConfig`) and drop the per-request `GET /auth/v1/user` round-trip, or remove the dependency.

### 2.2 Sensitive error detail leakage in 5xx bodies

`AppError::into_response` serializes `self.to_string()` directly into the JSON body:

```rust
ApiErrorResponse::new(status_code, error_message).into_response()
```

For 4xx that's fine (the validator messages, "Missing Authorization header", etc. are safe). For 5xx the same body now contains:

- `"External service error: HTTP error (status 503): ...; url=https://xxx.supabase.co/auth/v1/user"` — leaks the upstream URL.
- `"Configuration error: Failed to create HTTP client: ..."` — leaks library internals.
- `"Internal server error: JSON error: missing field 'id' at line 3 column 7"` — leaks payload shape.

Uncle Bob's *Boundary* chapter is unambiguous: information that crosses an architectural boundary must be intentional. Today the API leaks debug strings on every 5xx. Replace the 5xx body with a generic message + the existing `x-request-id`, and keep the raw `error_message` in the `tracing::error!` log only. Add a `correlation_id` field to `ApiErrorResponse` for client-side reporting.

### 2.3 `#[serde(skip)] StatusCode` is a footgun

`ApiErrorResponse` derives `Deserialize` and has `pub status_code: StatusCode` marked `#[serde(skip)]`. `StatusCode` is not `Default`, so deserialization of `ApiErrorResponse` will fail at compile time the moment anyone tries. Either drop `Deserialize` (it's never deserialized server-side) or wrap status in `#[serde(skip, default = "default_status")]`. Currently it compiles only because nothing exercises that path.

### 2.4 `unwrap_or_default()` masking a real branch

In `qdrant.rs::add_documents`:

```rust
serde_json::to_string(&doc_content).unwrap_or_default()
```

This *cannot* fail for a `HashMap<String, JsonValue>` of strings — but if it ever does (e.g., metadata is extended to include non-string-serializable types), you silently store `""` in the document payload and lose the document text. Use `serde_json::to_string(&doc_content).map_err(|e| AppError::Internal(format!("...: {e}")))?` — *Fail Fast* (Clean Code, ch. 7).

### 2.5 LLM/embedding wire types redefined in every method

`text_generation.rs::AnthropicService::generate_text` declares `AnthropicRequest` / `AnthropicResponse` / `ContentBlock` / `AnthropicUsage` inside the function body. Same pattern in `embedding.rs::create_embedding`. That works, but the types are part of the *vendor contract*; lifting them to `services/llm/anthropic_types.rs` and `services/llm/openai_embedding_types.rs` makes them documentable, testable in isolation, and reusable when streaming endpoints are added (already in `FutureImprovements.md`).

### 2.6 Doc comments mostly present, but a few public items lack them

`AppError` variants, `ApiErrorResponse` fields, and `DocumentData` all carry rustdoc. ✅. But several public functions in `app.rs` (`init_tracing`) and `qdrant.rs` (`extract_string_value`, `qdrant_payload_string_from_json`) are crate-private without comments — fine — yet `DocumentData` is `pub` and re-exported via `services::vectordb::qdrant::DocumentData`, used in `api/vectordb.rs::add_documents` as part of the conversion layer. It should either be private to `qdrant.rs` (it's a *gateway* DTO) or documented as such.

### 2.7 Test coverage is thin and uneven

- `backend/tests/api_errors.rs` (180 LOC) exercises **only auth-failure / 404 / 405** paths. There are zero tests for happy-path LLM/vectordb behavior (which would require mocking external services).
- `qdrant.rs` has good unit tests for filter construction (`scoped_filter`, `scoped_delete_filter`) — bravo, this is exactly the right surface to test.
- `auth_gateway.rs` only tests the `status -> AuthDomainError` mapper. It does **not** test the JSON parsing path. With a `MockAuthGateway` (already buildable because `AuthGateway: Send + Sync`), the `AuthUseCase` could be tested in isolation — and currently isn't.
- Frontend has **no** automated tests at all.

This is the single biggest *Three Laws of TDD* gap. The infrastructure is there; the tests aren't.

### 2.8 Logging hygiene is good, with one nit

`tracing::info!` calls universally include `user_id`, `model`, `provider`, `document_count` etc. as structured fields — exemplary. The one nit: `api/logging.rs::truncate_for_log` is used inconsistently (only for `prompt` and `query_text`). The Anthropic / OpenAI services log nothing at the outbound boundary. Add a `tracing::debug!` at the `client.post(...)` call with the provider name + model + token-budget so production traces can attribute latency to upstream vendors.

### 2.9 Magic-numberish constants

`api/state.rs` hardcodes `Duration::from_secs(30)` for the shared `reqwest::Client`. Anthropic uses `60s`, embeddings use `60s`, Supabase uses `30s`. These should be named constants in `config/timeouts.rs` (or fields on `Settings`), with env overrides. *Clean Code §17* — "Magic numbers must die."

### 2.10 `LazyLock<Settings>` panic on startup

`SETTINGS` panics on missing env. That's actually defensible (Uncle Bob: "fail at construction"), but combined with `LazyLock`, the panic surfaces on **first request that touches `SETTINGS`**, not at `main()`. Already partially mitigated because `AppState::try_new` accesses `SETTINGS` during boot via `SupabaseAuthGateway::new`, but a `let _ = &*SETTINGS;` in `app::run` before tracing init would make the failure mode obvious.

---

## 3. UI / UX

### 3.1 Login form mode copy is wrong

`LoginForm.tsx`:

```tsx
<p>
  {mode === 'signin' ? 'Or create a new account'
    : mode === 'signup' ? 'Or sign in to your account'
    : 'Enter your email to receive a reset link'}
</p>
```

The sub-headline contradicts the headline ("Sign in to your account" / "Or create a new account") — it reads like a hint when really it's a *call to action*, and the click target is buried below the form. Either drop the copy or convert the sub-headline into a link that flips `mode`. Standard sign-in UX (Stripe, Vercel, Linear) puts the toggle inline with the headline.

### 3.2 Mode switcher buttons render as a vertical stack on mobile

The three mode switchers (`Sign in / Create / Forgot`) are each full-width `<button>`s with no `role="group"` or visual grouping. On mobile they look like three primary CTAs. Wrap them in a single bar (`<div role="group">`) and downgrade two of them to text links.

### 3.3 Loading text instead of spinners on auth buttons

All three OAuth/email buttons swap to a literal `"Loading..."` string. Match the dashboard spinner pattern (`animate-spin rounded-full ... border-b-2`) for visual consistency.

### 3.4 No client-side validation feedback

`LoginForm` and `ResetPassword` rely entirely on `<input required minLength={8} />` + server error messaging. Add inline feedback for invalid email format and password strength so users don't round-trip to Supabase to learn their password is too short.

### 3.5 Error region is not announced to assistive tech

```tsx
{error && (<div className="...bg-red-100...">{error}</div>)}
```

This `<div>` appears dynamically with no `role="alert"` / `aria-live="polite"`. Screen-reader users won't hear that login just failed. Same for the success div. Two-line fix.

### 3.6 Color contrast on `text-gray-500` placeholders

Tailwind's `text-gray-500` on `bg-white` is ~4.0:1 — below WCAG AA for normal text (4.5:1). Bump to `text-gray-600` for body copy in `LoginForm`, `ResetPassword`, `TextGenerator`, and the homepage `FeatureCard` description.

### 3.7 `TextGenerator` model list lags reality

Hardcoded options: `gpt-3.5-turbo`, `gpt-4`, `claude-3-sonnet-20240229`, `claude-3-opus-20240229`. The form's default model (`settings.model = 'gpt-4o-mini'`) isn't in the dropdown — opening the form, then changing provider, then switching back will silently drop the user to `gpt-3.5-turbo`. Drive the dropdown from a single `MODEL_CATALOG` constant shared between provider state and option lists.

### 3.8 Response card lacks copy-to-clipboard / download

The generated text shows in a `whitespace-pre-wrap` div with no copy button. Adding one is ~10 lines, materially improves UX, and is the kind of obvious affordance Uncle Bob would call out as *user empathy*.

### 3.9 No global error boundary

`frontend/app/layout.tsx` has neither an `<ErrorBoundary>` nor a `not-found.tsx`. A thrown render error today crashes the page silently in production. Next 15 supports `error.tsx` and `not-found.tsx` per route segment — add at least the root pair.

### 3.10 Deprecated `@supabase/auth-helpers-nextjs` in callback

`frontend/app/auth/callback/route.ts` imports `createRouteHandlerClient` from `@supabase/auth-helpers-nextjs@0.8`. That package is deprecated by Supabase in favor of `@supabase/ssr`. Not a *current* bug, but it'll bit-rot. Migrate while it's a 15-line change.

---

## 4. Strategy

If we ranked the findings by **impact / effort** Uncle-Bob-style:

| # | Finding | Severity | Effort |
|---|---------|----------|--------|
| 1.1 | Dead `services/supabase/*` (780 LOC) + duplicate models | **High** | Low |
| 2.2 | 5xx error body leaks upstream URLs + internals | **High** | Low |
| 1.3 | Per-request HTTP client + service construction | **High** | Medium |
| 2.7 | Almost no happy-path tests | **High** | Medium |
| 1.6 / 3.10 | Hard-coded Docker host + deprecated auth-helpers | Medium | Low |
| 2.1 | Unused `jsonwebtoken` / `has_openai()` / `get_default()` | Medium | Low |
| 3.5 / 3.6 | A11y (aria-live, contrast) | Medium | Low |
| 1.5 | `models/` leaks Supabase shape | Medium | Low |
| 2.5 / 2.9 | Vendor types redeclared + magic timeouts | Low | Low |
| 3.1–3.4, 3.7–3.9 | UX polish in `LoginForm` and `TextGenerator` | Low | Low |

</analysis>

---

# Optimization Plan

> Atomic, dependency-ordered. Each step touches **≤ 20 files**, preserves behavior unless explicitly noted, and ends with concrete success criteria a reviewer (or the next agent) can check off.

## Code Structure & Organization

- [ ] **Step 1: Delete the dead `services/supabase/*` layer and consolidate Supabase types behind the gateway**
  - **Task**: Remove `SupabaseAuthService`, `SupabaseDatabaseService`, `SupabaseStorageService`, and the duplicate `SupabaseUser` / `UserMetadata` / `extract_bearer_token`. Promote `infrastructure::supabase::models::SupabaseUserRecord` as the single Supabase wire model.
  - **Files**:
    - `backend/src/services/supabase/auth.rs`: **delete**.
    - `backend/src/services/supabase/database.rs`: **delete** (or stage under a `wip/` branch if the team plans a future `DbGateway` port — but do not leave it on `main`).
    - `backend/src/services/supabase/storage.rs`: **delete**.
    - `backend/src/services/supabase/mod.rs`: **delete** (and drop `pub mod supabase;` from `services/mod.rs`).
    - `backend/src/services/mod.rs`: remove `pub mod supabase;`.
    - `backend/src/models/auth.rs`: remove `SupabaseUser` + `UserMetadata` + `impl From<SupabaseUser> for UserProfile`.
    - `backend/src/models/mod.rs`: drop `SupabaseUser, UserMetadata` from the re-export list.
    - `backend/src/http_auth.rs`: confirm it remains the sole bearer-parser; no change needed beyond removing the stale doc reference to `SupabaseAuthService::get_user`.
  - **Step Dependencies**: None.
  - **User Instructions**: Run `cargo check && cargo clippy --all-targets -- -D warnings && cargo test`. Expect a smaller binary and zero behavior change.
  - **Success Criteria**: `rg SupabaseAuthService backend/` returns 0 hits. `cargo clippy` passes without dead-code warnings. Integration tests still green.

- [ ] **Step 2: Drop the unused `jsonwebtoken` dependency** *(or wire it up for local JWT verification — pick one)*
  - **Task**: Decide: either (a) implement local JWT verification using `SUPABASE_JWT_SECRET` to skip the per-request `GET /auth/v1/user` round-trip (lower latency, fewer external calls), or (b) remove the dependency entirely. Default to (b) until benchmark data justifies (a).
  - **Files**:
    - `backend/Cargo.toml`: drop `jsonwebtoken = "10"`.
    - `backend/Cargo.lock`: regenerated.
  - **Step Dependencies**: None.
  - **User Instructions**: `cargo update -p jsonwebtoken --precise 0.0.0` is not needed; just `cargo build`.
  - **Success Criteria**: `rg jsonwebtoken backend/` returns 0 hits in `src/`. Build is smaller.

- [ ] **Step 3: Promote LLM/embedding service construction to `AppState` (share the `reqwest::Client`)**
  - **Task**: Today every request calls `LlmServiceFactory::get_service` / `EmbeddingServiceFactory::get_service`, which constructs a fresh `reqwest::Client`. Construct each provider once at boot, store as `Arc<dyn LlmService>` / `Arc<dyn EmbeddingService>` in `AppState`, and inject them into handlers.
  - **Files**:
    - `backend/src/api/state.rs`: add `pub openai_llm: Option<Arc<dyn LlmService>>`, `pub anthropic_llm: Option<Arc<dyn LlmService>>`, `pub openai_embedding: Option<Arc<dyn EmbeddingService>>`. Build them lazily inside `try_new` using the shared `reqwest_client`.
    - `backend/src/services/llm/text_generation.rs`: change `OpenAiService::new` / `AnthropicService::new` to accept an injected `reqwest::Client` / `OpenAIClient`; keep the existing factory as a thin convenience for tests only.
    - `backend/src/services/llm/embedding.rs`: same — `OpenAiEmbeddingService::new(client: Arc<reqwest::Client>, api_key: String)`.
    - `backend/src/api/llm.rs`: replace `LlmServiceFactory::get_service(...)` with `state.llm_for(request.provider)?` (helper on `AppState` that returns `Result<Arc<dyn LlmService>, AppError>` mapping missing-config to `AppError::Configuration`).
    - `backend/src/api/vectordb.rs`: same for `EmbeddingServiceFactory::get_service`.
  - **Step Dependencies**: Step 1 (cleaner module tree).
  - **User Instructions**: Run `cargo test` plus a manual smoke (`curl /api/llm/generate` with a real key) to confirm connection pooling.
  - **Success Criteria**: `reqwest::Client::builder()` appears exactly **once** in `backend/src/` (in `AppState::try_new`). `async_openai::Client::with_config` appears exactly once. `cargo test` green.

- [ ] **Step 4: Extract composition out of `AppState::try_new` into `app/composition.rs`**
  - **Task**: `AppState::try_new` currently does HTTP-client building, Qdrant connection, *and* auth-stack wiring. Split into pure factory functions so integration tests can compose with a `MockAuthGateway`.
  - **Files**:
    - `backend/src/app.rs`: add `mod composition;` (or new file `backend/src/composition.rs`).
    - `backend/src/composition.rs` (new): expose `pub fn build_shared_http_client() -> Result<Arc<reqwest::Client>, AppRunError>`, `pub fn build_auth_use_case(client: Arc<reqwest::Client>) -> Arc<AuthUseCase>`, `pub async fn build_qdrant() -> Result<Arc<QdrantService>, AppRunError>`.
    - `backend/src/api/state.rs`: `try_new` becomes a 6-line composition that calls those three helpers.
    - `backend/src/lib.rs`: add `pub mod composition;` if Step uses a top-level module.
  - **Step Dependencies**: Steps 1 & 3.
  - **User Instructions**: None.
  - **Success Criteria**: `AppState::try_new` ≤ 15 LOC. New helpers covered by a unit test that swaps in a `MockAuthGateway` via `AuthUseCase::new`.

- [ ] **Step 5: Fix the Docker-hostname leak in `frontend/app/api/health/route.ts`**
  - **Task**: Read `process.env.NEXT_PUBLIC_API_URL` (already documented as the canonical name) with a sensible local fallback. Stop logging `getaddrinfo ENOTFOUND backend` during native builds.
  - **Files**:
    - `frontend/app/api/health/route.ts`: replace `const apiUrl = 'http://backend:8000';` with `const apiUrl = process.env.NEXT_PUBLIC_API_URL ?? process.env.BACKEND_INTERNAL_URL ?? 'http://localhost:8000';`. Remove the bespoke `declare const process` block (Node types already cover it).
    - `AGENTS.md`: remove the "expected `getaddrinfo ENOTFOUND backend`" caveat in the Frontend section.
    - `frontend/Dockerfile` / `docker-compose.yml`: set `BACKEND_INTERNAL_URL=http://backend:8000` for the compose path.
  - **Step Dependencies**: None.
  - **User Instructions**: `cd frontend && npm run build` and confirm no DNS warnings.
  - **Success Criteria**: `npm run build` produces no `ENOTFOUND` warnings locally. `docker compose up` still resolves `http://backend:8000` via the compose-injected env.

## Code Quality & Best Practices

- [ ] **Step 6: Sanitize 5xx error responses and add a correlation id**
  - **Task**: Stop returning raw `self.to_string()` to clients on 5xx. Return a generic message + a `request_id` echoed from the `x-request-id` header; keep the detailed message in the tracing log.
  - **Files**:
    - `backend/src/models/error.rs`: add `pub request_id: Option<String>` to `ApiErrorResponse`. Split `IntoResponse for AppError` into a small helper that maps to a *public* body (generic for 5xx, descriptive for 4xx) plus a `tracing::error!` with the full detail. Drop `Deserialize` from `ApiErrorResponse` (it has no use server-side; client uses zod's `ApiErrorBodySchema`).
    - `backend/src/api/auth_handler.rs` / `backend/src/api/llm.rs` / `backend/src/api/vectordb.rs`: extract `x-request-id` and pass to the response (probably via a small `axum::extract::FromRequestParts`-based helper or `tower_http` propagation already enabled).
    - `frontend/lib/api-types.ts`: extend `ApiErrorBodySchema` with `request_id: z.string().optional()`.
    - `frontend/services/llm.ts::parseJsonErrorMessage`: include the request id in the thrown `Error`'s message if present (`"... [req: abc123]"`).
  - **Step Dependencies**: None.
  - **User Instructions**: Trigger a forced 5xx (e.g. point `SUPABASE_URL` at an invalid host) and verify the response body says "Internal server error" but logs carry the upstream URL.
  - **Success Criteria**: No 5xx response body contains the substring `https://` or `url=`. All 5xx bodies contain a non-empty `request_id` matching the `x-request-id` response header. Logs unchanged.

- [ ] **Step 7: Replace `unwrap_or_default()` swallows with explicit errors**
  - **Task**: Fail fast where today we silently lose data.
  - **Files**:
    - `backend/src/services/vectordb/qdrant.rs`: replace `serde_json::to_string(&doc_content).unwrap_or_default()` with `?`-propagated `AppError::Internal`. Audit the file for any other `unwrap_or_default()` calls on `Result` chains.
  - **Step Dependencies**: None.
  - **User Instructions**: None.
  - **Success Criteria**: `rg "unwrap_or_default" backend/src/services` returns 0 hits.

- [ ] **Step 8: Lift LLM vendor wire types out of method bodies**
  - **Task**: Move Anthropic and OpenAI-embedding wire types to dedicated files; expose them only at crate-private visibility.
  - **Files**:
    - `backend/src/services/llm/anthropic_types.rs` (new): `pub(super) struct AnthropicRequest<'a>`, `AnthropicResponse`, `ContentBlock`, `AnthropicUsage`.
    - `backend/src/services/llm/openai_embedding_types.rs` (new): the equivalent for OpenAI embeddings.
    - `backend/src/services/llm/text_generation.rs`: import from the new module; drop inline definitions.
    - `backend/src/services/llm/embedding.rs`: same.
    - `backend/src/services/llm/mod.rs`: add `mod anthropic_types; mod openai_embedding_types;`.
  - **Step Dependencies**: Step 3 (these files will be next to the new injectable services).
  - **User Instructions**: None.
  - **Success Criteria**: Each provider has exactly one file per concern; methods bodies contain only orchestration, not type defs.

- [ ] **Step 9: Centralize HTTP timeouts in `Settings`**
  - **Task**: Promote the magic `Duration::from_secs(30)` / `60` values to `Settings::http_timeouts` with env overrides (`HTTP_TIMEOUT_SUPABASE_SECS`, `HTTP_TIMEOUT_LLM_SECS`, `HTTP_TIMEOUT_EMBEDDING_SECS`), with the current values as defaults.
  - **Files**:
    - `backend/src/config/mod.rs`: add `pub struct HttpTimeouts { pub supabase: Duration, pub llm: Duration, pub embedding: Duration, }` to `Settings`, with parser + `Debug`.
    - `backend/src/api/state.rs`: use `SETTINGS.http.timeouts.supabase` in the shared client (or per-service timeouts on the request, since one client serves all).
    - `backend/src/services/llm/text_generation.rs` + `embedding.rs`: drop `Client::builder().timeout(...)` (after Step 3 they take the shared client) and replace with `.timeout(SETTINGS.http.timeouts.llm)` on the per-call `.post(...)`.
    - `.env.example`: document the three new vars.
  - **Step Dependencies**: Step 3.
  - **User Instructions**: None.
  - **Success Criteria**: `rg "Duration::from_secs" backend/src` returns hits only inside `config/`.

- [ ] **Step 10: Add a happy-path integration test with a `MockAuthGateway`**
  - **Task**: Currently `tests/api_errors.rs` only covers 4xx. Add `tests/api_authenticated.rs` that builds the router with a mock `AuthGateway` returning a canned `AuthenticatedUser`, then hits `GET /api/auth/me` and asserts the 200 body matches the canned user.
  - **Files**:
    - `backend/src/composition.rs` (from Step 4): expose a `pub fn build_app_with_auth_use_case(auth: Arc<AuthUseCase>) -> Router<()>` helper for tests.
    - `backend/tests/api_authenticated.rs` (new): ~80 LOC.
    - `backend/Cargo.toml` (`[dev-dependencies]`): no new deps; `async-trait` already pulled in.
  - **Step Dependencies**: Step 4.
  - **User Instructions**: None.
  - **Success Criteria**: `cargo test --test api_authenticated` passes with `SUPABASE_URL` and `SUPABASE_SERVICE_KEY` set to anything (mock bypasses upstream).

## UI / UX

- [ ] **Step 11: Accessibility pass on auth + LLM screens**
  - **Task**: Add `role="alert"` + `aria-live="polite"` to error / success banners. Replace `text-gray-500` with `text-gray-600` for body copy where contrast is sub-4.5:1. Add `aria-busy` to loading buttons. Swap literal `"Loading..."` for the dashboard spinner.
  - **Files**:
    - `frontend/components/auth/LoginForm.tsx`
    - `frontend/components/llm/TextGenerator.tsx`
    - `frontend/app/auth/reset-password/page.tsx`
    - `frontend/app/page.tsx` (FeatureCard description gray bump)
  - **Step Dependencies**: None.
  - **User Instructions**: Test with VoiceOver / NVDA on the login flow; confirm error toasts are announced.
  - **Success Criteria**: All dynamic error/success regions have `role="alert"`. Lighthouse a11y score ≥ 95 on `/dashboard`.

- [ ] **Step 12: Add `error.tsx`, `not-found.tsx`, and global ErrorBoundary**
  - **Task**: Per Next 15 app-router conventions, add root error + 404 segments. Surface request id (from Step 6) when an API call throws.
  - **Files**:
    - `frontend/app/error.tsx` (new): client component, renders the error + a "Try again" reset button + the request id.
    - `frontend/app/not-found.tsx` (new): branded 404.
    - `frontend/app/layout.tsx`: no change required (Next picks them up by convention).
  - **Step Dependencies**: Step 6 (for the request-id surfacing).
  - **User Instructions**: Visit `/does-not-exist`. Throw an error in `Dashboard` to verify the boundary renders.
  - **Success Criteria**: 404 page renders for unknown routes. Forced render error renders `error.tsx` rather than the white-screen-of-death.

- [ ] **Step 13: Unify model catalog + fix `TextGenerator` default**
  - **Task**: Single `MODEL_CATALOG: Record<LlmProvider, { id: string; label: string }[]>` constant drives both the dropdown options *and* the default model when the provider changes.
  - **Files**:
    - `frontend/lib/api-types.ts`: export `MODEL_CATALOG` and the per-provider default.
    - `frontend/components/llm/TextGenerator.tsx`: replace inline `<option>` lists with `MODEL_CATALOG[settings.provider].map(...)`. On provider change, reset `settings.model` to `MODEL_CATALOG[provider][0].id`.
  - **Step Dependencies**: None.
  - **User Instructions**: Switch provider back-and-forth and verify the model dropdown always shows a valid selection.
  - **Success Criteria**: It is impossible to submit a model not in the dropdown.

- [ ] **Step 14: Polish `LoginForm` mode UX + add copy-to-clipboard on LLM output**
  - **Task**: Replace the contradictory sub-headline with a single inline "Sign in / Create account / Forgot password?" toggle row. Add a copy button on `TextGenerator`'s response card.
  - **Files**:
    - `frontend/components/auth/LoginForm.tsx`: restructure the header + mode-switch section; group the three mode toggles in a `<nav role="tablist">`.
    - `frontend/components/llm/TextGenerator.tsx`: add a `<button>` that calls `navigator.clipboard.writeText(response.text)` with a transient "Copied" state.
  - **Step Dependencies**: Step 11 (a11y baseline).
  - **User Instructions**: None.
  - **Success Criteria**: The login screen no longer shows a "hint" that contradicts the headline. The response card has a working copy button.

- [ ] **Step 15: Migrate the OAuth callback to `@supabase/ssr`**
  - **Task**: Replace `@supabase/auth-helpers-nextjs`'s `createRouteHandlerClient` with `createServerClient` from `@supabase/ssr`. Update `package.json`, lockfile, and the callback handler.
  - **Files**:
    - `frontend/package.json`: drop `@supabase/auth-helpers-nextjs`, add `@supabase/ssr`.
    - `frontend/app/auth/callback/route.ts`: use `createServerClient({ cookies })` from `@supabase/ssr`.
    - `AuthSetup.md`: update docs.
  - **Step Dependencies**: None.
  - **User Instructions**: Run `npm install`. Manually test the Google OAuth round-trip.
  - **Success Criteria**: `rg "@supabase/auth-helpers-nextjs" frontend/` returns 0 hits. Google sign-in still lands on `/dashboard`.

---

## Next Logical Step

After Step 15, the highest-leverage next move is **introducing real domain ports for the LLM and Qdrant gateways** (mirroring the auth slice). That unlocks:

1. Mock-driven happy-path tests for `/api/llm/generate` and `/api/vectordb/{search,documents}` without standing up OpenAI/Anthropic/Qdrant in CI.
2. A future swap of vendors (e.g. add a `BedrockGateway` or `WeaviateGateway`) by implementing the port — no handler changes.
3. Symmetry: every external dependency becomes a *port + adapter*, and `services/` disappears entirely in favor of `infrastructure/`.

That is the architecture the auth migration was pointing at. Finish it.
