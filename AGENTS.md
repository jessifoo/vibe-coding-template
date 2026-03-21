# Vibe Coding Template - Agent Instructions

This is a modern full-stack application template with Next.js frontend and Python FastAPI backend, integrated with Supabase for authentication, database, and storage.

## Architecture Overview

- **Backend**: Python FastAPI with Supabase integration
- **Frontend**: Next.js with Tailwind CSS and TypeScript
- **Database**: Supabase PostgreSQL with migrations
- **Vector DB**: Qdrant for semantic search
- **LLM Integration**: OpenAI and Anthropic support

## Development Standards

### Code Style
- Use TypeScript for all frontend files
- Use Python type hints for all backend functions
- Follow async/await patterns consistently
- Use snake_case for Python, camelCase for TypeScript
- Include proper error handling in all functions

### Architecture Patterns
- Follow the service layer pattern for external integrations
- Use Pydantic models for API request/response validation
- Implement proper authentication on all protected endpoints
- Use the generic SupabaseDatabaseService for database operations
- Abstract LLM providers through service classes

### File Organization
- Backend: `backend/app/` with api/, models/, services/ subdirectories
- Frontend: `frontend/` with app/, components/, services/ subdirectories
- Database: `supabase/migrations/` for all schema changes
- Rules: `.cursor/rules/` for detailed development guidelines

## Common Patterns

### FastAPI Endpoints
```python
@router.post("/items", response_model=ItemResponse)
async def create_item(
    request: CreateItemRequest,
    current_user: User = Depends(get_current_user)
) -> ItemResponse:
    try:
        # Use service layer
        service = SupabaseDatabaseService("items", ItemResponse)
        result = await service.create({**request.dict(), "user_id": current_user.id})
        return result
    except Exception as e:
        raise HTTPException(status_code=400, detail=str(e))
```

### React Components
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
4. **Testing**: Visit http://localhost:8000/docs for API testing
5. **Frontend**: Visit http://localhost:3000 for the application

## Key Services

- **SupabaseDatabaseService**: Generic CRUD operations
- **SupabaseAuthService**: User authentication and token management
- **SupabaseStorageService**: File upload and management
- **LLMService**: Text generation with OpenAI/Anthropic
- **EmbeddingService**: Vector embeddings for semantic search
- **QdrantService**: Vector database operations

## Environment Configuration

Required environment variables:
- `SUPABASE_URL` and `SUPABASE_SERVICE_KEY` (required)
- `OPENAI_API_KEY` and/or `ANTHROPIC_API_KEY` (for LLM features)
- `QDRANT_URL` and `QDRANT_API_KEY` (for vector database)

## Best Practices

- Always use the service layer for external API calls
- Implement proper error handling with descriptive messages
- Use authentication dependencies on protected endpoints
- Follow the established patterns for consistency
- Test API endpoints using the FastAPI docs interface
- Use database migrations for all schema changes
- Implement proper RLS policies for data security

When adding new features, follow the established patterns and maintain consistency with the existing codebase structure.

## Cursor Cloud specific instructions

### Python version
The backend requires Python 3.11 (not 3.12) because `qdrant-client==1.5.*` in `backend/requirements.txt` needs Python <3.12. A virtual environment is set up at `backend/.venv` using Python 3.11. Always activate it before running backend commands: `source /workspace/backend/.venv/bin/activate`.

### Running services natively (without Docker)

**Backend** (port 8000):
```bash
source /workspace/backend/.venv/bin/activate
cd /workspace/backend
PYTHONPATH=/workspace/backend uvicorn app.main:app --host 0.0.0.0 --port 8000 --reload
```
Requires `SUPABASE_URL` and `SUPABASE_SERVICE_KEY` env vars (or `.env` at repo root). These have no defaults and the app will crash at import time without them.

**Frontend** (port 3000):
```bash
cd /workspace/frontend
npm run dev
```
Needs `frontend/.env.local` with `NEXT_PUBLIC_SUPABASE_URL`, `NEXT_PUBLIC_SUPABASE_ANON_KEY`, and `NEXT_PUBLIC_API_URL`.

### Lint
- Frontend: `cd frontend && npx next lint` (uses `.eslintrc.json` with `next/core-web-vitals`)
- Backend: `source backend/.venv/bin/activate && cd backend && flake8 app --max-line-length=120`

### Key gotchas
- No lockfile exists for the frontend; `npm install` generates `package-lock.json` at install time.
- The root `.env` file is loaded by the backend's `pydantic-settings` (via `env_file = ".env"` in `Settings.Config`). When running the backend from `backend/` dir, env vars must be set explicitly or the `.env` must be in the CWD or passed via environment.
- Supabase clients are lazily instantiated in service class constructors, so the backend starts fine with placeholder Supabase credentials; actual Supabase calls will fail at request time.
- Qdrant gracefully falls back to in-memory mode when `QDRANT_URL` is empty.
