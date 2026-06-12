# DEFAI Staking Program

A Solana program for staking DEFAI tokens with tiered APY rewards and sustainable tokenomics.

## Overview

The DEFAI Staking program enables users to:
- Stake DEFAI tokens in three tiers (Gold, Titanium, Infinite)
- Earn APY rewards based on stake amount
- Compound rewards to increase stake
- Time-locked withdrawals with penalty system

## Features

### 1. Staking Tiers
- **Gold Tier**: 10M - 99.99M DEFAI (0.5% APY)
- **Titanium Tier**: 100M - 999.99M DEFAI (0.75% APY)
- **Infinite Tier**: 1B+ DEFAI (1% APY)

### 2. Reward System
- Linear reward accrual based on tier APY
- Rewards funded through separate escrow account
- Compound functionality to reinvest rewards

### 3. Unstaking Rules
- 7-day initial lock period
- Early unstaking penalties:
  - < 30 days: 2% penalty
  - 30-90 days: 1% penalty
  - > 90 days: No penalty
- Penalties redistributed to reward escrow

### 4. Security Features
- 48-hour timelock for admin changes
- Program pause functionality
- Separate escrow for reward distribution

## Build Instructions

```bash
# Ensure you're in the security-auditor directory
cd security-auditor

# Build the program
anchor build --skip-lint

# The built program will be at:
# target/deploy/defai_staking.so
```

## Program Addresses

- **Program ID**: `DpAeweyqvHt7iuufYGoJC7oJXbpBNFgeDWCh2jKfwyWd`
- **Localnet**: `CyYfX3MjkuQBTpD8N3KLXBAr8Nik89f63FZ3jFVSMd6s`

## Initialization

The program requires initialization in the following order:

1. **Initialize Program State**
   ```typescript
   await program.methods.initializeProgram(
     defaiMint  // DEFAI token mint address
   )
   ```

2. **Initialize Reward Escrow**
   ```typescript
   await program.methods.initializeEscrow()
   ```

3. **Fund Reward Escrow**
   ```typescript
   await program.methods.fundEscrow(
     amount  // Amount of DEFAI tokens to add to escrow
   )
   ```

## Key Constants

```rust
// Tier Requirements (in DEFAI with 6 decimals)
pub const GOLD_MIN: u64 = 10_000_000 * 10^6;      // 10M DEFAI
pub const GOLD_MAX: u64 = 99_999_999 * 10^6;      // 99.99M DEFAI
pub const TITANIUM_MIN: u64 = 100_000_000 * 10^6; // 100M DEFAI
pub const TITANIUM_MAX: u64 = 999_999_999 * 10^6; // 999.99M DEFAI
pub const INFINITE_MIN: u64 = 1_000_000_000 * 10^6; // 1B DEFAI

// APY Rates (in basis points)
pub const GOLD_APY_BPS: u16 = 50;      // 0.5%
pub const TITANIUM_APY_BPS: u16 = 75;  // 0.75%
pub const INFINITE_APY_BPS: u16 = 100; // 1%

// Timelock
pub const ADMIN_TIMELOCK_DURATION: i64 = 48 * 60 * 60; // 48 hours
```

## Usage Examples

### Stake Tokens
```typescript
await program.methods.stakeTokens(
  new BN(50_000_000 * 10**6)  // Stake 50M DEFAI
)
```

### Claim Rewards
```typescript
await program.methods.claimRewards()
```

### Compound Rewards
```typescript
await program.methods.compoundRewards()
```

### Unstake Tokens
```typescript
await program.methods.unstakeTokens(
  new BN(20_000_000 * 10**6)  // Unstake 20M DEFAI
)
```

## Account Structure

### ProgramState
- Tracks global staking metrics
- Stores authority and mint information
- Manages pause state

### UserStake
- Individual staking position
- Tracks rewards earned and claimed
- Stores tier and lock information

### RewardEscrow
- Holds reward tokens for distribution
- Tracks total distributed rewards

## Security Features

1. **Time-locked Admin Actions**: 48-hour delay for critical changes
2. **Pause Mechanism**: Emergency pause for protocol protection
3. **Escrow System**: Separate reward pool prevents insolvency
4. **Lock Periods**: Prevents gaming through quick stake/unstake
5. **Penalty System**: Discourages short-term staking

## Error Codes

- `AmountTooLow`: Stake amount below minimum tier requirement
- `InsufficientStake`: Attempting to unstake more than staked
- `TokensLocked`: Tokens still in lock period
- `NoRewards`: No rewards available to claim
- `InvalidAuthority`: Unauthorized admin action
- `ProgramPaused`: Program is paused
- `InsufficientEscrowBalance`: Escrow lacks funds for rewards

## Events

- `StakeEvent`: Emitted when tokens are staked
- `UnstakeEvent`: Emitted when tokens are unstaked
- `RewardsClaimedEvent`: Emitted when rewards are claimed
- `RewardsCompoundedEvent`: Emitted when rewards are compounded
- `EscrowFundedEvent`: Emitted when escrow is funded
- `ProgramPausedEvent`: Emitted when program is paused/unpaused

## Admin Functions

1. **Update Authority**: Propose and accept authority changes (48h timelock)
2. **Update DEFAI Mint**: Change the accepted token mint
3. **Pause/Unpause**: Emergency controls for the program
4. **Fund Escrow**: Add rewards to the distribution pool 