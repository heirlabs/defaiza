#!/usr/bin/env bash
# Health check for DEFAI on-chain programs. Exits non-zero if any program is missing or not executable.
#
# Usage:
#   SOLANA_URL=https://api.mainnet-beta.solana.com ./scripts/monitor-programs.sh
#   ./scripts/monitor-programs.sh --json
#
set -euo pipefail

JSON=false
if [[ "${1:-}" == "--json" ]]; then
  JSON=true
fi

declare -A PROGRAMS=(
  [defai_swap]=DB9Zvhdp5xh853d2Tr2HBkRDDaCSioD7vwchhcGaXCw3
  [defai_staking]=2TLhCW35y5jcuoKtfwTx7H5EPMqUtCf3UQhYKdKKg3Hq
  [defai_estate]=HvyyPrXbrhNEiGhttDUGMsYjKDPkYER2uFaLo7Bkei92
)

RPC="${SOLANA_URL:-${DEFAI_MONITOR_RPC:-https://api.devnet.solana.com}}"
FAIL=0
RESULTS=()

for name in defai_swap defai_staking defai_estate; do
  id="${PROGRAMS[$name]}"
  if ! show="$(solana program show "$id" --url "$RPC" 2>&1)"; then
    FAIL=1
    RESULTS+=("$name|ERROR|$show")
    continue
  fi
  if echo "$show" | grep -q "Program Id: $id"; then
    slot="$(echo "$show" | awk '/Last Deployed In Slot/ {print $5}')"
    data_len="$(echo "$show" | awk '/Data Length/ {print $3}')"
    RESULTS+=("$name|OK|slot=$slot bytes=$data_len")
  else
    FAIL=1
    RESULTS+=("$name|ERROR|not executable")
  fi
done

if $JSON; then
  printf '{\n  "rpc": "%s",\n  "programs": [\n' "$RPC"
  first=true
  for row in "${RESULTS[@]}"; do
    IFS='|' read -r n status detail <<<"$row"
    $first || printf ',\n'
    first=false
    printf '    {"name":"%s","status":"%s","detail":"%s"}' "$n" "$status" "$detail"
  done
  printf '\n  ],\n  "healthy": %s\n}\n' "$([[ $FAIL -eq 0 ]] && echo true || echo false)"
else
  echo "RPC: $RPC"
  for row in "${RESULTS[@]}"; do
    IFS='|' read -r n status detail <<<"$row"
    echo "[$status] $n — $detail"
  done
fi

exit "$FAIL"
