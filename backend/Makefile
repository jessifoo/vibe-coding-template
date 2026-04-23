.PHONY: build dev test lint clean check fmt doc

# Default target
.DEFAULT_GOAL := help

# Colors for terminal output
GREEN=\033[0;32m
YELLOW=\033[0;33m
NC=\033[0m # No Color

build: ## Build the application in release mode
	@echo "${GREEN}Building release binary...${NC}"
	cargo build --release

dev: ## Run development server with hot reload
	@echo "${GREEN}Starting development server with hot reload...${NC}"
	cargo watch -x run

run: ## Run the application
	@echo "${GREEN}Running application...${NC}"
	cargo run

test: ## Run all tests
	@echo "${GREEN}Running tests...${NC}"
	cargo test

lint: ## Run clippy linter with all warnings
	@echo "${GREEN}Running clippy...${NC}"
	cargo clippy -- -D warnings

check: ## Check code compiles without building
	@echo "${GREEN}Checking code...${NC}"
	cargo check

fmt: ## Format code with rustfmt
	@echo "${GREEN}Formatting code...${NC}"
	cargo fmt

fmt-check: ## Check code formatting
	@echo "${GREEN}Checking formatting...${NC}"
	cargo fmt -- --check

doc: ## Generate documentation
	@echo "${GREEN}Generating documentation...${NC}"
	cargo doc --no-deps --open

clean: ## Clean build artifacts
	@echo "${YELLOW}Cleaning build artifacts...${NC}"
	cargo clean

audit: ## Run security audit on dependencies
	@echo "${GREEN}Running security audit...${NC}"
	cargo audit

# Development setup
setup: ## Install development tools
	@echo "${GREEN}Installing development tools...${NC}"
	cargo install cargo-watch
	cargo install cargo-audit
	cargo install cargo-edit

# All checks (for CI)
ci: fmt-check lint test audit ## Run all CI checks (incl. dependency audit)
	@echo "${GREEN}All CI checks passed!${NC}"

help: ## Show this help
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'