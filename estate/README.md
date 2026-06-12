# DEFAI Estate Program

A Solana program for digital estate management with inheritance planning, trading capabilities, and multi-signature support.

## Overview

The DEFAI Estate program provides:
- Digital estate creation with dead man's switch functionality
- Beneficiary management for inheritance distribution
- Real-world asset (RWA) tracking
- AI-powered trading capabilities
- Multi-signature support for enhanced security
- Emergency recovery mechanisms

## Features

### 1. Estate Management
- **Dead Man's Switch**: Automatic inheritance trigger after inactivity
- **Configurable Timers**: 
  - Inactivity period: 24 hours to 300 years
  - Grace period: 24 hours to 90 days
- **Check-in System**: Reset timer to prevent unwanted triggers
- **Asset Tracking**: SOL balances and RWA management

### 2. Beneficiary System
- Support for up to 10 beneficiaries
- Percentage-based inheritance distribution (must sum to 100%)
- Email hash storage for notifications
- Individual claim tracking for tokens and NFTs

### 3. Trading Features
- **AI Agent Integration**: Joint human-AI trading accounts
- **Profit Sharing**: Configurable split (50-100% for human)
- **Trading Strategies**: Conservative, Balanced, Aggressive
- **Emergency Withdrawal**: Time-delayed exit mechanism
- **Stop Loss**: Optional percentage-based protection

### 4. Multi-Signature Support
- Create multi-sig accounts with 2-10 signers
- Configurable approval threshold
- Proposal-based governance for estate actions
- 48-hour timelock for admin changes

### 5. Real-World Assets (RWA)
- Track off-chain assets (real estate, vehicles, jewelry, etc.)
- Metadata storage with IPFS URIs
- Soft-delete functionality
- Per-estate RWA numbering

## Build Instructions

```bash
# Ensure you're in the security-auditor directory
cd security-auditor

# Build the program
anchor build --skip-lint

# The built program will be at:
# target/deploy/defai_estate.so
```

## Program Addresses

- **Program ID**: `3WN7Eiq5pCGdoCXJW4jf8NygqPv8FzTvwXZArHtYFKYV`
- **Localnet**: `HYJe4U2DToJCjb5T8tysN4784twLUk48dUjPGD7dKYut`

## Initialization

### 1. Initialize Global Counter (One-time)
```typescript
await program.methods.initializeGlobalCounter()
```

### 2. Create Estate
```typescript
await program.methods.createEstate(
  inactivityPeriod,  // e.g., 365 * 24 * 60 * 60 (1 year)
  gracePeriod,       // e.g., 30 * 24 * 60 * 60 (30 days)
  ownerEmailHash     // SHA256 hash of owner's email
)
```

### 3. Update Beneficiaries
```typescript
await program.methods.updateBeneficiaries([
  {
    address: beneficiary1,
    emailHash: emailHash1,
    sharePercentage: 50,
    claimed: false,
    notificationSent: false
  },
  {
    address: beneficiary2,
    emailHash: emailHash2,
    sharePercentage: 50,
    claimed: false,
    notificationSent: false
  }
])
```

### 4. Enable Trading (Optional)
```typescript
await program.methods.enableTrading(
  aiAgent,            // AI agent pubkey
  humanShare,         // 50-100 (percentage)
  strategy,           // Conservative/Balanced/Aggressive
  stopLoss,           // Optional stop loss percentage
  emergencyDelayHours // 24-168 hours
)
```

## Key Constants

```rust
// Estate Limits
pub const MIN_INACTIVITY_PERIOD: i64 = 24 * 60 * 60;       // 24 hours
pub const MAX_INACTIVITY_PERIOD: i64 = 300 * 365 * 24 * 60 * 60; // 300 years
pub const MIN_GRACE_PERIOD: i64 = 24 * 60 * 60;            // 24 hours  
pub const MAX_GRACE_PERIOD: i64 = 90 * 24 * 60 * 60;       // 90 days
pub const MAX_BENEFICIARIES: u8 = 10;

// Fees
pub const ESTATE_FEE: u64 = 100_000_000;  // 0.1 SOL
pub const RWA_FEE: u64 = 10_000_000;      // 0.01 SOL

// Trading Limits
pub const MAX_PROFIT_SHARE: u8 = 50;      // Max 50% for AI
pub const MIN_EMERGENCY_DELAY: u32 = 24;   // 24 hours
pub const MAX_EMERGENCY_DELAY: u32 = 168;  // 7 days

// Admin
pub const ADMIN_TIMELOCK_DURATION: i64 = 48 * 60 * 60; // 48 hours
```

## Usage Examples

### Regular Check-in
```typescript
await program.methods.checkIn()
```

### Create RWA
```typescript
await program.methods.createRwa(
  "realEstate",           // Type
  "Beach House",          // Name
  "Malibu property",      // Description
  "$2,500,000",          // Value
  "ipfs://..."           // Metadata URI
)
```

### Trigger Inheritance
```typescript
await program.methods.triggerInheritance()
```

### Claim Inheritance
```typescript
await program.methods.claimInheritance(
  beneficiaryIndex  // 0-based index
)
```

### Emergency Lock/Unlock
```typescript
// Lock
await program.methods.emergencyLock()

// Unlock
await program.methods.emergencyUnlock(verificationCode)
```

## Multi-Signature Usage

### Initialize Multi-sig
```typescript
await program.methods.initializeMultisig(
  [signer1, signer2, signer3],  // Signers
  2                             // Threshold
)
```

### Create Proposal
```typescript
await program.methods.createProposal(
  targetEstate,
  {
    updateBeneficiaries: {
      beneficiaries: [...]
    }
  }
)
```

### Approve Proposal
```typescript
await program.methods.approveProposal(proposalId)
```

### Execute Proposal
```typescript
await program.methods.executeProposal()
```

### Emergency Force Unlock (Multisig)

Security checks enforced:
- Proposal must target the same estate
- Executor must be the original proposer
- Approvals must meet or exceed multisig.threshold

## Security Features

1. **Dead Man's Switch**: Automatic inheritance after inactivity
2. **Multi-signature Support**: Enhanced security for high-value estates
3. **Emergency Controls**: Lock/unlock mechanisms
4. **Time Delays**: Emergency withdrawals and admin changes
5. **Recovery System**: Admin-initiated recovery after 30+ days of claimability
6. **Soft Deletes**: RWAs are marked inactive rather than deleted

## Error Codes

- `InvalidInactivityPeriod`: Period outside allowed range
- `InvalidGracePeriod`: Grace period outside allowed range
- `EstateLocked`: Estate is locked
- `UnauthorizedAccess`: Caller not authorized
- `EstateClaimable`: Estate already in claim state
- `TooManyBeneficiaries`: Exceeds maximum of 10
- `InvalidBeneficiaryShares`: Shares don't sum to 100%
- `NotYetClaimable`: Waiting periods not elapsed
- `AlreadyClaimed`: Beneficiary already claimed
- `TradingAlreadyEnabled`: Trading already active
- `InvalidProfitShare`: Share outside 50-100% range

## Events

- `EstateCreated`: New estate initialized
- `EstateCheckedIn`: Timer reset
- `EstateLocked`: Estate locked for claims
- `BeneficiaryUpdated`: Beneficiary list changed
- `RWACreated`: New RWA added
- `ClaimExecuted`: Beneficiary claimed share
- `TradingEnabled`: Trading activated
- `ProfitsDistributed`: Trading profits distributed
- `MultisigCreated`: New multi-sig account
- `ProposalExecuted`: Multi-sig proposal executed 