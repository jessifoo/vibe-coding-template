# Code Standards and Guidelines

This document defines the coding standards for this project. All contributors (human and AI) must follow these guidelines.

## Table of Contents

1. [General Principles](#general-principles)
2. [Rust Backend Standards](#rust-backend-standards)
3. [TypeScript Frontend Standards](#typescript-frontend-standards)
4. [Database Standards](#database-standards)
5. [API Design Standards](#api-design-standards)
6. [Security Standards](#security-standards)
7. [Testing Standards](#testing-standards)
8. [Git Standards](#git-standards)

---

## General Principles

### The Golden Rules

1. **Type Safety First** - Never compromise on types
2. **Explicit Over Implicit** - Be clear about intent
3. **Handle All Errors** - No silent failures
4. **Keep It Simple** - Avoid over-engineering
5. **Document Intent** - Code says what, comments say why

### No Shortcuts Policy

The following are **strictly forbidden**:

| ❌ Forbidden | ✅ Required |
|-------------|-------------|
| `any` type in TypeScript | Explicit interfaces/types |
| `.unwrap()` in Rust (except in tests or main) | `?` operator or explicit match |
| `// @ts-ignore` | Fix the type error properly |
| Commented-out code | Delete or document why kept |
| Magic numbers | Named constants |
| Silent error swallowing | Log and handle appropriately |

---

## Rust Backend Standards

### File Structure

```
backend/src/
├── main.rs          # Entry point only
├── lib.rs           # Module exports
├── config/          # Configuration types and loading
├── models/          # Data structures and validation
├── services/        # Business logic (one service per external dependency)
├── api/             # HTTP handlers (thin layer, delegates to services)
└── utils/           # Shared utilities (if needed)
```

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Files | snake_case | `user_service.rs` |
| Modules | snake_case | `mod user_service;` |
| Types/Structs | PascalCase | `UserProfile` |
| Functions | snake_case | `get_user_by_id` |
| Constants | SCREAMING_SNAKE_CASE | `MAX_RETRY_COUNT` |
| Type parameters | Single uppercase | `T`, `E`, `R` |

### Error Handling

```rust
// Define specific error types
#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("User not found: {0}")]
    NotFound(String),
    
    #[error("Invalid email format: {0}")]
    InvalidEmail(String),
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

// Use Result<T, E> for all fallible operations
pub async fn get_user(id: &str) -> Result<User, UserError> {
    let user = db.find(id).await?;
    user.ok_or_else(|| UserError::NotFound(id.to_string()))
}
```

### Documentation Requirements

Every public item must have documentation:

```rust
/// Retrieves a user by their unique identifier.
///
/// # Arguments
///
/// * `id` - The UUID of the user to retrieve
///
/// # Returns
///
/// The user if found, or an error if not found or database failure.
///
/// # Errors
///
/// * `UserError::NotFound` - If no user exists with the given ID
/// * `UserError::Database` - If a database operation fails
pub async fn get_user(id: &str) -> Result<User, UserError> {
    // ...
}
```

### Required Lints

These are enforced in `Cargo.toml`:

```toml
[lints.rust]
unsafe_code = "forbid"

[lints.clippy]
all = "warn"
unwrap_used = "warn"
expect_used = "warn"
```

### Command Reference

```bash
cargo check          # Quick compilation check
cargo build          # Full build
cargo test           # Run tests
cargo clippy         # Run linter
cargo fmt            # Format code
cargo doc --open     # Generate and view docs
```

---

## TypeScript Frontend Standards

### File Structure

```
frontend/
├── app/             # Next.js App Router pages
├── components/      # Reusable React components
│   ├── ui/          # Generic UI components
│   └── features/    # Feature-specific components
├── services/        # API client functions
├── types/           # TypeScript type definitions
├── hooks/           # Custom React hooks
└── utils/           # Utility functions
```

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Files (components) | PascalCase | `UserProfile.tsx` |
| Files (utilities) | camelCase | `formatDate.ts` |
| Components | PascalCase | `export function UserProfile` |
| Functions | camelCase | `function formatDate()` |
| Constants | SCREAMING_SNAKE_CASE | `const MAX_ITEMS = 100` |
| Types/Interfaces | PascalCase | `interface UserProfile` |
| Props interfaces | ComponentNameProps | `interface UserProfileProps` |

### Component Standards

```tsx
// ✅ CORRECT: Full type definitions, proper structure
'use client';

import { useState } from 'react';

interface UserCardProps {
  userId: string;
  initialName: string;
  onUpdate?: (name: string) => void;
}

export function UserCard({ userId, initialName, onUpdate }: UserCardProps): JSX.Element {
  const [name, setName] = useState(initialName);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (): Promise<void> => {
    setIsLoading(true);
    setError(null);
    
    try {
      await updateUser(userId, name);
      onUpdate?.(name);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="p-4 rounded-lg border">
      {error && <p className="text-red-500">{error}</p>}
      {/* Component content */}
    </div>
  );
}
```

### Type Definition Standards

```typescript
// types/user.ts

// Use interfaces for object shapes
export interface User {
  id: string;
  email: string;
  fullName: string | null;
  createdAt: string;
}

// Use type for unions and computed types
export type UserRole = 'admin' | 'user' | 'guest';

// API response types
export interface ApiResponse<T> {
  data: T;
  error: string | null;
}

// Never use 'any' - use 'unknown' if type is truly unknown
export function parseJson(input: string): unknown {
  return JSON.parse(input);
}
```

### Required ESLint Rules

See `.eslintrc.json` for full configuration. Key rules:

- `@typescript-eslint/no-explicit-any`: error
- `@typescript-eslint/explicit-function-return-type`: warn
- `no-console`: warn (except warn/error)
- `eqeqeq`: error

---

## Database Standards

### Migration Naming

```
{timestamp}_{action}_{table}.sql

Examples:
20240315000000_create_users.sql
20240315000001_add_users_email_index.sql
20240316000000_alter_users_add_avatar.sql
```

### Table Design

```sql
-- Every table needs:
-- 1. UUID primary key
-- 2. Timestamps
-- 3. User reference (if applicable)
-- 4. RLS policies

CREATE TABLE public.items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Always enable RLS
ALTER TABLE public.items ENABLE ROW LEVEL SECURITY;

-- Define explicit policies
CREATE POLICY "Users can read own items"
    ON public.items FOR SELECT
    USING (auth.uid() = user_id);

CREATE POLICY "Users can insert own items"
    ON public.items FOR INSERT
    WITH CHECK (auth.uid() = user_id);

CREATE POLICY "Users can update own items"
    ON public.items FOR UPDATE
    USING (auth.uid() = user_id)
    WITH CHECK (auth.uid() = user_id);

CREATE POLICY "Users can delete own items"
    ON public.items FOR DELETE
    USING (auth.uid() = user_id);

-- Add indexes for common queries
CREATE INDEX idx_items_user_id ON public.items(user_id);
CREATE INDEX idx_items_created_at ON public.items(created_at DESC);
```

---

## API Design Standards

### URL Structure

```
GET    /api/resources          # List resources
POST   /api/resources          # Create resource
GET    /api/resources/:id      # Get single resource
PUT    /api/resources/:id      # Update resource (full)
PATCH  /api/resources/:id      # Update resource (partial)
DELETE /api/resources/:id      # Delete resource
```

### Response Format

```typescript
// Success response
{
  "data": { ... },
  "meta": {
    "page": 1,
    "limit": 10,
    "total": 100
  }
}

// Error response
{
  "error": "Human readable message",
  "code": "ERROR_CODE",
  "details": { ... }  // Optional additional context
}
```

### HTTP Status Codes

| Code | Usage |
|------|-------|
| 200 | Success (with body) |
| 201 | Created |
| 204 | Success (no body) |
| 400 | Bad request (validation error) |
| 401 | Unauthorized (not authenticated) |
| 403 | Forbidden (not authorized) |
| 404 | Not found |
| 422 | Unprocessable entity |
| 500 | Internal server error |

---

## Security Standards

### Authentication

- All protected endpoints must verify JWT tokens
- Tokens must be validated on every request
- Never trust client-side data for authorization

### Input Validation

- Validate all inputs at the API boundary
- Use allow-lists, not deny-lists
- Sanitize data before storage
- Use parameterized queries only

### Secrets Management

- Never commit secrets to git
- Use environment variables
- Rotate credentials regularly
- Use different credentials per environment

---

## Testing Standards

### Test File Location

- Place tests next to source files: `user.rs` → `user_test.rs`
- Or in `tests/` directory for integration tests

### Test Naming

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_user_returns_user_when_exists() { }

    #[test]
    fn get_user_returns_not_found_when_missing() { }

    #[test]
    fn create_user_fails_with_invalid_email() { }
}
```

### What to Test

- **Unit tests**: Business logic, utilities, transformations
- **Integration tests**: API endpoints, database operations
- **Not to test**: External services (mock them)

---

## Git Standards

### Commit Message Format

```
type(scope): description

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting (no code change)
- `refactor`: Code restructuring
- `test`: Adding tests
- `chore`: Maintenance

Examples:
```
feat(auth): add Google OAuth integration
fix(api): handle null user profile gracefully
docs(readme): update installation instructions
refactor(services): extract common HTTP client logic
```

### Branch Naming

```
feature/short-description
fix/issue-number-description
hotfix/critical-issue
docs/what-documenting
```

### Pull Request Requirements

- [ ] Descriptive title following commit format
- [ ] Description of changes
- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] Lints pass
- [ ] Build succeeds
- [ ] Self-reviewed code