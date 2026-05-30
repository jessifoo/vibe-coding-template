# Architecture Plan — Clean Architecture, Deployed as Microservices

> Successor to the *Optimization Plan* in `CODE_REVIEW.md`. The analysis there still stands; this document replaces the action plan.
>
> **Goal:** Lead-developer-quality Clean Architecture across the codebase. Every feature in the template stays; bounded contexts are made explicit; each context is deployed as its own service.
>
> **Non-goal:** Reducing the line count. Deletion happens only when it serves cleanliness (duplicate models, redundant parsers). Anything that represents real domain capability — including code that is currently unused — gets a proper home, not the trash can.

---

## 1. Framing (the part Bob actually cares about)

> "The architecture of a system is independent of the framework or the deployment mechanism. Microservices are a deployment option, not an architecture." — *Robert C. Martin*

The architectural boundary that matters is the **use case + the port**. Microservices are how we *express* that boundary at runtime. If we get the boundaries right inside the codebase, splitting them across processes is a configuration change. If we get them wrong, no number of containers will save us.

So the plan does two things, in order:

1. **Finish drawing the boundaries** — every external dependency lives behind a domain port; every use case lives in `application/`; every wire format lives in `infrastructure/<vendor>/`. This work is independent of the process split and pays off immediately even if we stopped here.
2. **Express each boundary as a deployable service** — one Cargo workspace, one binary crate per bounded context, typed HTTP clients between them, a thin API gateway in front. This is what we ship.

The previous audit's "delete the dead `services/supabase/*`" framing is replaced with "promote it to a real bounded context (`documents-service`) where its capabilities become productive." `SupabaseDatabaseService` and `SupabaseStorageService` are not dead; they're early. They become the engine of the documents service.

---

## 1.1 What we mean by "domain" (and why this lets you ship many apps)

> "Domain" is overloaded. Disambiguating before the rest of this document gets misread:
>
> - **DNS / web domain** — `notes-ai.com`, the URL/host an app is reached at. Each app you ship has one (or a few).
> - **Domain (Clean Architecture sense)** — the *subject matter* a chunk of code is about: identity, billing, scheduling, knowledge search, document management, chat. Each is a *bounded context* with its own entities, rules, ports, and use cases.
>
> Throughout this plan, "domain" means the **second** thing. Each of the six bounded contexts in §2 is a domain in that sense. The two meanings are independent — one app (= one DNS domain) is composed of *many* Clean-Architecture domains.

### This is a template, not an app

You said you want to spin up many different products (different URLs, different feature sets, different DBs) fast. That is exactly what this architecture is shaped for. The split is:

- **Shared across every app you'll ever ship** — the `crates/` workspace (one error type, one bootstrap, one tracing init, one JWT scheme, one HTTP-client builder, the four-layer pattern enforced by lints). This is the **template**.
- **Picked per app** — *which* Clean-Architecture domains the app instantiates, and *which* adapter backs each port. This is the **app**.

So when you have idea #4 and want to spin it up, the shared crates come along for free; you only decide which domains the app has and which DBs back them.

### Workflow for a new app

1. **Fork (or copy) the workspace.** You get every shared crate plus the per-service skeletons for free.
2. **Decide which domains the app needs.** Almost every product needs `identity` and `api-gateway`. The rest you pick:
   - *AI note-taking:* `identity` + `llm` + `embedding` + `knowledge` + `documents`.
   - *Scheduling SaaS:* `identity` + new `calendar` + new `notifications`; drop `llm`/`embedding`/`knowledge`.
   - *AI customer support:* `identity` + `llm` + `knowledge` + new `chat`.
   - *Multiplayer game lobby:* `identity` + new `lobby` backed by Redis; nothing else from the template.
3. **Delete the services you don't need.** Cargo workspace — remove the entry, the shared crates are untouched.
4. **Add the new domains.** Same four-layer cake (`domain/` → `application/` → `infrastructure/<vendor>/` → `api/`). Copy a template service directory, rename, wire its routes into the gateway. ~10 minutes for the skeleton.
5. **Pick the adapter(s).** Each domain owns its persistence and external-service choices independently (see below).

### "Different DBs" is just "different adapters behind the same port"

The architecture treats persistence as an external dependency behind a port. The DB choice never leaks past the adapter, so swapping it is mechanical:

