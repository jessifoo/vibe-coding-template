# Architecture Plan — Clean Architecture, Deployed as Microservices

> Successor to the *Optimization Plan* in `CODE_REVIEW.md`. The analysis there still stands; this document replaces the action plan.
>
> **Goal:** Lead-developer-quality Clean Architecture across the codebase. Every feature in the template stays; bounded contexts are made explicit; each context is deployed as its own service.
>
> **Non-goal:** Reducing the line count / "simplifying" away feature complexity. Deletion happens only when it serves cleanliness (duplicate models, redundant parsers, never-imported deps). Anything that represents real domain capability — including code that is currently unused — gets a proper home, not the trash can. Dropping an unused crate does **not** close a richer deferred path (e.g. local JWT verification via `SUPABASE_JWT_SECRET`).

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

- **Some domains are "shared across the template," not "per app."** Identity is one of those — every app you ship will want JWT verification, OAuth exchange, an `AuthenticatedUser` value. Rather than re-writing the `identity` context per app, treat it as part of the template and let each app supply its own `IdentityConfig` (Supabase project, JWT secret, OAuth providers).
- **One bounded context per binary is not a law.** §1.2 below makes the default exactly this — every context starts as a module in **one** binary. The architectural boundary (port + use case) is what matters; the process split is a deployment choice you can make later, per context, per app.

---

## 1.2 Deploy story: modular monolith first, services later if needed

> You said: *"I don't want to spin up six containers — ideally one or few build steps."* Agreed. The architecture is the same; the deployment is one binary.

### The default

The six bounded contexts below are **modules inside one Rust binary**, not separate services. Each one keeps its own `domain/` / `application/` / `infrastructure/` / `api/` (the same four-layer cake), and they all link into a single binary that runs in a single process. Frontend talks to that one URL; there is no internal network.

**Build steps for the whole stack:**

```bash
cd backend  && cargo build --release      # 1 binary, all contexts linked in
cd frontend && npm run build              # 1 static bundle
docker build -t myapp-backend ./backend   # 1 image (optional)
```

That's the build. Deploy is one backend container (Fly, Railway, Render, Cloud Run, your VM, whatever) and the frontend on a static host (Vercel, Cloudflare Pages, or served by the backend itself).

### Why this still gives you Clean Architecture

The boundary that matters is the *port + use case*, not the *process boundary*. Inside the monolith:

- Each context's `pub fn build_router(deps) -> Router` mounts under its own path prefix.
- Each context's `domain::ports::*` traits are still pluggable.
- Each context's `infrastructure/<vendor>/` adapters are still swappable — Supabase or Postgres or DynamoDB; OpenAI or Bedrock or Ollama; Qdrant or Pinecone or pgvector.
- Each context's tests still run against mocked ports, no external services in CI.

You get every reuse and substitution property of the microservices design with **one container, one build, one deploy**.

### Why this still leaves microservices on the table

When (if!) a single context starts to *hurt* inside the monolith — different scaling profile, different release cadence, different team, different SLA — extracting it is mechanical, because the boundary is already there:

1. The context already lives in its own directory with its own `build_router`.
2. Promote that directory to its own binary crate; add a 15-line `main.rs` that calls `service_runtime::run(context::build_router(...))`.
3. Replace the in-process call from the rest of the monolith with an HTTP call through `crates/clients/<context>.rs`.
4. Turn on `internal-auth` between the now-split processes.

The microservices apparatus described in §4–§7 (inter-service contracts, internal auth, tracing propagation, separate health endpoints) is **dormant until you reach for it**. The shared crates (`internal-auth`, `clients`) exist in the workspace but ship empty/unused in the default template. You don't pay for them until you use them.

### Two-mode `Makefile`

```make
make build      # cargo build --release && (cd frontend && npm run build)
make run        # one process: ./target/release/app
make docker     # one image: docker build -t myapp ./backend
```

If/when you split a context out, `make build` still builds everything in one step (Cargo builds every binary in the workspace); `make run` and `make docker` grow a docker-compose target. That's the only delta.

### What this means for §2 onward

The rest of this document describes the architecture in terms that *also work* for the split-process deployment, because the architecture is the same either way. When you see "service" below, read "context" by default; only when a context has been physically extracted does it become a "service" in the network sense. The "API Gateway" in §2 is the single binary in the monolith mode (it *is* the app); it becomes a proper ingress only after the first extraction.

---

## 1.3 The actual goal: guardrailing AI code quality

