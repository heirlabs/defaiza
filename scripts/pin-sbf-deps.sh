#!/usr/bin/env bash
# Pin transitive deps that require Rust edition2024 and break Solana SBF cargo (1.79–1.84).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo update -p borsh@1.6.1 --precise 1.5.7 2>/dev/null || cargo update -p borsh@1.5.7 --precise 1.5.7
cargo update -p proc-macro-crate@3.5.0 --precise 3.1.0 2>/dev/null || true
cargo update -p indexmap@2.14.0 --precise 2.11.4 2>/dev/null || cargo update -p indexmap@2.11.4 --precise 2.11.4 2>/dev/null || true

echo "SBF-compatible dependency pins applied."
