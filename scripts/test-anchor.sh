#!/usr/bin/env bash
# Anchor integration tests require IDL JSON under target/idl/.
# IDL generation is blocked on anchor-syn 0.30.1 + proc_macro2 1.0.106 (OOS-04).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

IDL_DIR="$ROOT/target/idl"
REQUIRED=(defai_swap.json defai_estate.json defai_staking.json)

missing=()
for idl in "${REQUIRED[@]}"; do
  [[ -f "$IDL_DIR/$idl" ]] || missing+=("$idl")
done

if [[ ${#missing[@]} -gt 0 ]]; then
  echo "Anchor integration tests require IDL files that are not present:"
  printf '  - %s\n' "${missing[@]}"
  echo ""
  echo "IDL generation fails: anchor-syn 0.30.1 needs proc_macro2::Span::source_file()."
  echo "Use Rust unit tests instead: npm run test:unit"
  echo "Track: OUT_OF_SCOPE_FINDINGS.md OOS-04"
  exit 1
fi

anchor test --skip-build -- --features prod-governance