> Updated framing after your follow-up: *"This is a template for quick startup, and the main goal is restricting the code quality AI can write."*

Clean Architecture is a means, not the end. The end is: **when you (or an AI agent acting on your behalf) add a feature to an app spun up from this template, it should be hard to produce code that's wrong, sloppy, or unsafe, and easy to produce code that's correct, typed, tested, and consistent.**

The repo already states this intent in `README.md` ("Maximum Guardrails", "Compiler as Guardian") and `AGENTS.md` (the NEVER/ALWAYS lists). This plan now treats *that* as the headline goal and re-explains the Clean-Architecture work as one of several tools that serve it.

### Leverage hierarchy

Guardrails operate at five tiers. Higher tiers do more work because they require less goodwill from the AI:

| Tier | Mechanism | When it fires |
|---|---|---|
| **1** | **Compiler refuses to compile bad code** | Before the agent finishes its turn |
| **2** | **CI refuses to merge bad code** | Before a human reviews |
| **3** | **Repo structure makes the right thing easy and the wrong thing hard** | While the agent is writing |
| **4** | **The agent is told the rules** | Before the agent starts |
| **5** | **Human review catches what slipped through** | Last line of defense |

This template **leans heavily on tiers 1 and 4 today**, lightly on 2 and 5, and barely on 3. The Clean-Architecture work in §2–§10 is principally an investment in tier 3, with side benefits at tiers 1 and 2 (more lints, more layers, more typed boundaries). §14 enumerates each guardrail and its current/proposed status; the rest of this section explains why the Clean-Architecture parts of the plan earn their keep against this goal.

### How the architecture guardrails AI code

When `domain/` doesn't import `axum` and `application/` doesn't import `reqwest`, an AI is *prevented* from sprawling HTTP handling into business logic or skipping the adapter when it calls Supabase. It cannot do those things because the symbols are not in scope. This is tier-3 enforcement of separation of concerns, expressed as a Cargo dependency rule rather than a code-review rule.

When every external dependency sits behind a port and use cases consume the trait (not the concrete), an AI extending a feature has exactly one place to put its change. It also has a trivially-mockable seam, so the AI will write tests (easy = it writes them; hard = it skips them).

When new features follow a per-context *template* (copy `app/src/llm/`, rename, fill in), the AI's freelancing surface area shrinks dramatically. There is a shape to follow.

When the four-layer rules are enforced by `cargo deny`'s `bans` table and clippy's `disallowed_methods`, "you forgot to use the shared `reqwest::Client`" becomes a compile error, not a code-review nit.

### Fast-path ordering (if guardrails are what you want most)

The migration plan (§10) is ordered around architectural completeness. If your priority is *guardrails first, architecture second*, the higher-leverage ordering is:

1. **Tighten tiers 1 and 2 first** (small, mostly mechanical, biggest immediate effect on AI output): add `clippy::disallowed_methods` for the patterns the audit found bad; add `cargo deny`/`cargo audit`/`cargo machete` to `make agent-verify`; add a real `.github/workflows/ci.yml` so CI gates exist (today the project has *no* CI workflow committed); add a PR template that asks for the AGENTS.md checklist. Half a day of work; pays off immediately.
2. **Then Phase A + B** (Clean-Architecture tier-3 work). Now that tiers 1 and 2 are tight, Phase B's port/adapter restructure has somewhere to plug *new* lints into (`disallowed_types` against `Box<dyn Error>`, dependency bans against `axum` in `domain-core`, etc.).
3. **Then Phase C** (audit polish + a11y).
4. **Phase D only if a single context starts hurting.**

§14 maps every guardrail to one of these steps so you can see what lands when.

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

Each is a **module inside one binary by default** (§1.2). Promoting any of them to its own deployable service later is mechanical and per-context. The frontend always talks to **one** address — that binary in monolith mode, or the gateway in split mode — and never knows the topology behind it.

### 2.1 Runtime topology — default (modular monolith)

```
        ┌───────────────────────────┐
        │     frontend (Next 15)    │
        └──────────────┬────────────┘
                       │  HTTPS, Supabase JWT
                       ▼
   ┌─────────────────────────────────────────────────┐
   │           backend binary (one process)          │
   │                                                 │
   │   ┌─────────────────────────────────────────┐   │
   │   │      service-runtime (Axum, tracing)    │   │
   │   └──┬─────┬─────┬───────┬──────────┬───────┘   │
   │      │     │     │       │          │           │
   │      ▼     ▼     ▼       ▼          ▼           │
   │   identity llm embedding knowledge documents    │
   │      │     │     │       │          │           │
   └──────┼─────┼─────┼───────┼──────────┼───────────┘
          ▼     ▼     ▼       ▼          ▼
       Supabase OpenAI/  OpenAI   Qdrant   Supabase
        Auth   Anthropic                   DB + Storage
```

