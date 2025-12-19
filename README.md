# Full Stack Vibe Coding Template

A modern, modular full-stack application starter template with **Next.js frontend** and **Rust Axum backend**, integrated with Supabase for maximum type safety and guardrails.

**Why Rust?** The Rust compiler is the ultimate code reviewer. It catches errors at compile time that other languages only discover in production. No `any` types, no null pointer exceptions, no race conditions - the code either compiles correctly or doesn't compile at all.

Contains all the common boilerplate features. Just add the README.md and CONTEXT.md files to AI coding agent's context.

Don't waste your time and tokens on boilerplate code. Use it to build your app.

## 🤖 AI-Powered Development with Maximum Guardrails

This template includes comprehensive **Cursor Rules** and **Agent Instructions** to supercharge your AI-assisted development:

### Why Rust Backend?

- **🛡️ Compiler as Guardian** - The Rust compiler catches bugs before they happen
- **🔒 No Null Pointer Exceptions** - Uses `Option<T>` and `Result<T, E>` for explicit handling
- **⚡ Zero-Cost Abstractions** - High-level code with low-level performance
- **🧵 Fearless Concurrency** - The borrow checker prevents data races at compile time
- **📝 Self-Documenting Types** - The type system serves as documentation
- **🚫 No Escape Hatches** - Unlike TypeScript's `any`, Rust forces correctness

### Cursor Rules (`.cursor/rules/`)
- **Context-aware guidance** that automatically applies based on the files you're editing
- **Template system** with production-ready code patterns (`@api-endpoint-template`, `@react-component-template`)
- **Best practices enforcement** for Axum, Next.js, Supabase, and LLM integration
- **Automatic rule application** - no manual setup required

### AGENTS.md
- **Simplified instructions** for AI coding assistants
- **Project patterns** and common code examples
- **Architecture overview** and development standards
- **Quick reference** for established patterns

### Benefits
- ⚡ **Faster Development** - Templates and patterns accelerate coding
- 🎯 **Consistency** - All code follows established patterns
- 🛡️ **Quality** - Built-in best practices and error handling
- 📚 **Learning** - New developers quickly understand project structure
- 🤖 **AI-Optimized** - Designed specifically for AI coding assistants

## Features

### Backend (Rust Axum)
- **Axum REST API** - Fast, type-safe API development with async/await
- **Strong Type Safety** - Compiler-enforced correctness
- **Supabase Integration**
  - Authentication (Google, LinkedIn, Email/Password)
  - Database connectivity via REST API
  - Storage management
  - Database migrations
- **LLM Integration**
  - OpenAI and Claude support
  - Abstracted LLM service traits
  - Vector embeddings service
- **Vector Database**
  - Qdrant integration
  - Document storage and semantic search
  - Automatic fallback to local database

### Frontend (Next.js)
- **Next.js** - React framework with routing, SSR, and more
- **TypeScript** - Type-safe frontend development
- **Tailwind CSS** - Utility-first CSS framework
- **Responsive design** - Mobile-first approach
- **Supabase client** - For auth and data access
- **Complete auth flows** - Login, signup, password reset

## Getting Started

