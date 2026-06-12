#!/usr/bin/env bash
# Fail CI if any BPF instruction frame exceeds the 4096-byte stack limit.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

check_features() {
  local features="$1"
  echo "==> anchor build (BPF stack check, features=$features)"
  local build_log
  build_log="$(mktemp)"
  if ! anchor build --no-idl -- --features "$features" >"$build_log" 2>&1; then
    cat "$build_log"
    rm -f "$build_log"
    return 1
  fi
  local stack_errors
  stack_errors="$(grep -E 'Stack offset of [0-9]+ exceeded max offset of 4096' "$build_log" || true)"
  rm -f "$build_log"
  if [[ -n "$stack_errors" ]]; then
    echo "BPF stack limit violations detected (features=$features):"
    echo "$stack_errors"
    return 1
  fi
}

check_features prod-governance
check_features test-bypass

echo "==> BPF stack check passed (prod-governance + test-bypass)"