Same architectural boundaries; one process; no internal network. This is what ships by default and what you spin every new app up as.

### 2.1.alt Runtime topology — optional split (when a context outgrows the monolith)

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
├── Cargo.toml                  # [workspace]
├── rust-toolchain.toml
│
├── crates/                     # SHARED — the part you reuse across every app
│   │                           # you spin up from this template. Stable, versioned,
│   │                           # promotable to a private registry when you have 3+ apps.
│   │
│   ├── contracts/              # request/response DTOs + Zod-mirrored shapes
│   ├── domain-core/            # AppError, AppRunError, AuthenticatedUser, bearer parsing,
│   │                           # log truncation, error response — one of each.
│   ├── service-runtime/        # Axum bootstrap, tracing init, CORS, request-id + traceparent
│   │                           # middleware, /healthz + /readyz, shared reqwest::Client builder,
│   │                           # common config primitives (ServerConfig, CorsConfig, HttpTimeouts).
│   ├── internal-auth/          # signed internal-token issuer + verifier.
│   │                           # DORMANT in monolith mode; activated only when a context is
│   │                           # extracted into its own service.
│   └── clients/                # typed HTTP clients per downstream context.
│                               # DORMANT in monolith mode; activated only after extraction.
│
└── app/                        # PER-APP — this is the binary for the current app.
    │                           # When you fork this template for a new product, you edit
    │                           # this directory and (mostly) only this directory.
    │
    ├── Cargo.toml              # depends on the shared crates above
    └── src/
        ├── main.rs             # ~30 lines: load config, build router from the contexts
        │                       # you've enabled, call service_runtime::run(...).
        ├── config.rs           # per-app config struct (OpenAi, Anthropic, Qdrant, Supabase…)
        ├── composition.rs      # builds AppState, wires ports → adapters.
        │
        ├── identity/           # bounded context: identity
        │   ├── mod.rs          # pub fn build_router(deps: IdentityDeps) -> Router
        │   ├── domain/         # entities, ports (AuthGateway), domain errors
        │   ├── application/    # use cases
        │   ├── infrastructure/ # adapters: supabase/, future auth0/, future clerk/
        │   └── api/            # Axum handlers + DTOs (DTOs come from `contracts`)
        │
        ├── llm/                # bounded context: text generation (same shape)
        ├── embedding/          # bounded context: embeddings
        ├── knowledge/          # bounded context: vector search
        └── documents/          # bounded context: file upload + metadata