| Domain | Today's default adapter | Swap looks like |
|---|---|---|
| `identity` | Supabase Auth | Auth0, Clerk, FusionAuth, raw JWT → new `AuthGateway` impl |
| `documents` (metadata) | Supabase Database (Postgres via REST) | Postgres direct (`sqlx`), MongoDB, DynamoDB, SQLite → new `DocumentRepository` impl |
| `documents` (files) | Supabase Storage | S3, R2, GCS, local disk → new `FileStore` impl |
| `knowledge` (vectors) | Qdrant | Pinecone, Weaviate, pgvector, LanceDB → new `VectorIndex` impl |
| `llm` | OpenAI + Anthropic | Bedrock, Vertex, Groq, Ollama (local) → new `TextGenerationGateway` impl |
| `embedding` | OpenAI | Cohere, Voyage, BGE-local → new `EmbeddingGateway` impl |

For a brand-new domain in a brand-new app — say `billing` — you choose the DB the same way: define a `BillingRepository` port in the new service's `domain/`, write a `StripeBillingAdapter` (or `PostgresBillingAdapter`, or `LemonSqueezyBillingAdapter`) in `infrastructure/`, wire it in `composition.rs`. Application code talks to the port; nothing else changes.

### When you have 3+ apps live, promote the template to a private crate registry

Phase 0 says "Cargo workspace." That's right for *one* app. The moment you have three apps sharing the same shared crates, fork-and-backport becomes a maintenance tax. At that point, publish `crates/*` to a private registry (Cloudsmith, ktra, or a `crates.io` org) and each app's `Cargo.toml` picks them up by version: `domain-core = "0.4"`, `service-runtime = "0.4"`. A bug fix in the shared crate goes to every app you have with one `cargo update`. The per-app code is unaffected — that was the whole point of factoring it out.

If you stay at one app for a while, the workspace is fine. The decision to promote is reversible and entirely opt-in.

### Two caveats worth being explicit about

- **Some domains are "shared across the template," not "per app."** Identity is one of those — every app you ship will want JWT verification, OAuth exchange, an `AuthenticatedUser` value. Rather than re-writing the `identity` service per app, treat it as part of the template and let each app supply its own `IdentityConfig` (Supabase project, JWT secret, OAuth providers). Same for `api-gateway` — the routing tables are per-app, but the gateway *frame* is template.
- **One bounded context per binary is not a law.** If app #N has three small domains that always deploy together, it is fine to start with them in one binary, with three properly-separated module trees inside. The architectural boundary (port + use case) is what matters; the process split is a deployment choice you can make per app. The template makes the split easy; it does not force it.

---

## 2. Bounded Contexts

Six contexts, derived from the existing feature set with no capability removed:

| # | Context | Owns | Upstream(s) | Today lives in |
|---|---------|------|-------------|----------------|
| 1 | **Identity** | JWT verification, user profile, OAuth provider exchange | Supabase Auth | `domain/auth.rs`, `application/auth.rs`, `infrastructure/supabase/auth_gateway.rs` |
| 2 | **LLM** | Text generation across providers | OpenAI, Anthropic | `services/llm/text_generation.rs` |
| 3 | **Embeddings** | Vector embedding across providers | OpenAI (and future Cohere/Voyage) | `services/llm/embedding.rs` |
| 4 | **Knowledge** | Document indexing + semantic search, owner-scoped | Qdrant, Embeddings (internal) | `services/vectordb/qdrant.rs` |
| 5 | **Documents** | File upload, metadata persistence, content workflows | Supabase Storage, Supabase DB, Knowledge (internal) | `services/supabase/database.rs`, `services/supabase/storage.rs` (currently unused) |
| 6 | **API Gateway** | Ingress, CORS, JWT verification, internal-token issuance, fan-out routing | All of the above | `app.rs`, `api/*` |

Each becomes a deployable service of the same name. The frontend continues to talk to **one** address — the gateway — and never knows the topology behind it.

### 2.1 Runtime topology

```
                          ┌───────────────────────────┐
                          │      frontend (Next 15)   │
                          └──────────────┬────────────┘
                                         │  HTTPS, Supabase JWT
                                         ▼
                          ┌───────────────────────────┐
                          │       api-gateway         │
                          │  - verifies JWT           │
                          │  - issues internal token  │
                          │  - request-id, tracing    │
                          └───┬────┬────┬────┬────────┘
                              │    │    │    │  HTTP/JSON + signed internal token
              ┌───────────────┘    │    │    └────────────────┐
              ▼                    ▼    ▼                     ▼
   ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
   │ identity-service │  │   llm-service    │  │embedding-service │  │knowledge-service │
   │   → Supabase     │  │ → OpenAI/Anthr.  │  │   → OpenAI       │  │   → Qdrant       │
   └──────────────────┘  └──────────────────┘  └────────▲─────────┘  └────────▲─────────┘
                                                        │                     │
                                                        └──────────┬──────────┘
                                                                   │
                                                       ┌───────────┴───────────┐
                                                       │   documents-service   │
                                                       │ → Supabase Storage    │
                                                       │ → Supabase Database   │
                                                       └───────────────────────┘
```

