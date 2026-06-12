#!/usr/bin/env bash
# Snapshot deployed program binaries for rollback (run before every mainnet upgrade).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

STAMP="$(date +%Y%m%d-%H%M%S)"
OUT="$ROOT/artifacts/rollback"
mkdir -p "$OUT"

declare -A PROGRAMS=(
  [defai_swap]=DB9Zvhdp5xh853d2Tr2HBkRDDaCSioD7vwchhcGaXCw3
  [defai_staking]=2TLhCW35y5jcuoKtfwTx7H5EPMqUtCf3UQhYKdKKg3Hq
  [defai_estate]=HvyyPrXbrhNEiGhttDUGMsYjKDPkYER2uFaLo7Bkei92
)

RPC="${SOLANA_URL:-$(solana config get 2>/dev/null | awk '/RPC URL/ {print $3}')}"

for name in defai_swap defai_staking defai_estate; do
  id="${PROGRAMS[$name]}"
  dest="$OUT/${name}-${STAMP}.so"
  echo "==> Dumping $name ($id) -> $dest"
  solana program dump "$id" "$dest" ${RPC:+--url "$RPC"}
done

echo "==> Rollback snapshots saved under $OUT"
