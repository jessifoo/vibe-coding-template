# Vibe Coding Template - Agent Instructions

This is a modern full-stack application template with **Next.js frontend** and **Rust Axum backend**, integrated with Supabase for authentication, database, and storage.

---

## ⚠️ CRITICAL: AI CODING RULES

### NEVER DO THESE (Instant Code Review Failure)

```
❌ Use `any` type in TypeScript
❌ Use `.unwrap()` or `.expect()` in Rust (except tests/main)
❌ Use `// @ts-ignore` or `// @ts-nocheck`
❌ Leave TODO comments without context
❌ Commit commented-out code
❌ Swallow errors silently
❌ Skip input validation
❌ Hardcode secrets or API keys
❌ Use magic numbers without constants
❌ Skip error handling "for now"
```

### ALWAYS DO THESE (Required for Every Change)

```
✅ Run `cargo check` and `cargo clippy` before completing Rust changes
✅ Run `npm run lint` before completing TypeScript changes
✅ Add explicit types to all functions
✅ Handle ALL error cases with proper types
✅ Log important operations with context
✅ Validate all inputs at API boundaries
✅ Document public functions
✅ Follow existing patterns in the codebase
```

---

## Why Rust?

The Rust compiler provides maximum guardrails:
- **No null pointer exceptions** - Uses `Option<T>` and `Result<T, E>`
- **Compile-time error checking** - Catches bugs before runtime
- **Explicit error handling** - Every error must be handled
- **Type-safe concurrency** - Data races caught at compile time
- **No escape hatches** - Unlike TypeScript's `any`, Rust enforces correctness

## Architecture Overview

- **Backend**: Rust Axum with Supabase integration
- **Frontend**: Next.js with Tailwind CSS and TypeScript
- **Database**: Supabase PostgreSQL with migrations
- **Vector DB**: Qdrant for semantic search
- **LLM Integration**: OpenAI and Anthropic support

## Development Standards

### Code Style
- Use TypeScript for all frontend files
- Use Rust with full type annotations for all backend code
- Follow async/await patterns consistently
- Use snake_case for Rust, camelCase for TypeScript
- Include proper error handling with `Result<T, E>` in Rust

### Architecture Patterns
- Follow the service layer pattern for external integrations
- Use serde for API request/response serialization
- Implement proper authentication on all protected endpoints
- Use traits for abstraction (e.g., `LlmService` trait)
- Handle all errors explicitly with proper error types

### File Organization
- Backend: `backend/src/` with api/, config/, models/, services/ subdirectories
- Frontend: `frontend/` with app/, components/, services/ subdirectories
- Database: `supabase/migrations/` for all schema changes
- Rules: `.cursor/rules/` for detailed development guidelines

## Common Patterns

### Axum Endpoints (Rust)
```rust
/// Create a new item.
#[axum::debug_handler]
async fn create_item(
    headers: HeaderMap,
    Json(request): Json<CreateItemRequest>,
) -> Result<Json<ItemResponse>, AppError> {
    // Authenticate user
    let user = authenticate(&headers).await?;
    
    // Validate request
    use validator::Validate;
    request.validate().map_err(AppError::from)?;
    
    // Use service layer
    let db_service = SupabaseDatabaseService::new()?;
    let item = db_service.create("items", &request).await?;
    
    Ok(Json(item))
}
```

### Service Implementation (Rust)
```rust
/// Trait for LLM services.
#[async_trait]
pub trait LlmService: Send + Sync {
    async fn generate_text(
        &self,
        prompt: &str,
        model: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<TextGenerationResponse, AppError>;
}

/// OpenAI implementation.
pub struct OpenAiService {
    client: Client,
    api_key: String,
}

#[async_trait]
impl LlmService for OpenAiService {
    async fn generate_text(
        &self,
        prompt: &str,
        model: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<TextGenerationResponse, AppError> {
        // Implementation with explicit error handling
        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .json(&request)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(e.to_string()))?;
            
        // ...
    }
}
```

### React Components (TypeScript)
```tsx
'use client'
export default function ComponentName({ title, onAction }: Props) {
  const [loading, setLoading] = useState(false)

  const handleAction = async () => {
    try {
      setLoading(true)
      await onAction?.()
    } catch (error) {
      console.error('Action failed:', error)
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="p-4 rounded-lg border">
      {/* Component content */}
    </div>
  )
}
```

### Database Migrations
```sql
-- Create table with RLS
CREATE TABLE public.items (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE public.items ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can manage own items"
  ON public.items
  USING (auth.uid() = user_id);
```

## Development Workflow

1. **Setup**: Run `./first-time.sh` for initial configuration
2. **Development**: Use `make dev` to start all services
3. **Database**: Use `make db-migration-new name=description` for schema changes
4. **Testing**: Visit http://localhost:8000/ for health check
5. **Frontend**: Visit http://localhost:3000 for the application

## Key Services

### Backend Services (Rust)
- **SupabaseAuthService**: User authentication and token verification
- **SupabaseDatabaseService**: Generic CRUD operations via REST API
- **SupabaseStorageService**: File upload and management
- **LlmService trait**: Text generation (OpenAI/Anthropic implementations)
- **EmbeddingService trait**: Vector embeddings
- **QdrantService**: Vector database operations

### API Endpoints
- `GET /` - Health check
- `GET /api/auth/me` - Get current user
- `POST /api/auth/provider-token` - Exchange OAuth token
- `POST /api/llm/generate` - Generate text
- `POST /api/llm/embedding` - Create embedding
- `POST /api/vectordb/documents` - Add documents
- `POST /api/vectordb/search` - Semantic search
- `DELETE /api/vectordb/documents` - Delete documents

## Environment Configuration

Required environment variables:
- `SUPABASE_URL` and `SUPABASE_SERVICE_KEY` (required)
- `OPENAI_API_KEY` and/or `ANTHROPIC_API_KEY` (for LLM features)
- `QDRANT_URL` and `QDRANT_API_KEY` (for vector database)

## Error Handling

Rust's type system enforces explicit error handling:

```rust
/// Application error type.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Authentication failed: {0}")]
    Unauthorized(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("External service error: {0}")]
    ExternalService(String),
    
    // ... other variants
}

impl AppError {
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::ExternalService(_) => StatusCode::BAD_GATEWAY,
            // ...
        }
    }
}
```

## Best Practices

- Always use `Result<T, E>` for operations that can fail
- Use the service layer for external API calls
- Implement proper error handling with descriptive messages
- Use authentication middleware on protected endpoints
- Follow the established patterns for consistency
- Test API endpoints before committing
- Use database migrations for all schema changes
- Implement proper RLS policies for data security
- Let the Rust compiler guide you - if it compiles, it's likely correct

## Linting and Formatting

Backend (Rust):
```bash
cargo clippy -- -D warnings  # Linting
cargo fmt                     # Formatting
cargo test                    # Testing
```

Frontend (TypeScript):
```bash
npm run lint    # Linting
npm run format  # Formatting (if configured)
```

When adding new features, follow the established patterns and maintain consistency with the existing codebase structure.

---

## 🔒 Guardrail Checklist

Before marking any task complete, verify:

### Rust Backend
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes (no warnings)
- [ ] `cargo fmt` has been run
- [ ] No `.unwrap()` or `.expect()` (except tests)
- [ ] All errors use `Result<T, AppError>`
- [ ] Public functions have doc comments
- [ ] Logging added for important operations

### TypeScript Frontend
- [ ] `npm run lint` passes
- [ ] `npm run build` passes
- [ ] No `any` types
- [ ] All components have typed props
- [ ] Error states handled in UI
- [ ] Loading states handled in UI

### API Changes
- [ ] Request validation added
- [ ] Response types defined
- [ ] Error responses documented
- [ ] Authentication verified

### Database Changes
- [ ] Migration file created
- [ ] RLS policies defined
- [ ] Indexes added for queries
- [ ] Migration reversible (or documented)

---

## 📚 Reference Files

- **AI Guidelines**: `.cursor/rules/ai-guidelines.mdc`
- **Code Standards**: `CODE_STANDARDS.md`
- **Backend Rules**: `.cursor/rules/backend/`
- **Frontend Rules**: `.cursor/rules/frontend/`
- **Templates**: `.cursor/rules/templates/`

---

## Cursor Cloud specific instructions

### Running services natively (without Docker)

**Backend** (port 8000):
```bash
cd /workspace/backend
python3.11 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
uvicorn src.main:app --host 0.0.0.0 --port 8000 --reload
```
Requires `SUPABASE_URL` and `SUPABASE_SERVICE_KEY` env vars (or `.env` at repo root).

**Frontend** (port 3000):
```bash
cd /workspace/frontend
npm install
npm run dev
```
Needs `frontend/.env.local` with `NEXT_PUBLIC_SUPABASE_URL`, `NEXT_PUBLIC_SUPABASE_ANON_KEY`, and `NEXT_PUBLIC_API_URL`.

### Lint
- Frontend: `cd frontend && npm run lint`
- Backend: `cd backend && source .venv/bin/activate && flake8`

### Key gotchas
- The root `.env` file is loaded by the backend. When running the backend from `backend/` dir, env vars must be set explicitly or the `.env` must be in the CWD or passed via environment.
- Supabase clients are lazily instantiated in service constructors, so the backend may start with placeholder credentials; actual Supabase calls will fail at request time.
- Qdrant gracefully falls back to in-memory mode when `QDRANT_URL` is empty.