```

**Adding/removing a context for a new app is mechanical:**

- *Remove* a context (this app doesn't need vector search): delete the directory, remove the `merge(...)` line in `main.rs`, drop its config fields. The shared crates are untouched.
- *Add* a brand-new context (this app needs `billing`): `cp -r app/src/llm app/src/billing`, rename the types, define the new `BillingRepository` port in `domain/`, write a `StripeAdapter` in `infrastructure/`, add `merge(billing::build_router(...))` in `main.rs`. ~30 minutes for the skeleton + 1 line of wiring.

**Promotion path when you have 3+ apps sharing a context:** lift `app/src/<context>/` into `crates/<context>/` as a library crate, publish it to a private registry; every app depending on it now upgrades with `cargo update`. The four-layer shape inside is unchanged.

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

There is **one** of each. Every service imports them. No service is allowed to re-define them — enforced by workspace lints + module visibility (`pub(crate)`) + CI checks on the dependency graph. `cargo deny` plays a complementary, narrower role: it polices dependency-graph policy (bans, license allow-list, security advisories, source allow-list) so the workspace does not silently grow a second copy of a shared crate or pick up an unvetted source.

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

> **Sections 4–8 apply to the optional split-process mode (Phase D).** In default monolith mode they are largely dormant — `crates/clients/` and `crates/internal-auth/` exist but contain only the scaffolding needed for future use; no inter-process traffic exists. Read these sections as "what's already designed for when you reach for it," not "what you need to set up today."

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

Once the gateway has resolved `AuthenticatedUser`, it issues a short-lived (60 s) HS256-signed JWS and attaches it as `Authorization: Internal <token>` on the outbound request to a downstream service. Downstream services use `internal-auth::Verifier` as an Axum extractor; it consumes that header and produces an `InternalPrincipal { user_id, roles, request_id }`. The original Supabase bearer token (`Authorization: Bearer ...`) never crosses the internal boundary — the gateway is the only component that ever sees it.

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

Three ship-as-template phases plus an optional fourth that you only reach for per context, per app, when the monolith starts to hurt. Every phase leaves `main` deployable as one binary.

> **Convention:** each phase lists `Goal`, `Deliverable`, `Preserves`, and `Done when`.

### Phase A — Workspace + shared crates *(the template's bones)*

- **Goal:** Stand up the reusable parts. No context split yet, no behavior change.
- **Deliverable:** Convert `backend/` to a Cargo workspace; introduce `crates/contracts/`, `crates/domain-core/`, `crates/service-runtime/`, and the (initially empty) `crates/internal-auth/`, `crates/clients/`. The existing monolith moves to `app/` and becomes a binary crate that depends on the shared crates. `domain-core` absorbs `http_auth.rs`, `models/error.rs`, `domain/auth.rs::AuthenticatedUser`. `service-runtime` absorbs `app.rs::{build_app, init_tracing, build_cors_layer}`, the `request-id` middleware, and the common config primitives.
- **Preserves:** Every external HTTP route, every behavior, every test. One binary, one container, one `cargo build`.
- **Done when:** `cargo build --workspace`, `cargo test --workspace`, the existing `tests/api_errors.rs` (now under `app/tests/`) all pass; `curl localhost:8000/` returns the same JSON as before; the resulting Docker image is **one** image, not six.

### Phase B — Modular contexts inside the monolith *(the architecture)*

- **Goal:** Every external dependency sits behind a domain port; bounded contexts are visible as modules of `app/`. Still one binary.
- **Deliverable:**
  - `app/src/identity/`, `app/src/llm/`, `app/src/embedding/`, `app/src/knowledge/`, `app/src/documents/` — each with `domain/`, `application/`, `infrastructure/`, `api/`, and a `pub fn build_router(deps) -> Router` entry point.
  - `domain::<ctx>::*` ports for every external dependency: `AuthGateway`, `TextGenerationGateway`, `EmbeddingGateway`, `VectorIndex`, `DocumentRepository`, `FileStore`. Adapters in `infrastructure/<vendor>/`. Vendor wire types live in `infrastructure/<vendor>/wire.rs`.
  - `app/src/composition.rs` wires ports → adapters once, at startup. `AppState` becomes `Arc<dyn ...Gateway>` handles.
  - `app/src/main.rs` becomes ~30 lines: load config, build state, merge every context's router, call `service_runtime::run(...)`.
  - The previously-unused Supabase `database.rs`/`storage.rs` become the `DocumentRepository` and `FileStore` adapters for the new `documents` context — i.e. the audit's "dead code" becomes the engine of a real feature, in place.
  - Duplicates collapse (`SupabaseUser`/`UserMetadata` clones, duplicate bearer-token parser, per-request HTTP-client construction from `CODE_REVIEW.md` §1.3) — but as a side effect of giving everything a single home, not as a goal.
  - Sanitized 5xx responses (audit §2.2) and W3C `traceparent` middleware land here, in `service-runtime`, where every context inherits them.
- **Preserves:** All public routes, JSON shapes, behavior, deploy story (still one binary, one container).
- **Done when:** No `reqwest::Client::builder()` anywhere except `app/src/composition.rs`; no Axum import inside any `domain/` or `application/` module; clippy clean; every context has at least one happy-path test against mocked ports.

> **This is the ship state for the template.** Every property you want — Clean Architecture, swappable DBs, per-app context selection, mockable use cases, sane error responses — exists here. The remaining phases are *optional*, per context, per app.

### Phase C — Audit-cleanup polish *(small, safe, valuable)*

- **Goal:** Land the remaining findings from `CODE_REVIEW.md` that aren't structural.
- **Deliverable:** Frontend Docker-hostname fix (`route.ts`), migration off deprecated `@supabase/auth-helpers-nextjs` to `@supabase/ssr`, accessibility pass (`role="alert"`, contrast bumps), `error.tsx` + `not-found.tsx`, unified `MODEL_CATALOG`, copy-to-clipboard on LLM output. **Local JWT verification (Option B)** remains available as a config-flag enrichment using already-plumbed `SUPABASE_JWT_SECRET` — the unused `jsonwebtoken` crate was dropped as option (b) only; re-add it when wiring Option B, do not treat removal as closing the richer path.
- **Preserves:** All Phase B properties.
- **Done when:** `npm run build` clean (no `ENOTFOUND` warnings), Lighthouse a11y ≥ 95 on `/dashboard`, no deprecated dependencies in `frontend/package.json`.

### Phase D *(optional, per context, per app)* — Extract a context into its own service

- **When you reach for this:** a single context develops different scaling needs, release cadence, security posture, team ownership, or SLA from the rest of the binary. **Not before.**
- **Deliverable for the extracted context (`<ctx>` — pick one: identity, llm, embedding, knowledge, documents):**
  - Promote `app/src/<ctx>/` to a binary crate `services/<ctx>-service/`. The four-layer cake inside is unchanged.
  - Add `services/<ctx>-service/src/main.rs` (~15 lines: load context-specific config, call `service_runtime::run(<ctx>::build_router(...))`).
  - Implement `crates/clients/src/<ctx>.rs` — a typed HTTP client backed by the shared `reqwest::Client`.
  - Swap the in-process call site inside `app/`: replace `merge(<ctx>::build_router(...))` with a handler that delegates through `<Ctx>Client`. The application layer for that context now lives in the extracted service; the gateway only translates HTTP in / HTTP out.
  - Turn on `internal-auth` between gateway and the extracted service (`Authorization: Internal <token>` issued by gateway, verified by `internal-auth::Verifier` extractor in the service).
  - W3C `traceparent` propagation across the new boundary is already in place via `service-runtime` middleware + `clients/` — no additional code.
  - `docker-compose.yml` adds the new service alongside the gateway; production deploy adds one container.
- **What you do not have to do:** rewrite anything in `domain/` or `application/`. The boundary you maintained inside the monolith *is* the seam.
- **Done when:** the extracted context's tests pass against the extracted binary; the gateway's tests pass against the new client; an end-to-end test through both processes is green; bringing the extracted service down produces a graceful 503 with `request_id` in the gateway.

You apply Phase D to as many contexts as you actually need to extract. Most apps will never apply it; some will apply it to one context (typically `llm` for cost/scaling isolation, or `knowledge` for vector-DB locality). Each Phase D extraction is independent of the others.

---

## 11. Non-goals (explicit)

To keep the scope honest:

- **No mandatory process split.** The template ships as one binary. Microservices are a tool for when you actually need them, available context-by-context via Phase D — not a starting condition.
- **No message broker.** All flows are request/response. If async eventing is needed later (e.g., async indexing of uploaded documents), it's a clean addition — Postgres LISTEN/NOTIFY or a `crates/events/` abstraction over NATS. Not in the template.
- **No service mesh.** mTLS, retries, and rate limiting are the cluster's job in production *if* you've split. HMAC-signed internal tokens cover the dev/small-prod gap. No Istio, no Linkerd in the template.
- **No shared database in split mode.** If/when you extract a context, the data it owns goes with it — `documents-service` owns its tables; `knowledge-service` owns Qdrant; `identity-service` owns nothing locally (Supabase Auth is its store). In monolith mode this is just module hygiene.
- **No "distributed monolith."** Extracted contexts communicate **only** through their published clients; no context reads another's tables, ever.
- **No gRPC in the template.** Considered and deferred. The `crates/clients/` abstraction is built so the swap stays local to that crate.
- **No premature observability stack.** Tracing + JSON logs + `/healthz` + `/readyz` are in from Phase A. OTel exporters, Prometheus scraping, Grafana dashboards — wire them up *when you have something to monitor*, not before.
- **No deleting unused-but-capable template code to shrink LOC.** "Unused today" ≠ "delete." Promote early services (`SupabaseDatabaseService`, `SupabaseStorageService`) into bounded contexts. Dropping a *dead dependency* (never imported) is fine; dropping a *feature surface* is not. Goal is CLEAN CODE, not simplification.

---

## 12. Open Questions (defaults chosen; flag if you want different)

| Question | Default chosen | Why | Override looks like |
|---|---|---|---|
| Mono-repo or multi-repo? | **Mono-repo, Cargo workspace.** | Atomic refactors, one CI, one set of shared crates. | Split `crates/` into a separate repo + git submodule when team scales past ~10. |
| Inter-service transport? | **HTTP/JSON.** | Same skill set as the public API; debuggable; cheap. | Swap `clients/` to `tonic` + add `.proto` files; api layer unchanged. |
| Internal auth? | **HMAC-signed JWS (HS256).** | Zero key infra; one env var. | Add mTLS via service mesh; remove HMAC code. |
| JWT verification at the gateway? | **Default Option A (HTTP to Supabase) for templates; Option B (local with `SUPABASE_JWT_SECRET`) as a config flag.** | Simpler default; faster opt-in. | Flip `IDENTITY_VERIFY_MODE=local`. |
| Default deployment? | **One binary, one container (modular monolith).** | Matches your "one or few build steps" requirement; preserves every Clean-Arch property; leaves microservices on the table per-context via Phase D. | Skip straight to per-context binaries if a specific app already knows it needs them. |
| Per-context-process split (when reached)? | **One service per context that was extracted.** | Smallest unit that owns a complete capability. | Co-deploy two small extracted contexts in one binary if they always release together. |
| Database per context? | **Yes — `documents` owns its tables, `knowledge` owns Qdrant, `identity` is stateless.** | Standard hygiene; works in monolith mode (different modules) and in split mode (different services). | Allow shared read views in Supabase, but no cross-context writes, ever. |

Tell me to flip any of these and I will.

---

## 13. What this plan delivers

After Phase A + B + C (the ship state of the template):

- **One binary, one container, one `cargo build`.** Frontend is one `npm run build`. Two build steps, one deploy. That's it.
- **Six clearly bounded contexts as modules of that binary**, each with the same four-layer Clean Architecture shape. Identical mental model from one context to the next — a new contributor learns it once.
- **Zero framework imports outside `api/` and `infrastructure/`.** `domain/` and `application/` are pure Rust. Lints enforce it.
- **Every external dependency behind a port.** Mock-driven happy-path tests become trivial; CI gets real coverage without standing up OpenAI/Anthropic/Qdrant/Supabase. Swapping Postgres for Mongo, or Qdrant for Pinecone, or OpenAI for Bedrock is a one-adapter change.
- **The currently-unused Supabase database/storage code becomes the engine of the `documents` context.** Feature complexity grows, not shrinks.
- **Shared cross-cutting concerns** (error type, bootstrap, tracing init, JWT scheme, HTTP client, config primitives) are written once in `crates/` and used by every context and every app you spin up from this template.
- **Adding/removing/swapping contexts for a new app is mechanical** — directory operations + a few lines in `main.rs` and `composition.rs`.

Optionally, later, **per context, per app, only if needed** (Phase D):

- Promote any context to its own binary + container. The boundary you maintained inside the monolith *is* the seam; the extraction is mechanical (`main.rs`, an HTTP adapter in `clients/`, turn on `internal-auth`). No `domain/` or `application/` code changes.

### Two lead-developer-honest qualifiers

- **The split-process apparatus (`internal-auth`, `clients`, traceparent propagation across boundaries) is dormant in the default template.** Those crates exist with minimal code so that Phase D extraction is mechanical when you need it. They cost ~zero in compile time and zero at runtime until you reach for them.
- **The architectural work in Phases A + B is real investment.** It pays off across every app you spin from this template *and* tightens the AI-guardrail story (§1.3). If your priority is the guardrails specifically, the §1.3 fast-path order lets you cash some of that in before Phase A starts.

---

## 14. Guardrails Inventory

Every guardrail named in the plan, with current state and where it lives. ✅ = already in place; 🟡 = exists but should be strengthened; 🆕 = to add.

### Tier 1 — Compiler refuses to compile bad code

| Status | Guardrail | Location |
|---|---|---|
| ✅ | `clippy::unwrap_used = deny` | `backend/Cargo.toml` `[lints.clippy]` |
| ✅ | `clippy::expect_used = deny` | `backend/Cargo.toml` |
| ✅ | `unsafe_code = forbid` | `backend/Cargo.toml` `[lints.rust]` |
| ✅ | `clippy::all = warn` (priority -1) | `backend/Cargo.toml` |
| ✅ | `allow-unwrap-in-tests = true` (so test code isn't fighting the lint) | `backend/clippy.toml` |
| ✅ | `@typescript-eslint/no-explicit-any: error` | `frontend/.eslintrc.json` |
| ✅ | `no-debugger: error`, `no-console: warn` | `frontend/.eslintrc.json` |
| ✅ | Zod runtime validation on every API boundary | `frontend/lib/api-types.ts`, `frontend/services/llm.ts` |
| ✅ | `clippy::todo = deny`, `clippy::unimplemented = deny`, `clippy::dbg_macro = deny`, `clippy::print_stdout = deny` | `backend/Cargo.toml` `[lints.clippy]` |
| ✅ | `clippy::disallowed_methods` for `Result::unwrap_or_default` (Result variant only — Option remains allowed) and `chrono::Local::now`/`today` (force UTC) | `backend/clippy.toml` |
| ✅ | `#[non_exhaustive]` on `AppError` so pattern matches must include a `_ =>` arm | `backend/src/models/error.rs` |
| 🟡 | Promote Rust lints to `[workspace.lints]` so every crate inherits | new workspace root in Phase A |
| 🆕 | `clippy::disallowed_methods` for `reqwest::Client::new` (force shared builder from `service-runtime::http_client`) and `std::env::var` outside `service-runtime::config` | extend `backend/clippy.toml` in Phase A (the shared homes need to exist first) |
| 🆕 | `clippy::disallowed_types` for parameterized types like `Box<dyn Error>` — note: clippy currently rejects parameterized paths as "unreachable"; this rule lives at tier 4 (AGENTS.md) until clippy supports it | tracked in `backend/clippy.toml` comments |
| 🆕 | Code-gen TypeScript Zod schemas from Rust DTOs (`ts-rs` or `schemars` + `openapi-typescript`) — contracts cannot drift between backend and frontend | `crates/contracts/build.rs` (Phase A) |
| 🆕 | `tsconfig.json`: `strict: true`, `noUncheckedIndexedAccess: true`, `noImplicitOverride: true` (verify; current setting may already be strict) | `frontend/tsconfig.json` |

