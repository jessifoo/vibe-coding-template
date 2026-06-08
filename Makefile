.PHONY: dev dev-frontend dev-backend prod prod-frontend prod-backend clean help
.PHONY: logs stop prod-logs prod-stop
.PHONY: db-migration-new db-apply db-list db-push db-status
.PHONY: install-frontend build-frontend install-backend build-backend test-backend lint-backend check-backend fmt-backend ci ci-full lint-frontend test-frontend verify-tracked
.PHONY: verify-agent-toolchain agent-verify
.PHONY: deny-backend machete-backend supply-chain-backend

# Default target
.DEFAULT_GOAL := help

# Colors for terminal output
GREEN=\033[0;32m
YELLOW=\033[0;33m
RED=\033[0;31m
NC=\033[0m # No Color

# Development environment
dev: ## Start the full development environment
	@echo "${GREEN}Starting full development environment...${NC}"
	docker-compose up

dev-frontend: ## Start only the frontend development server
	@echo "${GREEN}Starting frontend development server...${NC}"
	docker-compose up frontend

dev-backend: ## Start only the backend development server
	@echo "${GREEN}Starting backend development server...${NC}"
	docker-compose up backend

logs: ## Stream logs from all development services
	@echo "${GREEN}Streaming development logs...${NC}"
	docker-compose logs -f

stop: ## Stop all development services
	@echo "${YELLOW}Stopping development services...${NC}"
	docker-compose down

# Production environment
prod: ## Start the full production environment
	@echo "${GREEN}Starting full production environment...${NC}"
	docker-compose -f docker-compose.prod.yml up -d

prod-frontend: ## Start only the frontend production server
	@echo "${GREEN}Starting frontend production server...${NC}"
	docker-compose -f docker-compose.prod.yml up -d frontend

prod-backend: ## Start only the backend production server
	@echo "${GREEN}Starting backend production server...${NC}"
	docker-compose -f docker-compose.prod.yml up -d backend

prod-logs: ## Stream logs from production services
	@echo "${GREEN}Streaming production logs...${NC}"
	docker-compose -f docker-compose.prod.yml logs -f

prod-stop: ## Stop all production services
	@echo "${YELLOW}Stopping production services...${NC}"
	docker-compose -f docker-compose.prod.yml down

# Clean up
clean: ## Remove containers and volumes
	@echo "${YELLOW}Cleaning up containers and volumes...${NC}"
	docker-compose down -v
	docker-compose -f docker-compose.prod.yml down -v

# Frontend helpers
install-frontend: ## Install frontend dependencies (npm ci when lockfile exists)
	@echo "${GREEN}Installing frontend dependencies...${NC}"
	cd frontend && if [ -f package-lock.json ]; then npm ci; else npm install; fi

build-frontend: ## Build frontend for production
	@echo "${GREEN}Building frontend for production...${NC}"
	cd frontend && npm run build

# Backend helpers (Rust)
# Installs globally to ~/.cargo/bin (standard for Rust CLI tools). Ensure that directory is on PATH.
# `cargo-deny` and `cargo-machete` are the tier-2 supply-chain gates wired into `make supply-chain-backend`.
install-backend: ## Install backend development tools
	@echo "${GREEN}Installing backend development tools...${NC}"
	cd backend && cargo install --locked cargo-watch cargo-deny cargo-machete

build-backend: ## Build backend in release mode
	@echo "${GREEN}Building backend for production...${NC}"
	cd backend && cargo build --release

test-backend: ## Run backend tests
	@echo "${GREEN}Running backend tests...${NC}"
	cd backend && cargo test

lint-backend: ## Run backend linter (clippy)
	@echo "${GREEN}Running backend linter...${NC}"
	cd backend && cargo clippy -- -D warnings

check-backend: ## Check backend compiles
	@echo "${GREEN}Checking backend...${NC}"
	cd backend && cargo check

fmt-backend: ## Format backend code
	@echo "${GREEN}Formatting backend code...${NC}"
	cd backend && cargo fmt

# Tier-2 AI-guardrail supply-chain gates (see ARCHITECTURE_PLAN.md §14).
# Each target soft-fails with a clear message if the tool is not installed so
# `make agent-verify` still runs on minimal toolchains.
deny-backend: ## Run cargo-deny (license, advisory, source, bans)
	@echo "${GREEN}Running cargo deny check...${NC}"
	@if command -v cargo-deny >/dev/null 2>&1; then \
		cd backend && cargo deny check; \
	else \
		echo "${YELLOW}cargo-deny not installed; skipping. Install: cargo install --locked cargo-deny${NC}"; \
	fi

machete-backend: ## Run cargo-machete (unused dependency detection)
	@echo "${GREEN}Running cargo machete...${NC}"
	@if command -v cargo-machete >/dev/null 2>&1; then \
		cd backend && cargo machete; \
	else \
		echo "${YELLOW}cargo-machete not installed; skipping. Install: cargo install --locked cargo-machete${NC}"; \
	fi

supply-chain-backend: deny-backend machete-backend ## Run all backend supply-chain checks
	@echo "${GREEN}Backend supply-chain checks complete.${NC}"

lint-frontend: ## Run frontend ESLint
	@echo "${GREEN}Linting frontend...${NC}"
	cd frontend && npm run lint

test-frontend: ## Run frontend typecheck (Next.js build includes typecheck)
	@echo "${GREEN}Typechecking/building frontend...${NC}"
	cd frontend && npm run build

# Supabase database migrations (all for remote database)
db-migration-new: ## Create a new migration file (Usage: make db-migration-new name=create_users_table)
	@echo "${GREEN}Creating new migration file: $(name)${NC}"
	supabase migration new $(name)

db-apply: ## Apply pending migrations to the remote database
	@echo "${GREEN}Applying pending migrations to remote database...${NC}"
	supabase db push

db-list: ## List all applied migrations on the remote database
	@echo "${GREEN}Listing applied migrations on remote database...${NC}"
	supabase migration list

db-push: ## Push migrations to remote Supabase project (same as db-apply)
	@echo "${GREEN}Pushing migrations to remote project...${NC}"
	supabase db push

db-status: ## Show pending migrations status
	@echo "${GREEN}Checking migration status...${NC}"
	@echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
	@echo "${YELLOW}Migration status (via supabase migration list):${NC}"
	@supabase migration list || echo "  Failed to get migration status"
	@echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
	@echo "${YELLOW}Migration files in project:${NC}"
	@ls -1 supabase/migrations/*.sql 2>/dev/null | sed 's/.*\//  /' || echo "  None"

# CI helpers
# `ci` runs backend checks only (fast path for API work). Use `ci-full` for full stack.
ci: lint-backend test-backend ## Run backend CI (lint + test)
	@echo "${GREEN}Backend CI checks passed!${NC}"

ci-full: lint-backend test-backend lint-frontend test-frontend ## Run full-stack CI
	@echo "${GREEN}Full CI checks passed!${NC}"

# One-shot verification for cloud agents / fresh VMs (Node 20+, Rust, npm ci, fmt, clippy, tests, frontend build)
verify-agent-toolchain: ## Verify toolchain and run full build+test (see scripts/verify-agent-toolchain.sh)
	@chmod +x scripts/verify-agent-toolchain.sh 2>/dev/null || true
	@./scripts/verify-agent-toolchain.sh

agent-verify: verify-agent-toolchain ## Full non-interactive build+test gate for agents (see scripts/verify-agent-toolchain.sh)
	@echo "${GREEN}agent-verify: OK${NC}"

# Fail if build artifacts or dependencies were accidentally `git add`ed
verify-tracked: ## Ensure git does not track node_modules, target/, .next, etc.
	@bad=$$(git ls-files | grep -E 'node_modules/|/target/|/\.next/|__pycache__/|(^|/)\.env$$|(^|/)\.env\.(local|development|test|production)' || true); \
	if [ -n "$$bad" ]; then \
		echo "${RED}These paths are tracked but must stay ignored (see .gitignore):${NC}"; \
		echo "$$bad"; \
		echo "Run: git rm -r --cached <path>   # then commit"; \
		exit 1; \
	fi; \
	n=$$(git ls-files | wc -l); \
	echo "${GREEN}verify-tracked: OK ($${n} files).${NC}"

# Help command
help: ## Show this help
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'