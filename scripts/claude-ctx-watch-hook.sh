#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="${CLAUDE_PROJECT_DIR:-$(pwd)}"
CTX_BIN="${CTX_HOOK_CTX_BIN:-$PROJECT_ROOT/target/debug/ctx}"

if [ ! -x "$CTX_BIN" ]; then
  if command -v ctx >/dev/null 2>&1; then
    CTX_BIN="$(command -v ctx)"
  else
    exit 0
  fi
fi

if [ ! -d "$PROJECT_ROOT/.git" ] && [ ! -f "$PROJECT_ROOT/Cargo.toml" ]; then
  exit 0
fi

"$CTX_BIN" -p "$PROJECT_ROOT" ensure-watch --json >/dev/null 2>&1 || true