### Tier 2 — CI refuses to merge bad code

| Status | Guardrail | Location |
|---|---|---|
| ✅ | `cargo clippy --all-targets -- -D warnings` | `Makefile`, `scripts/verify-agent-toolchain.sh` |
| ✅ | `cargo fmt --check` | same |
| ✅ | `cargo test` | same |
| ✅ | `npm run lint`, `npm run build` | same |
| ✅ | `make agent-verify` aggregate | Makefile |
| ✅ | **`.github/workflows/ci.yml`** runs `make agent-verify` on every PR + push to `main` | `.github/workflows/ci.yml` |
| ✅ | `cargo deny check` with `deny.toml` covering license allow-list, advisory ignores (with rationale per ignore), source allow-list, wildcard bans | `backend/deny.toml`, `make deny-backend` |
| ✅ | `cargo machete` for unused dependencies (caught & removed never-imported `chrono` + `jsonwebtoken`) | `make machete-backend` |
| ✅ | Never-imported `jsonwebtoken` dep dropped (audit §2.1 option b). **Not closed:** Option B local JWT verify via `SUPABASE_JWT_SECRET` (§5.1 / §12) remains deferred enrichment — re-add the crate when implementing it | `backend/Cargo.toml` + `SupabaseConfig::jwt_secret` still plumbed |
| ✅ | Never-imported `chrono` dependency dropped | `backend/Cargo.toml` |
| 🆕 | Gateway `IDENTITY_VERIFY_MODE=local` (Option B): verify Supabase JWT locally with `SUPABASE_JWT_SECRET`; same `AuthenticatedUser` funnel as Option A | Phase C enrichment / identity context |
| ✅ | Audit §2.4 fix: `serde_json::to_string(&doc_content).unwrap_or_default()` → propagates `AppError::Internal` (lossless) | `backend/src/services/vectordb/qdrant.rs` |
| 🆕 | `cargo audit` standalone (currently covered by `cargo deny check advisories`; only add if we need finer-grained CI separation) | optional |
| 🆕 | `cargo llvm-cov` with a coverage floor (start at 60%, ratchet up; fails CI if it drops) | CI |
| 🆕 | Branch protection on `main` requiring all of the above green | repo settings (manual one-time) |
| 🆕 | Pre-commit hooks (`lefthook` recommended over `pre-commit` for Rust speed) running clippy + fmt + eslint on staged files | new `lefthook.yml` |