### Prerequisites
- Docker and Docker Compose
- Make
- Node.js 18+ (for local frontend development)
- Rust 1.75+ (for local backend development)
- Supabase CLI (for database migrations, install with `brew install supabase/tap/supabase` or see [Supabase CLI docs](https://supabase.com/docs/guides/cli))

### Quick Start

1. Clone this repository:
   ```bash
   git clone https://github.com/humanstack/vibe-coding-boilerplate
   cd vibe-coding-boilerplate
   ```

2. Run the first-time setup script to configure your environment:
   ```bash
   ./first-time.sh
   ```
   This will:
   - Check for required tools
   - Guide you through setting up API keys
   - Generate the necessary .env files

3. Start the development environment:
   ```bash
   make dev
   ```

4. Access the applications:
   - Frontend: http://localhost:3000
   - Backend API: http://localhost:8000
   - Health Check: http://localhost:8000/

## Setup Without Script

If you prefer to set up manually:

1. Copy the `.env.example` file to `.env`:
   ```bash
   cp .env.example .env
   ```

2. Create a frontend environment file:
   ```bash
   cp .env.example frontend/.env.local
   ```

3. Edit both files to add your API keys for:
   - Supabase (required for auth)
   - OpenAI and/or Anthropic (for LLM features)
   - Qdrant (for vector database features, optional)

4. Start the development environment:
   ```bash
   make dev
   ```

## Authentication Setup

For detailed instructions on setting up authentication providers (Google, LinkedIn, GitHub, etc.), see the [Authentication Setup Guide](./AuthSetup.md).

## Structure

```
/
├── .cursor/                  # Cursor AI configuration
│   └── rules/                # Cursor rules for AI assistance
│       ├── backend/          # Backend-specific rules (Rust/Axum)
│       ├── frontend/         # Frontend-specific rules
│       └── templates/        # Code templates
├── AGENTS.md                 # AI agent instructions
│
├── backend/                  # Rust Axum application
│   ├── src/                  # Application source code
│   │   ├── api/              # API endpoint handlers
│   │   ├── config/           # Configuration management
│   │   ├── models/           # Data models (serde)
│   │   └── services/         # Service layer
│   │       ├── llm/          # LLM services
│   │       ├── supabase/     # Supabase services
│   │       └── vectordb/     # Vector DB services
│   ├── Cargo.toml            # Rust dependencies
│   ├── Dockerfile            # Production container
│   └── Dockerfile.dev        # Development container
│
├── frontend/                 # Next.js application
│   ├── app/                  # Next.js app directory
│   ├── components/           # UI components
│   ├── services/             # API services
│
├── supabase/                 # Supabase configuration
│   ├── migrations/           # Database migrations
│   ├── seed.sql              # Database seed data
│   └── README.md             # Migrations documentation
│
├── docker-compose.yml        # Docker configuration
├── Makefile                  # Project commands
├── first-time.sh             # Setup script
├── .gitignore                # Git ignore patterns
├── .env.example              # Example environment variables
├── CHANGELOG.md              # Project changelog
└── FutureImprovements.md     # Future feature roadmap
```

## Common Tasks

### Development

- Start all services: `make dev`
- Frontend only: `make dev-frontend`
- Backend only: `make dev-backend`

### Backend Development (Rust)

- Build release: `make build-backend`
- Run tests: `make test-backend`
- Run linter: `make lint-backend`
- Check compilation: `make check-backend`
- Format code: `make fmt-backend`

### Production

- Start production services: `make prod`
- Frontend only: `make prod-frontend`
- Backend only: `make prod-backend`

### Cleanup

- Clean up containers: `make clean`

### Database Migrations

- Create a migration: `make db-migration-new name=create_table`
- Apply migrations to remote: `make db-apply`
- List applied migrations: `make db-list`
- Check pending migrations: `make db-status`
- Push migrations (same as apply): `make db-push`

See `supabase/README.md` for more details on database migrations.

## AI Development Support

### Using Cursor Rules
The project includes comprehensive Cursor rules that automatically provide context-aware guidance:

- **Automatic Application**: Rules apply automatically based on the files you're editing
- **Template Usage**: Reference templates with `@api-endpoint-template`, `@react-component-template`, `@service-class-template`
- **Best Practices**: Built-in patterns for Axum, Next.js, Supabase, and LLM integration

### Using AGENTS.md
For simpler AI assistance, use the consolidated `AGENTS.md` file that provides:
- Project overview and architecture
- Common patterns and examples
- Development standards and workflows

## API Endpoints

### Health Check
- `GET /` - Returns server status

### Authentication (`/api/auth`)
- `GET /api/auth/me` - Get current user profile
- `POST /api/auth/provider-token` - Exchange provider token for Supabase token

### LLM (`/api/llm`)
- `POST /api/llm/generate` - Generate text with OpenAI/Anthropic
- `POST /api/llm/embedding` - Create text embedding

### Vector Database (`/api/vectordb`)
- `POST /api/vectordb/documents` - Add documents to vector DB
- `POST /api/vectordb/search` - Semantic search
- `DELETE /api/vectordb/documents` - Delete documents

## Documentation

### Guidelines & Standards
- [Code Standards](./CODE_STANDARDS.md) - Comprehensive coding standards
- [AI Agent Instructions](./AGENTS.md) - Guidelines for AI coding assistants
- [Cursor Rules Guide](./.cursor/rules/README.md) - Context-aware AI rules

### Project Docs
- [Authentication Setup Guide](./AuthSetup.md)
- [Database Migrations](./supabase/README.md)
- [Project Changelog](./CHANGELOG.md)
- [Future Improvements](./FutureImprovements.md)

## Linting & Formatting

### Backend (Rust)
```bash
cd backend
cargo check      # Quick compilation check
cargo clippy     # Linting (must pass with no warnings)
cargo fmt        # Format code
cargo test       # Run tests
```

### Frontend (TypeScript)
```bash
cd frontend
npm run lint     # ESLint (must pass)
npm run build    # Type check + build
```

## License

MIT
