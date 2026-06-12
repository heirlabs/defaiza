#!/usr/bin/env bash
# Roll back a deployed Solana program to a previously saved buffer artifact.
#
# Usage:
#   ./scripts/rollback-program.sh <program-name> [buffer-keypair.json]
#
# Program names: defai_swap | defai_staking | defai_estate
#
# Prerequisites:
#   - solana CLI configured for target cluster (SOLANA_URL or solana config)
#   - Upgrade authority keypair in SOLANA_KEYPAIR or ~/.config/solana/id.json
#   - Prior .so saved under artifacts/rollback/<program-name>-<slot>.so
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PROGRAM="${1:-}"
BUFFER_KP="${2:-}"

usage() {
  echo "Usage: $0 <defai_swap|defai_staking|defai_estate> [buffer-keypair.json]"
  exit 1
}

[[ -n "$PROGRAM" ]] || usage

declare -A PROGRAM_IDS=(
  [defai_swap]="DB9Zvhdp5xh853d2Tr2HBkRDDaCSioD7vwchhcGaXCw3"
  [defai_staking]="2TLhCW35y5jcuoKtfwTx7H5EPMqUtCf3UQhYKdKKg3Hq"
  [defai_estate]="HvyyPrXbrhNEiGhttDUGMsYjKDPkYER2uFaLo7Bkei92"
)

PROGRAM_ID="${PROGRAM_IDS[$PROGRAM]:-}"
[[ -n "$PROGRAM_ID" ]] || { echo "Unknown program: $PROGRAM"; usage; }

ROLLBACK_DIR="$ROOT/artifacts/rollback"
LATEST_SO="$(ls -t "$ROLLBACK_DIR/${PROGRAM}-"*.so 2>/dev/null | head -1 || true)"
if [[ -z "$LATEST_SO" ]]; then
  echo "No rollback artifact found in $ROLLBACK_DIR/${PROGRAM}-*.so"
  echo "Save current binary before deploy:"
  echo "  solana program dump $PROGRAM_ID artifacts/rollback/${PROGRAM}-\$(date +%Y%m%d).so"
  exit 1
fi

echo "Rolling back $PROGRAM ($PROGRAM_ID)"
echo "  artifact: $LATEST_SO"
echo "  cluster:  $(solana config get | grep 'RPC URL' || true)"

if [[ -n "$BUFFER_KP" ]]; then
  BUFFER_PUBKEY="$(solana-keygen pubkey "$BUFFER_KP")"
else
  BUFFER_KP="$(mktemp /tmp/defai-buffer-XXXX.json)"
  solana-keygen new --no-bip39-passphrase -o "$BUFFER_KP" --force >/dev/null
  BUFFER_PUBKEY="$(solana-keygen pubkey "$BUFFER_KP")"
  echo "  ephemeral buffer: $BUFFER_PUBKEY"
fi

echo "==> Writing buffer"
solana program write-buffer "$LATEST_SO" --buffer "$BUFFER_KP"

echo "==> Upgrading program from buffer"
solana program deploy --program-id "$PROGRAM_ID" --buffer "$BUFFER_PUBKEY"

echo "==> Rollback complete for $PROGRAM"
solana program show "$PROGRAM_ID"
