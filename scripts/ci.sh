#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

bash scripts/pin-sbf-deps.sh

echo "==> cargo fmt --check"
cargo fmt --all -- --check

echo "==> cargo clippy (workspace)"
cargo clippy --workspace --all-targets

echo "==> cargo test (common + staking + estate + swap unit tests)"
cargo test -p defai-protocol-common
cargo test -p defai-protocol-common --features dev-governance-bypass
cargo test -p DEFAI-staking
cargo test -p defai_estate
cargo test -p defai_swap

echo "==> cargo check (workspace, test-bypass)"
cargo check --workspace --features test-bypass

echo "==> anchor build + BPF stack limit gate"
bash scripts/check-bpf-stack.sh

echo "==> cargo audit (dependency security scan)"
if ! command -v cargo-audit >/dev/null 2>&1; then
  cargo install cargo-audit --version 0.22.1 --locked
fi
cargo audit

echo "==> All CI checks passed"
