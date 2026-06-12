# Out-of-Scope Findings (BlockerCrush PR)

Issues observed during blocker remediation but intentionally not fixed in this PR.

| ID | Severity | Finding | Location |
|----|----------|---------|----------|
| OOS-01 | Medium | `initialize_collection` / `initialize_whitelist` not gated by protocol governance | `conversion/src/lib.rs` |
| OOS-02 | Medium | Switchboard reveal validates owner only, not full account layout | `conversion/src/randomness_v2.rs` |
| OOS-03 | Medium | BPF stack warnings on large account structs — **mitigated in this PR** via `Box<Account>` + account trimming; CI gate in `scripts/check-bpf-stack.sh` | `conversion/src/lib.rs`, `estate/src/lib.rs`, `scripts/check-bpf-stack.sh` |
| OOS-04 | Low | Anchor IDL generation fails on host (anchor-syn 0.30 + proc_macro2 `source_file` API); use `anchor build --no-idl` until toolchain catches up | CI / `scripts/ci.sh` |
| OOS-05 | Low | Non-VRF randomness path when `vrf_enabled=false` | `conversion/src/lib.rs` |
| OOS-06 | Low | Mirror drift at `site/heir-front/external/defai/` | separate tree |
| OOS-07 | Info | Legacy EVM/Hardhat artifacts under `estate/` | not Solana BPF |
