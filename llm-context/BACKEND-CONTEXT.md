# Backend Context (Rust Axum)

This document provides context for the Rust Axum backend.

## Technology Stack

- **Framework**: Axum 0.7 (async web framework)
- **Runtime**: Tokio (async runtime)
- **Serialization**: Serde (JSON serialization)
- **Validation**: Validator (request validation)
- **HTTP Client**: Reqwest (async HTTP client)
- **Logging**: Tracing (structured logging)
- **Error Handling**: thiserror + anyhow

## Project Structure

```
backend/
├── src/
│   ├── main.rs           # Application entry point
│   ├── lib.rs            # Library exports
│   ├── api/              # HTTP endpoint handlers
│   │   ├── mod.rs        # Router configuration
│   │   ├── auth.rs       # Authentication endpoints
│   │   ├── llm.rs        # LLM endpoints
│   │   └── vectordb.rs   # Vector database endpoints
│   ├── config/           # Configuration management
│   │   └── mod.rs        # Settings from environment
│   ├── models/           # Data models
│   │   ├── mod.rs        # Module exports
│   │   ├── auth.rs       # Auth-related types
│   │   ├── error.rs      # Error types
│   │   ├── llm.rs        # LLM types
│   │   └── vectordb.rs   # Vector DB types
│   └── services/         # Business logic
│       ├── mod.rs        # Module exports
│       ├── llm/          # LLM services
│       │   ├── mod.rs
│       │   ├── llm.rs    # Text generation
│       │   └── embedding.rs # Embeddings
│       ├── supabase/     # Supabase services
│       │   ├── mod.rs
│       │   ├── auth.rs   # Authentication
│       │   ├── database.rs # CRUD operations
│       │   └── storage.rs  # File storage
│       └── vectordb/     # Vector DB services
│           ├── mod.rs
│           └── qdrant.rs # Qdrant client
├── Cargo.toml            # Dependencies
├── Dockerfile            # Production image
└── Dockerfile.dev        # Development image
```

## Key Concepts

### Error Handling

All errors use the `AppError` enum:

```rust
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Authentication failed: {0}")]
    Unauthorized(String),
    
    #[error("Invalid request: {0}")]
    BadRequest(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("External service error: {0}")]
    ExternalService(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}
```

### Service Traits

Services use traits for abstraction:

```rust
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
```

### Configuration

Settings loaded from environment via `once_cell::Lazy`:

```rust
pub static SETTINGS: Lazy<Settings> = Lazy::new(|| {
    Settings::from_env().expect("Failed to load settings")
});
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Health check |
| GET | `/api/auth/me` | Get current user |
| POST | `/api/auth/provider-token` | Exchange OAuth token |
| POST | `/api/llm/generate` | Generate text |
| POST | `/api/llm/embedding` | Create embedding |
| POST | `/api/vectordb/documents` | Add documents |
| POST | `/api/vectordb/search` | Search documents |
| DELETE | `/api/vectordb/documents` | Delete documents |

## Environment Variables

Required:
- `SUPABASE_URL` - Supabase project URL
- `SUPABASE_SERVICE_KEY` - Supabase service role key

Optional:
- `ENVIRONMENT` - development/staging/production
- `HOST` - Server host (default: 0.0.0.0)
- `PORT` - Server port (default: 8000)
- `CORS_ORIGINS` - Comma-separated allowed origins
- `OPENAI_API_KEY` - For text generation
- `ANTHROPIC_API_KEY` - For text generation
- `QDRANT_URL` - Vector database URL
- `QDRANT_API_KEY` - Vector database API key
- `QDRANT_COLLECTION_NAME` - Default collection name

## Development Commands

```bash
# Build
cargo build --release

# Run
cargo run

# Run with hot reload
cargo watch -x run

# Test
cargo test

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt
```

## Key Dependencies

```toml
axum = "0.7"              # Web framework
tokio = "1.35"            # Async runtime
serde = "1.0"             # Serialization
reqwest = "0.12"          # HTTP client
validator = "0.18"        # Validation
thiserror = "1.0"         # Error handling
tracing = "0.1"           # Logging
qdrant-client = "1.10"    # Vector database
```

## Linting Rules

The project enforces strict linting:
- No `unsafe` code
- No `.unwrap()` or `.expect()`
- All clippy warnings as errors

## Why Rust?

1. **Compile-time correctness** - Bugs caught before runtime
2. **No null pointer exceptions** - `Option<T>` and `Result<T, E>`
3. **Explicit error handling** - Every error must be handled
4. **Zero-cost abstractions** - High-level code, low-level performance
5. **Memory safety** - No data races, no use-after-free