Service-to-service edges only flow downward and rightward (no cycles). Identity is leaf. Embeddings is leaf. LLM is leaf. Knowledge depends on Embeddings. Documents depends on Knowledge and Embeddings (indirectly). The gateway depends on everyone but is depended on by no one.

### 2.2 Per-service layering — the same cake, six times

Every service uses the identical four-layer shape:

```
services/<name>-service/
├── Cargo.toml                  # binary crate
└── src/
    ├── main.rs                 # ~15 lines: load config, call service-runtime::run(...)
    ├── api/                    # Axum router, handlers, request/response DTOs
    │   ├── mod.rs
    │   └── <feature>.rs
    ├── application/            # use-cases — orchestration, transport-agnostic
    │   ├── mod.rs
    │   └── <feature>_use_case.rs
    ├── domain/                 # entities, value objects, ports (traits), domain errors
    │   ├── mod.rs
    │   ├── entities.rs
    │   └── ports.rs
    └── infrastructure/         # port implementations: external adapters
        ├── mod.rs
        └── <vendor>/
            ├── mod.rs
            ├── adapter.rs
            └── wire.rs         # vendor-specific (de)serialization types
```

Rules (enforced by clippy `disallowed_methods` + module visibility):

- `domain` depends on **nothing** except `std` and `domain-core` (shared crate).
- `application` depends only on `domain`.
- `infrastructure` depends on `domain` (to implement ports) and external SDKs.
- `api` depends on `application`, plus `service-runtime` (HTTP boilerplate) and `contracts` (DTO definitions shared with consumers).
- No layer ever imports `Axum`, `reqwest`, `qdrant-client`, `async-openai` outside `infrastructure/` and `api/`.

This is *exactly* the structure PR #36 introduced for auth. We are repeating it deliberately and uniformly.

---

## 3. Cargo Workspace Layout

```
backend/
├── Cargo.toml                  # [workspace], shared profiles, shared deps
├── rust-toolchain.toml         # unchanged
├── crates/
│   ├── contracts/              # request/response DTOs shared by services and clients
│   ├── domain-core/            # cross-context primitives: AuthenticatedUser, RequestId, AppError
│   ├── service-runtime/        # Axum bootstrap, tracing, request-id, health endpoints, AppRunError
│   ├── internal-auth/          # signed internal token format (issuer + verifier)
│   └── clients/                # typed HTTP clients (one module per downstream service)
└── services/
    ├── api-gateway/
    ├── identity-service/
    ├── llm-service/
    ├── embedding-service/
    ├── knowledge-service/
    └── documents-service/
```

### 3.1 Shared crate responsibilities

