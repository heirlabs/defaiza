# DEFAI Solana Programs — Deploy Assumptions

This documents what the production-readiness work actually verified vs what it assumes.

## Cluster and program IDs

- Program IDs in `Anchor.toml` are the same for localnet, devnet, and mainnet **slots**.
- As of the last health check, all three programs **exist on devnet only** (`solana program show` succeeds on devnet, fails on mainnet).
- Monitoring defaults to **devnet** (`scripts/monitor-programs.sh`, `.github/workflows/defai-programs-monitor.yml`). Set `SOLANA_URL` or `DEFAI_MONITOR_RPC` when mainnet is live.

## Build features

| Feature | Purpose | CI |
|---------|---------|-----|
| `prod-governance` | Default for deployment; enforces `PROTOCOL_GOVERNANCE` on gated inits | BPF stack gate |
| `test-bypass` | Lets unit/integration harness use non-governance signers | BPF stack gate + `cargo check` |

Production binaries must be built with **`prod-governance`** (default in program `Cargo.toml`).

## OG tier-0 swap (breaking client change)

`swap_og_tier0_for_pnft_v6` no longer uses `init_if_needed` on the OG claim PDA (BPF stack limit).

Clients must call **`init_og_tier0_claim`** once per user before the swap instruction.

## IDL and TypeScript tests

- `anchor build` (with IDL) **fails** on Rust 1.85 + proc_macro2 1.0.106 (OOS-04).
- CI uses `anchor build --no-idl`.
- `npm run test:anchor` → `scripts/test-anchor.sh` exits with a clear message if IDL is missing.
- **56 Rust unit tests** are the current integration substitute.

## Rollback

1. Before upgrade: `./scripts/snapshot-rollback-artifacts.sh` (requires `solana` CLI + RPC access).
2. To roll back: `./scripts/rollback-program.sh <defai_swap|defai_staking|defai_estate>`.

**Not verified end-to-end** in CI (requires live cluster + upgrade authority). Scripts are syntax-checked only.

## Dependency security

- `cargo audit` runs in CI with allowlist in `.cargo/audit.toml`.
- Allowlisted advisories are **transitive** from Anchor 0.30 / solana-program 1.18 and cannot be fixed without a toolchain upgrade.

## Known production risks (not fixed)

See `OUT_OF_SCOPE_FINDINGS.md`: ungated collection init (OOS-01), Switchboard validation depth (OOS-02), non-VRF randomness when `vrf_enabled=false` (OOS-05).

## Pre-deploy commands

```bash
bash scripts/ci.sh
SOLANA_URL=https://api.devnet.solana.com ./scripts/snapshot-rollback-artifacts.sh
# deploy .so from target/deploy/
SOLANA_URL=https://api.devnet.solana.com ./scripts/monitor-programs.sh
```
