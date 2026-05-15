#!/usr/bin/env bash
# Non-interactive check that a machine (e.g. a cloud agent) can build and test
# the full stack. Exits non-zero with a clear message if anything is missing.
#
# Usage (from repo root):
#   ./scripts/verify-agent-toolchain.sh
#
# Optional: set SUPABASE_URL / SUPABASE_SERVICE_KEY for stricter backend tests;
# otherwise placeholder values are used so `cargo check` / `cargo test` can run.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

err() {
  echo "error: $*" >&2
  exit 1
}

need_cmd() {
  local name="$1"
  local hint="$2"
  command -v "$name" >/dev/null 2>&1 || err "missing required command '${name}' (${hint})"
}

need_cmd git "https://git-scm.com/"
need_cmd curl "used for rustup installs and some tooling docs"

if ! command -v node >/dev/null 2>&1; then
  err "Node.js is not installed (install Node 20+ from https://nodejs.org/ or your OS package manager; see AGENTS.md provisioning)"
fi

if ! command -v npm >/dev/null 2>&1; then
  err "npm is not installed (it normally ships with Node.js)"
fi

node_major="$(node -p 'Number(process.version.slice(1).split(".")[0])')"
if [ "$node_major" -lt 20 ]; then
  err "Node.js $(node -v) is too old; install Node 20 or newer (see frontend/package.json \"engines\")"
fi

if ! command -v cargo >/dev/null 2>&1 || ! command -v rustc >/dev/null 2>&1; then
  err "Rust (cargo + rustc) not found; install from https://rustup.rs/ then re-run"
fi

if command -v rustup >/dev/null 2>&1; then
  echo "==> rustup: ensure rustfmt + clippy for backend/rust-toolchain.toml override"
  (cd "${ROOT}/backend" && rustup component add rustfmt clippy) || {
    echo "warning: rustup component add failed; ensure rustfmt and clippy are installed for the active toolchain" >&2
  }
else
  echo "warning: rustup not found; cannot install rustfmt/clippy automatically" >&2
fi

export SUPABASE_URL="${SUPABASE_URL:-https://placeholder.supabase.co}"
export SUPABASE_SERVICE_KEY="${SUPABASE_SERVICE_KEY:-placeholder}"

echo "==> frontend: npm ci"
(cd "${ROOT}/frontend" && npm ci)

echo "==> backend: cargo fmt (check only)"
(cd "${ROOT}/backend" && cargo fmt -- --check)

echo "==> backend: cargo clippy"
(cd "${ROOT}/backend" && cargo clippy --all-targets -- -D warnings)

echo "==> backend: cargo test"
(cd "${ROOT}/backend" && cargo test -q)

echo "==> frontend: npm run lint"
(cd "${ROOT}/frontend" && npm run lint)

echo "==> frontend: npm run build"
(cd "${ROOT}/frontend" && npm run build)

echo "verify-agent-toolchain: OK (Node $(node -v), $(rustc -V), $(cargo -V))"