- **`contracts`** — all `serde`-derived request/response types exchanged between services or returned to the frontend. Versioned (`v1` module per service). Stays small; no logic.
- **`domain-core`** — `AuthenticatedUser`, `UserId`, `RequestId`, `AppError`, `AppRunError`, the bearer-token parser (today's `http_auth.rs`), the error-response shape with a `request_id` field. Pure data + free functions. Zero framework imports.
- **`service-runtime`** — `pub fn run(router: Router, cfg: ServiceConfig) -> Result<(), AppRunError>`. Handles bind, tracing init, request-id middleware, CORS (per-service config), trace-propagation middleware (W3C `traceparent`), `GET /healthz` (liveness) and `GET /readyz` (readiness — checks port-bound + upstream-reachable). All six services call this; no service writes Axum bootstrap by hand.
- **`internal-auth`** — issuer (used by gateway): produces `Authorization: Internal <token>` where `<token>` is a JWS-signed JSON payload `{ user_id, roles, request_id, exp, iss }` with HS256 against `INTERNAL_AUTH_HMAC_SECRET`. Verifier (used by downstream services): an Axum extractor that yields the inner principal or rejects with 401.
- **`clients`** — typed clients (`IdentityClient`, `LlmClient`, `EmbeddingClient`, `KnowledgeClient`, `DocumentsClient`). Each owns its own `reqwest::Client`, propagates `traceparent`, attaches the internal-auth token. Returns `Result<T, ClientError>` where `T` is from `contracts`.

This split is what lets every service stay ~300–800 LOC of *its own* concerns, with the boilerplate factored out.

### 3.2 Shared vs. per-service — the explicit map

> Microservices, not duplication. Every cross-cutting concern lives in exactly one shared crate. Per-service code is reserved for *domain logic that is unique to that bounded context*. The table below is the literal source-to-destination map for every chunk in the current monolith — there is no ambiguity about where anything goes.

**What lives in the shared crates (written once, used by every service):**

| Concern | Current location | Future home |
|---|---|---|
| HTTP error type + `IntoResponse` | `backend/src/models/error.rs` — `AppError`, `ApiErrorResponse` | `domain-core::error` |
| Fatal startup error | `backend/src/models/error.rs` — `AppRunError` | `domain-core::error` |
| Authenticated principal | `backend/src/domain/auth.rs` — `AuthenticatedUser` | `domain-core::principal` |
| Bearer-token parsing | `backend/src/http_auth.rs` | `domain-core::http_auth` |
| Failed-response reader | `backend/src/http_error.rs` | `domain-core::http_error` |
| Log-preview truncation | `backend/src/api/logging.rs` — `truncate_for_log`, `LOG_PREVIEW_CHARS` | `domain-core::logging` |
| Axum bootstrap + tracing init + CORS layer | `backend/src/app.rs` — `build_app`, `init_tracing`, `build_cors_layer` | `service-runtime::bootstrap` |
| Request-id + W3C `traceparent` middleware | scattered (today: `tower-http` only) | `service-runtime::middleware` |
| `/healthz` + `/readyz` handlers | derived from current `/` | `service-runtime::health` |
| Shared `reqwest::Client` builder | `backend/src/api/state.rs` | `service-runtime::http_client` |
| Common config primitives (`ServerConfig`, `CorsConfig`, `Environment`, `HttpTimeouts`, `SettingsError`) | `backend/src/config/mod.rs` | `service-runtime::config` |
| Internal-auth token issuer + verifier | new | `internal-auth` |
| All cross-service DTOs + validator annotations | `backend/src/models/{auth,llm,vectordb}.rs` | `contracts::v1::{identity,llm,embedding,knowledge,documents}` |
| Provider enum `LlmProvider`, `LlmUsage` | `backend/src/models/llm.rs` | `contracts::v1::llm` |
| Typed clients for each downstream service | new | `clients::{identity,llm,embedding,knowledge,documents}` |

There is **one** of each. Every service imports them. No service is allowed to re-define them — enforced by `cargo deny` + workspace lints.

**What lives per-service (unique to that bounded context):**

| Concern | Owner |
|---|---|
| Narrow domain error enum (e.g. `AuthDomainError`, `KnowledgeDomainError`) | `services/<name>-service/src/domain/error.rs` |
| Domain entities (e.g. `Document`, `SearchResult`, `TextGenerationOutcome`) | `services/<name>-service/src/domain/entities.rs` |
| Domain ports / traits (e.g. `AuthGateway`, `VectorIndex`, `EmbeddingGateway`, `DocumentRepository`, `FileStore`) | `services/<name>-service/src/domain/ports.rs` |
| Use cases (e.g. `GenerateText`, `IndexDocument`, `UploadDocument`) | `services/<name>-service/src/application/` |
| Vendor adapters (e.g. `OpenAiTextGenerationAdapter`, `SupabaseAuthGateway`, `QdrantVectorIndex`, `SupabaseDatabaseAdapter`) | `services/<name>-service/src/infrastructure/<vendor>/adapter.rs` |
| Vendor wire types (e.g. `AnthropicRequest`, `OpenAiEmbeddingResponse`) | `services/<name>-service/src/infrastructure/<vendor>/wire.rs` |
| Axum routes + handlers | `services/<name>-service/src/api/` |
| Per-service config knobs (e.g. `OpenAiConfig`, `AnthropicConfig`, `QdrantConfig`, `SupabaseConfig`) | `services/<name>-service/src/config.rs` |

### 3.3 Two principles that keep §3.2 honest

1. **Domain errors are narrow per-service enums; the HTTP error response is the single shared type.** Each service defines its own `<Context>DomainError` (transport-agnostic, only the variants it actually needs). The `api/` layer of that service has a single ~6-line `map_<context>_error()` function that funnels it into the shared `AppError` from `domain-core`. This is not duplication — it is the architectural seam. Today's `backend/src/api/auth_handler.rs::map_auth_error` (5 match arms) is the template; every service will have exactly one of those.

2. **No grab-bag `utils` crate.** Every shared concern has a *named* module in `domain-core` or `service-runtime`. The moment we'd be tempted to create `utils/`, that means we have not understood the concern well enough to name it. `bearer_token_from_value` lives in `domain-core::http_auth`; `truncate_for_log` lives in `domain-core::logging`; the `reqwest::Client` builder lives in `service-runtime::http_client`. If a new helper appears that fits neither, we name a new module — never a generic one.

### 3.4 Dependency graph (arrows = depends on)

```
                         ┌────────────────────┐
                         │      contracts     │   DTOs only, no logic
                         └─────────┬──────────┘
                                   │
       ┌───────────────────────────┼───────────────────────────┐
       │                           │                           │
       ▼                           ▼                           ▼
┌─────────────┐         ┌───────────────────┐         ┌────────────────┐
│ domain-core │ ◄────── │  service-runtime  │ ◄────── │    clients     │
│   error     │         │   bootstrap       │         │  (per service) │
│   principal │         │   middleware      │         └───────┬────────┘
│   http_auth │         │   health          │                 │
│   http_error│         │   http_client     │                 │
│   logging   │         │   config          │                 │
└──────┬──────┘         └─────────┬─────────┘                 │
       │                          │                           │
       │                          ▼                           │
       │              ┌───────────────────────┐               │
       │              │     internal-auth     │               │
       │              │  issuer (gateway)     │               │
       │              │  verifier (downstream)│               │
       │              └───────────┬───────────┘               │
       │                          │                           │
       └──────────────┬───────────┴────────────┬──────────────┘
                      ▼                        ▼
   ┌─────────────────────────────────────────────────────────┐
   │   services/<each>-service                               │
   │     domain/  application/  infrastructure/  api/        │
   │     (only domain-specific code lives here)              │
   └─────────────────────────────────────────────────────────┘
```

Shared crates never depend on services. Services never depend on each other directly — they go through `clients/`. There are no cycles, and the dependency direction matches the architectural direction.

---

## 4. Inter-service Contracts

### 4.1 Transport

**HTTP/JSON over the cluster network.** Justification:

- Same skill set as the external API; no new tooling.
- Plays well with the existing tracing/request-id middleware.
- Easy to debug with `curl`.
- gRPC/tonic is a viable upgrade later; it's contained inside `clients/` and `api/` so the swap stays local.

### 4.2 Versioning

- Each service exposes `/v1/...`.
- DTOs live in `crates/contracts/src/<service>/v1/`.
- Breaking changes ship a new module (`v2/`) alongside v1; deprecation window enforced via a tracing warning emitted by the old handler.

### 4.3 Frontend boundary

Frontend continues to call only the gateway. The frontend's `lib/api-types.ts` Zod schemas are kept in lockstep with `contracts/` via a small `make sync-contracts` step (initially manual; later a `ts-rs` or `schemars` + `openapi-typescript` pipeline).

---

## 5. Inter-service Authentication & Authorization

Two tiers:

### 5.1 External (frontend ↔ gateway)

Supabase JWT in the `Authorization: Bearer <token>` header. Gateway verifies it. Two options for verification, both supported:

- **Option A (default for the template):** call `GET {SUPABASE_URL}/auth/v1/user` with the bearer — current behavior. Simple, no key management.
- **Option B (production-recommended):** verify the JWT locally using `SUPABASE_JWT_SECRET` (already plumbed in `SupabaseConfig` — finally a use for it). Eliminates a network hop per request.

Both paths funnel into the same `AuthenticatedUser` value. The choice is a `IdentityConfig::Mode` enum so deployments pick.

### 5.2 Internal (gateway ↔ downstream services)

Once the gateway has resolved `AuthenticatedUser`, it issues a short-lived (60 s) HS256-signed JWS in a custom header (`X-Internal-Auth`). Downstream services use `internal-auth::Verifier` as an Axum extractor; it produces an `InternalPrincipal { user_id, roles, request_id }`. No bearer token ever crosses the internal boundary.

Why HMAC instead of mTLS for the template:

- Zero infrastructure (no per-service certs, no CA).
- A single rotation point: change the env var, restart services.
- mTLS can be layered on top later by the cluster (Istio, Linkerd, or a sidecar) without code changes.

Authorization decisions (roles, ownership) stay in the downstream service that owns the data — e.g., the `_owner_user_id` filter in `knowledge-service` continues to enforce per-user isolation, but now it reads `user_id` from `InternalPrincipal` instead of from a bearer token.

---

## 6. Observability

- **Tracing.** Every service inits `tracing` via `service-runtime`. The existing `tower-http::request-id` middleware is augmented with a W3C `traceparent` propagator: incoming `traceparent` becomes the parent span; outgoing client requests (via `clients/`) attach the current span's `traceparent`. End-to-end traces work without an OTel collector but slot directly into one when deployed.
- **Logging.** All logs JSON in production (already supported by `tracing-subscriber`'s `json` feature). Every log line carries `request_id`, `service`, and (when present) `user_id`.
- **Metrics.** Phase 8: add `metrics` + `metrics-exporter-prometheus` and a `/metrics` endpoint per service through `service-runtime`. Counters for request rate, latency histogram, error rate by status class, and per-downstream client metrics.
- **Health.** `/healthz` (process up) and `/readyz` (port bound + upstream pingable) on every service. Distinct endpoints so a slow Supabase doesn't take the pod out of the cluster.

---

## 7. Configuration

- One `.env.example` at the repo root, **sectioned by service**, lists every variable.
- Per-service config struct lives in that service's `src/config.rs`, parsed via `service-runtime::config::load::<MyConfig>()`. The shared `service-runtime` crate handles common knobs (port, log level, CORS origins, internal-auth secret).
- Production: each service gets only the variables it needs. The gateway has Supabase URLs but not OpenAI keys; `llm-service` has OpenAI/Anthropic keys but not Qdrant URL; etc. Principle of least privilege at the env-var level.
- Local dev: a single root `.env` shared across services, loaded by docker-compose into each container.

This is also where the magic timeouts called out in `CODE_REVIEW.md` §2.9 land — per-service, with sane defaults overridable by env.

---

## 8. Local Development

- **`docker-compose.yml`** at root grows from 1 backend container to 6 (one per service) + Qdrant. Each service exposed on `127.0.0.1:80XX` for direct curling during dev.
- **`make dev`** orchestrates everything. A new **`make dev-monolith`** target keeps the option of running the whole topology in one process (compiles all six services into a single binary with all routers mounted) for fast inner loops on a laptop. The two modes share the same `service-runtime` runner.
- **Frontend** unchanged: it talks to the gateway at `http://localhost:8000` (or whatever `NEXT_PUBLIC_API_URL` says), exactly as today.
- **Hot reload:** `cargo-watch` per service (or scoped via `cargo watch -w services/llm-service`).

---

## 9. Testing Strategy

Every layer keeps its own test surface; cross-cutting tests live at the top.

| Layer | Test type | Where it lives |
|---|---|---|
| `domain/` | Pure unit tests, no I/O | In-module `#[cfg(test)]` |
| `application/` | Use-case tests with mocked ports | In-module `#[cfg(test)]`; mocks under `application/tests/mocks.rs` |
| `infrastructure/` | Adapter unit tests (status-code mapping, parsing). For HTTP adapters, a `mockito` or `wiremock` server | `infrastructure/<vendor>/tests/` |
| `api/` | Router-level tests using `tower::ServiceExt::oneshot` with mock use-cases | `services/<svc>-service/tests/api_*.rs` |
| Inter-service | **Contract tests** — each consumer pins a JSON fixture of the upstream's response; both consumer and provider tests reference the same fixture | `crates/contracts/tests/contracts_<svc>.rs` |
| End-to-end | Real services in docker-compose, gateway hit from the test harness | Top-level `e2e/` crate or Playwright in `frontend/` |

The "happy-path test gap" called out in `CODE_REVIEW.md` §2.7 is fixed by **construction** in this architecture: every service has its own integration test crate that builds the service router with mocked ports and exercises the public routes. No external services needed in CI.

---

## 10. Migration Plan — Phased, Always-Deployable

Eight phases. `main` deploys after every phase. Each phase is atomic relative to the next.

> **Convention:** each phase below lists `Goal`, `Deliverable`, `Touches`, `Preserves`, and `Done when`.

### Phase 0 — Workspace + shared crates

- **Goal:** Set the stage. No service split yet.
- **Deliverable:** Convert `backend/` to a Cargo workspace; introduce `crates/contracts/`, `crates/domain-core/`, `crates/service-runtime/`, `crates/internal-auth/`, `crates/clients/`. The existing monolith moves to `services/api-gateway/` (renamed but still containing all logic).
- **Touches:** `backend/Cargo.toml` → workspace root; all of `backend/src/` migrates to `backend/services/api-gateway/src/`; `domain-core` absorbs `http_auth.rs`, `models/error.rs`, `domain/auth.rs`'s `AuthenticatedUser`; `service-runtime` absorbs the `app.rs` bootstrap.
- **Preserves:** Every external HTTP route, every behavior, every test passes unchanged. Binary name changes (`backend` → `api-gateway`); `Dockerfile`, `Makefile`, and `docker-compose.yml` updated accordingly.
- **Done when:** `cargo build --workspace`, `cargo test --workspace`, and the existing `tests/api_errors.rs` (now under `services/api-gateway/tests/`) all pass; `curl localhost:8000/` returns the same JSON as before.

### Phase 1 — Finish ports-and-adapters inside the monolith

- **Goal:** Every external dependency sits behind a domain port. No code crosses a layer it shouldn't.
- **Deliverable:**
  - `domain::llm::TextGenerationGateway` port; `infrastructure::openai::TextGenerationAdapter` + `infrastructure::anthropic::TextGenerationAdapter`; vendor wire types moved to `infrastructure/<vendor>/wire.rs`.
  - `domain::embedding::EmbeddingGateway` port; `infrastructure::openai::EmbeddingAdapter`.
  - `domain::knowledge::VectorIndex` port; `infrastructure::qdrant::VectorIndexAdapter` (today's `qdrant.rs`).
  - `domain::documents::{DocumentRepository, FileStore}` ports; `infrastructure::supabase::{DatabaseAdapter, StorageAdapter}` (the currently-unused database/storage code becomes the home for these).
  - Delete duplicates that now have one canonical home: the `SupabaseUser`/`UserMetadata` clones in `models/auth.rs` (the gateway has `SupabaseUserRecord` already), the duplicate bearer-token parser in `services/supabase/auth.rs` (the canonical lives in `domain-core` after Phase 0), and the per-request HTTP client constructions called out in `CODE_REVIEW.md` §1.3.
  - Application-layer use cases for every feature: `GenerateText`, `CreateEmbedding`, `IndexDocument`, `SearchDocuments`, `UploadDocument`, `ListDocuments`.
  - `AppState` becomes a struct of `Arc<dyn ...Gateway>` ports, composed in a new `composition.rs`.
- **Touches:** Most of `services/api-gateway/src/`. Roughly 30 files.
- **Preserves:** Public API routes, JSON shapes, behavior, tests.
- **Done when:** No `reqwest::Client::builder()` anywhere except `composition.rs`; no Axum import in `domain/` or `application/`; clippy clean; all routes still return the same JSON.

> At the end of Phase 1 we have a *Clean Architecture monolith*. If we stopped here we'd still have shipped a meaningful upgrade. We don't stop here.

### Phase 2 — Extract `identity-service`

- **Goal:** First bounded context lives in its own process.
- **Deliverable:**
  - `services/identity-service/` binary. Copies the auth domain/application/infrastructure layers from the gateway. Exposes `POST /v1/sessions/from-bearer` (verify a Supabase JWT → return profile) and `POST /v1/sessions/from-provider-token` (exchange OAuth → return session).
  - `crates/clients/src/identity.rs` — typed `IdentityClient` with `verify_bearer(...)` and `exchange_provider_token(...)`.
  - Gateway's `api/auth.rs` swaps from in-process `AuthUseCase` to `IdentityClient`. The gateway's middleware that resolves the authenticated user (today: `authenticated_user_from_headers`) now calls `IdentityClient::verify_bearer` and caches the result for the request lifetime.
  - docker-compose: add `identity-service` on `:8001`.
- **Preserves:** Frontend behavior is identical (`/api/auth/me`, `/api/auth/provider-token` still work).
- **Done when:** Gateway has no `domain::auth` module of its own; integration test verifies `/api/auth/me` end-to-end through both processes; bringing identity-service down causes a graceful 503 with `request_id`.

### Phase 3 — Extract `llm-service`

- Same pattern as Phase 2. New service on `:8002`. New `LlmClient` in `crates/clients/`. Gateway's `api/llm.rs::generate_text` becomes a 10-line proxy + DTO mapping.
- New service hosts the OpenAI/Anthropic adapters, the request validation, the per-provider model catalog, and (Phase 8) per-user rate limiting and cost accounting.

### Phase 4 — Extract `embedding-service`

- Same pattern. `:8003`. New `EmbeddingClient`. Knowledge service (still inside the gateway at this point) starts calling embedding-service over HTTP instead of in-process.

### Phase 5 — Extract `knowledge-service`

- Same pattern. `:8004`. New `KnowledgeClient`. Owns Qdrant. Calls `embedding-service` internally for both ingestion and search. The owner-scoping logic (`_owner_user_id` filter, the audit's §2.7 unit tests) moves with it.
- The internal-auth principal (`user_id`) now carries through from gateway → knowledge-service, so the owner filter has a clean, single source of truth.

### Phase 6 — Extract `documents-service`

- `:8005`. Owns Supabase database (document metadata) and Supabase storage (file blobs). Exposes `POST /v1/documents` (upload + index), `GET /v1/documents`, `GET /v1/documents/:id`, `DELETE /v1/documents/:id`.
- On upload: documents-service writes the blob to Supabase Storage, writes metadata to Supabase DB, then calls `knowledge-service` to embed + index, then returns the document with both its storage URL and its knowledge-base ID.
- This is the moment the dead code from the old audit becomes *the load-bearing capability of an entire service*. The `services/supabase/{database,storage}.rs` files are not deleted; they are promoted into `services/documents-service/src/infrastructure/supabase/`.
- Frontend gets a new `/api/documents` namespace exposed by the gateway.

### Phase 7 — Promote the gateway

- `api-gateway` becomes pure plumbing: routing tables, JWT pre-verification, internal-token issuance, response shaping, CORS, request-id, OpenAPI publishing. Zero business logic.
- The gateway's `Cargo.toml` drops `qdrant-client`, `async-openai`, and Supabase-specific deps; it depends only on `crates/contracts`, `crates/clients`, and `service-runtime`. The dependency graph now reflects the architectural diagram.

### Phase 8 — Hardening

- **Internal auth on:** every downstream service requires `X-Internal-Auth`; gateway issues it; rotation runbook in `docs/`.
- **W3C `traceparent` propagation** end-to-end; one screenshot of an end-to-end trace in the README.
- **Metrics + Prometheus** endpoint on every service.
- **Contract tests** pinned for every cross-service edge.
- **Per-user rate limits** in `llm-service` and `embedding-service` (configurable, default to generous).
- **Sanitized 5xx responses** (the audit's §2.2): generic body + `request_id`; full detail only in logs. Implemented once in `service-runtime`, picked up by every service.
- **A11y + UX fixes** from the audit's §3 — those are still valid and land here.

---

## 11. Non-goals (explicit)

To keep the scope honest:

- **No message broker.** All flows are request/response. If async eventing is needed later (e.g., async indexing of uploaded documents), it's a clean addition — add a `crates/events/` module + Postgres-LISTEN-based queue or NATS. Not now.
- **No service mesh.** mTLS, retries, and rate limiting are the cluster's job in production. The template ships with HMAC-signed internal tokens and per-client retry policies in `clients/` — enough for dev and small prod.
- **No shared database.** Every service that needs persistence owns its own schema (documents-service owns the `documents` and `document_files` tables; knowledge-service owns Qdrant; identity-service owns nothing locally — Supabase Auth is its store). Migrations stay per-service.
- **No "distributed monolith."** Services communicate **only** through their published clients; no service reads another's tables. The gateway is the only fan-out point.
- **No gRPC in Phase 1.** Considered and deferred. The `clients/` abstraction is built so the swap stays local.

---

## 12. Open Questions (defaults chosen; flag if you want different)

| Question | Default chosen | Why | Override looks like |
|---|---|---|---|
| Mono-repo or multi-repo? | **Mono-repo, Cargo workspace.** | Atomic refactors, one CI, one set of shared crates. | Split `crates/` into a separate repo + git submodule when team scales past ~10. |
| Inter-service transport? | **HTTP/JSON.** | Same skill set as the public API; debuggable; cheap. | Swap `clients/` to `tonic` + add `.proto` files; api layer unchanged. |
| Internal auth? | **HMAC-signed JWS (HS256).** | Zero key infra; one env var. | Add mTLS via service mesh; remove HMAC code. |
| JWT verification at the gateway? | **Default Option A (HTTP to Supabase) for templates; Option B (local with `SUPABASE_JWT_SECRET`) as a config flag.** | Simpler default; faster opt-in. | Flip `IDENTITY_VERIFY_MODE=local`. |
| Per-service or per-bounded-context deployments? | **One service per context.** | Smallest unit that owns a complete capability. | Merge embedding-service into llm-service if you're sure they'll never diverge. |
| Database per service? | **Yes (documents-service owns its tables in Supabase; identity-service is stateless; knowledge-service owns Qdrant).** | Standard microservices hygiene. | Allow shared read views in Supabase, but no cross-service writes. |

Tell me to flip any of these and I will.

---

## 13. What this plan delivers

- **Six clearly bounded contexts**, each a small Rust crate with the same four-layer Clean Architecture shape. Identical mental model from one service to the next — a new contributor learns it once.
- **One gateway, one frontend address.** External API surface is unchanged from today; everything else is behind the gateway.
- **Zero direct framework imports outside `api/` and `infrastructure/`.** `domain/` and `application/` are pure. Lints enforce it.
- **Every external dependency behind a port.** Mock-driven happy-path tests become trivial; CI gets real coverage without standing up OpenAI/Anthropic/Qdrant/Supabase.
- **The currently-unused Supabase database/storage code becomes the engine of a real feature** (`documents-service`). Feature complexity grows, not shrinks.
- **Inter-service auth, tracing, and configuration** are factored into shared crates so they are written once and used uniformly.
- **`main` is deployable at every phase** of the migration.

Two qualifiers, lead-developer-honest:

- The "right" time to actually split processes is when the boundaries *hurt* inside the monolith — when two contexts are deployed on different rhythms, scaled differently, owned by different teams, or constrained by different SLAs. The template doesn't suffer that pain yet; the split here is pedagogical — it makes the architecture visible and gives users a ready-made layout to grow into. If you intend the template to *encourage* monolith-first usage, we can stop at Phase 1 (Clean Architecture monolith) and ship Phases 2–8 as an optional `microservices/` reference deployment behind a feature flag. Say the word and I'll structure it that way.
- Operational cost goes up. Six containers, six logs, six health checks. The observability work in Phase 8 is not optional — it's what makes the cost survivable. Budget for it.

That's the plan. Greenlight Phase 0 and I'll start.
