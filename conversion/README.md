# DEFAI Swap Program

A Solana program that enables token swapping with tiered NFT bonuses, vesting mechanisms, and special support for OG holders.

## Overview

The DEFAI Swap program allows users to:
- Swap DEFAI tokens for tiered NFTs with random bonuses
- Claim vested tokens over a 90-day period
- Special OG Tier 0 access for whitelisted holders with 10:1 vesting
- Progressive tax system to prevent gaming
- Emergency controls and multi-level security

## Features

### 1. Token Swapping
- **5 Tiers of NFTs**: Each with different price points and bonus ranges
  - Tier 0 (OG): Free mint for whitelisted holders, no bonus
  - Tier 1 (Train): 0-15% bonus
  - Tier 2 (Boat): 15-50% bonus
  - Tier 3 (Plane): 20-100% bonus
  - Tier 4 (Rocket): 50-300% bonus

### 2. Vesting System
- 90-day linear vesting period
- 2-day cliff period before claims
- Support for both NFT-based and airdrop vesting

### 3. Tax Mechanism
- Progressive tax starting at 5%
- Increases by 1% per swap (max 30%)
- Resets after 24 hours of inactivity

### 4. Special Features
- **OG Tier 0**: Merkle proof-based whitelist for original holders
- **10:1 Airdrop**: Separate vesting for airdrop recipients (no NFT)
- **Reroll Mechanism**: Users can reroll their bonus for a tax fee
- **VRF Support**: Optional integration with Switchboard VRF for true randomness

## Build Instructions

```bash
# Ensure you're in the security-auditor directory
cd security-auditor

# Build the program
anchor build --skip-lint

# The built program will be at:
# target/deploy/defai_swap.so
```

## Program Addresses

- **Program ID**: `FxtwFmgibGqiiSQgpXy34eoDYjbTaXTCsvpvpzX2VReA`
- **Localnet**: `2rpNRpFnZEbb9Xoonieg7THkKnYEQhZoSK8bKNtVaVLS`

## Initialization

The program requires initialization in the following order:

1. **Initialize Main Config**
   ```typescript
   await program.methods.initialize([
     price_tier1,  // e.g., 10000 * 10^6 for 10k DEFAI
     price_tier2,  // e.g., 20000 * 10^6
     price_tier3,  // e.g., 30000 * 10^6
     price_tier4,  // e.g., 40000 * 10^6
     price_tier5   // e.g., 50000 * 10^6
   ])
   ```

2. **Initialize Collection**
   ```typescript
   await program.methods.initializeCollection(
     tierNames,     // ["OG", "Train", "Boat", "Plane", "Rocket"]
     tierSymbols,   // ["OG", "TRN", "BOT", "PLN", "RKT"]
     tierPrices,    // [0, 10000, 20000, 30000, 50000] * 10^6
     tierSupplies,  // [1000, 2000, 1500, 1000, 500]
     tierUriPrefixes, // IPFS URIs for each tier
     ogTier0MerkleRoot,  // Merkle root for OG holders
     airdropMerkleRoot   // Merkle root for 10:1 airdrop
   )
   ```

3. **Initialize User Tax State** (per user)
   ```typescript
   await program.methods.initializeUserTax()
   ```

## Key Constants

```rust
// Tax Configuration
const INITIAL_TAX_BPS: u16 = 500;     // 5%
const TAX_INCREMENT_BPS: u16 = 100;   // 1% increment
const TAX_CAP_BPS: u16 = 3000;        // 30% maximum

// Vesting Configuration  
const VESTING_DURATION: i64 = 90 * 24 * 60 * 60;  // 90 days
const CLIFF_DURATION: i64 = 2 * 24 * 60 * 60;     // 2 days

// Admin Timelock
const ADMIN_TIMELOCK_DURATION: i64 = 48 * 60 * 60; // 48 hours
```

## Usage Examples

### Swap DEFAI for NFT
```typescript
await program.methods.swapDefaiForPnftV6(
  tier,           // 0-4
  metadataUri,
  name,
  symbol
)
```

### OG Tier 0 Claim
```typescript
await program.methods.swapOgTier0ForPnftV6(
  vestingAmount,  // From merkle proof
  merkleProof,    // Proof array
  metadataUri,
  name,
  symbol
)
```

### Claim Vested Tokens
```typescript
await program.methods.claimVestedV6()
```

### Claim Airdrop (10:1)
```typescript
await program.methods.claimAirdrop(
  amount,         // AIRDROP amount from merkle proof
  merkleProof
)
```

## Security Features

1. **Merkle Proof Verification**: Ensures only authorized users can claim OG/airdrop tokens
2. **Progressive Tax**: Prevents swap spamming
3. **Timelock**: 48-hour delay for admin actions
4. **Pause Mechanism**: Emergency protocol pause
5. **Secure Randomness**: Multiple entropy sources for bonus generation

## Error Codes

- `InsufficientOldTokens`: Not enough OLD tokens provided
- `InsufficientDefaiTokens`: Not enough DEFAI tokens provided
- `NoLiquidity`: Tier supply exhausted
- `InvalidTier`: Invalid tier specified
- `NotOnOgWhitelist`: User not on OG whitelist
- `OgTier0AlreadyClaimed`: OG NFT already claimed
- `StillInCliff`: Vesting cliff period not over
- `NothingToClaim`: No vested tokens to claim

## Events

- `SwapExecuted`: Emitted when a swap is completed
- `VestingClaimed`: Emitted when vested tokens are claimed
- `RedemptionExecuted`: Emitted when NFT is redeemed
- `BonusRerolled`: Emitted when bonus is rerolled
- `AdminAction`: Emitted for admin operations 