### Tier 3 — Repo structure makes the right thing easy and the wrong thing hard

| Status | Guardrail | Location |
|---|---|---|
| 🟡 | `domain::auth` + `application::auth` + `infrastructure::supabase::auth_gateway` exist (PR #36). Other contexts (`llm`, `embedding`, `knowledge`, `documents`) don't yet — they're in the `services/` layer with framework + business logic mixed | Fixed in Phase B |
| 🆕 | Per-crate Cargo dependency bans enforced by `cargo deny`'s `bans` (e.g. `axum`, `reqwest`, `qdrant-client`, `async-openai` forbidden in `domain-core`; `axum` forbidden in `application/` modules) | `deny.toml` + Phase A |
| 🆕 | `pub(crate)` discipline inside contexts so only `build_router` + DTOs + ports + the error mapping are exposed to siblings; `cargo modules` snapshot in CI to catch drift | Phase B + CI |
| 🆕 | Per-context template directory (copy-this-context) for spinning up a new bounded context | `.cursor/rules/templates/context/` (Phase B) |
| 🆕 | "Spin up a new app" generator script that asks "which contexts? which adapters?" and wires `main.rs` + `composition.rs` accordingly | `scripts/new-app.sh` (post-Phase B) |
| 🆕 | Sanitized 5xx responses (audit §2.2): the shared error response in `domain-core::error` *cannot* expose upstream details — the only public field is `error` + `request_id`. Even if the AI types the wrong `.to_string()` into a 5xx variant, the wire format can't leak | `crates/domain-core/src/error.rs` (Phase B) |
| 🆕 | Shared `reqwest::Client` is the only one constructable (`Client::new` banned via tier-1 disallowed_methods); the AI can't create per-request clients even by accident | tier-1 + Phase B |

### Tier 4 — The AI is told the rules

| Status | Guardrail | Location |
|---|---|---|
| ✅ | Comprehensive `AGENTS.md` with NEVER/ALWAYS lists, project patterns, Cursor-Cloud-specific instructions | `AGENTS.md` |
| ✅ | `.cursor/rules/` with `ai-guidelines.mdc`, per-stack rules (`backend/`, `frontend/`), and `templates/` | `.cursor/rules/` |
| ✅ | `CODE_STANDARDS.md` (425 lines of golden rules) | root |
| ✅ | `.github/PULL_REQUEST_TEMPLATE.md` enforcing the AGENTS.md checklist (Rust + TS + API/data sections, manual-testing field) | `.github/PULL_REQUEST_TEMPLATE.md` |
| 🆕 | `CONTRIBUTING.md` codifying Conventional Commits (the project already does this informally — commit history is clean) | new |
| 🆕 | Refresh `AGENTS.md` after Phase B so the example patterns cited (`SupabaseDatabaseService::new()?`) match the new layout (`ports::DocumentRepository`) | Phase B follow-up |
| 🆕 | Per-context "how to extend this context" mini-README so the AI gets context-specific instructions when editing in that subtree | one `README.md` per `app/src/<ctx>/` |

### Tier 5 — Human review

| Status | Guardrail | Location |
|---|---|---|
| ✅ | CodeRabbit-style review on PRs (visible in recent commit history; e.g. `f667024 fix: apply CodeRabbit auto-fixes`) | already integrated |
| ✅ | `.github/CODEOWNERS` requiring reviewer for guardrail config (`.github/`, `AGENTS.md`, `clippy.toml`, `deny.toml`, lint configs, lockfiles) | `.github/CODEOWNERS` |
| 🆕 | PR size limits (label-based or workflow-enforced) — pushes the AI to make focused changes instead of dumping 30-file diffs | optional, low priority |

### Recommended order (if guardrails are the priority)

1. ✅ **Tier 1 + 2 quick wins** — **landed in this PR**:
   - `clippy::todo/unimplemented/dbg_macro/print_stdout = deny`.
   - `clippy::disallowed_methods` for `Result::unwrap_or_default` + `chrono::Local::now`.
   - `#[non_exhaustive]` on `AppError`.
   - `cargo deny check` (advisories, licenses, sources, bans) wired into `make supply-chain-backend` and `make agent-verify`.
   - `cargo machete` wired in (and caught + cleaned `chrono` + `jsonwebtoken` dead deps).
   - Audit §2.4 fix: `qdrant.rs` no longer silently loses document text on serialization failure.
   - `.github/workflows/ci.yml` running `make agent-verify` on every PR (the repo previously had no committed CI).
   - `.github/PULL_REQUEST_TEMPLATE.md` mirroring the AGENTS.md checklist.
   - `.github/CODEOWNERS` requiring review for guardrail config + lint files + lockfiles.
2. **Phase A** (workspace + shared crates). Tier-3 boundaries exist; lint statements can be expressed per-crate.
3. **Phase B** (modular contexts). Adds the architectural guardrails (`axum` banned in `domain-core` via `deny.toml`, sanitized 5xx via `domain-core::error`, mandatory adapters, etc.).
4. **Phase C** (audit polish + AGENTS.md refresh).
5. **Phase D only if a single context starts hurting operationally.**

That's the plan. Tier 1+2 is done. Greenlight Phase A next when ready.
