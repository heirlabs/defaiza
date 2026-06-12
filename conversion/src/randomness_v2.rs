use anchor_lang::prelude::*;
use defai_protocol_common::{
    SWITCHBOARD_ON_DEMAND_PROGRAM_ID, SWITCHBOARD_RANDOMNESS_MIN_LEN,
    SWITCHBOARD_RANDOMNESS_VALUE_OFFSET,
};

/// Switchboard On-Demand randomness account state (subset used for validation).
#[derive(Debug, Clone, Copy, AnchorSerialize, AnchorDeserialize)]
pub struct RandomnessAccountData {
    pub seed: [u8; 32],
    pub value: [u8; 32],
    pub slot: u64,
    pub timestamp: i64,
}

#[account]
pub struct RandomnessState {
    pub bump: u8,
    pub authority: Pubkey,
    pub randomness_account: Pubkey,
    pub committed_slot: u64,
    pub revealed_value: [u8; 32],
    pub last_update: i64,
    pub is_pending: bool,
}

impl RandomnessState {
    pub const LEN: usize = 1 + 32 + 32 + 8 + 32 + 8 + 1;
}

#[derive(Accounts)]
pub struct InitializeRandomness<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + RandomnessState::LEN,
        seeds = [b"randomness_state"],
        bump
    )]
    pub randomness_state: Account<'info, RandomnessState>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CommitRandomness<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"randomness_state"],
        bump = randomness_state.bump,
        constraint = randomness_state.authority == authority.key() @ RandomnessError::Unauthorized
    )]
    pub randomness_state: Account<'info, RandomnessState>,

    /// CHECK: Switchboard randomness account — owner verified at reveal.
    pub randomness_account: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct RevealRandomness<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"randomness_state"],
        bump = randomness_state.bump,
        constraint = randomness_state.authority == authority.key() @ RandomnessError::Unauthorized
    )]
    pub randomness_state: Account<'info, RandomnessState>,

    /// CHECK: Switchboard randomness account validated in handler.
    #[account(
        constraint = randomness_account.key() == randomness_state.randomness_account @ RandomnessError::InvalidRandomnessAccount
    )]
    pub randomness_account: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct SimpleRandomness<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"randomness_state"],
        bump = randomness_state.bump,
        constraint = randomness_state.authority == authority.key() @ RandomnessError::Unauthorized
    )]
    pub randomness_state: Account<'info, RandomnessState>,

    /// CHECK: Recent blockhashes sysvar
    #[account(address = solana_program::sysvar::recent_blockhashes::ID)]
    pub recent_blockhashes: AccountInfo<'info>,
}

/// Initialize randomness state; authority must be protocol governance.
pub fn initialize_randomness(ctx: Context<InitializeRandomness>) -> Result<()> {
    defai_protocol_common::require_protocol_governor(&ctx.accounts.authority.key())?;
    let randomness_state = &mut ctx.accounts.randomness_state;
    randomness_state.bump = ctx.bumps.randomness_state;
    randomness_state.authority = ctx.accounts.authority.key();
    randomness_state.randomness_account = Pubkey::default();
    randomness_state.committed_slot = 0;
    randomness_state.revealed_value = [0u8; 32];
    randomness_state.last_update = Clock::get()?.unix_timestamp;
    randomness_state.is_pending = false;

    msg!("Randomness state initialized");
    Ok(())
}

/// Record Switchboard randomness account for a pending reveal.
pub fn commit_randomness(ctx: Context<CommitRandomness>) -> Result<()> {
    let randomness_state = &mut ctx.accounts.randomness_state;
    let clock = Clock::get()?;

    randomness_state.randomness_account = ctx.accounts.randomness_account.key();
    randomness_state.committed_slot = clock.slot;
    randomness_state.is_pending = true;

    msg!("Randomness committed at slot {}", clock.slot);
    Ok(())
}

/// Read verified Switchboard randomness into program state.
pub fn reveal_randomness(ctx: Context<RevealRandomness>) -> Result<()> {
    let randomness_state = &mut ctx.accounts.randomness_state;
    let clock = Clock::get()?;

    require!(randomness_state.is_pending, RandomnessError::NoCommitment);

    require_keys_eq!(
        *ctx.accounts.randomness_account.owner,
        SWITCHBOARD_ON_DEMAND_PROGRAM_ID,
        RandomnessError::InvalidRandomnessAccountOwner
    );

    let data = ctx.accounts.randomness_account.try_borrow_data()?;
    require!(
        data.len() >= SWITCHBOARD_RANDOMNESS_MIN_LEN,
        RandomnessError::InvalidAccountData
    );

    let end = SWITCHBOARD_RANDOMNESS_VALUE_OFFSET
        .checked_add(32)
        .ok_or(RandomnessError::InvalidAccountData)?;
    randomness_state
        .revealed_value
        .copy_from_slice(&data[SWITCHBOARD_RANDOMNESS_VALUE_OFFSET..end]);
    randomness_state.last_update = clock.unix_timestamp;
    randomness_state.is_pending = false;

    msg!("Randomness revealed successfully");
    Ok(())
}

/// Authority-only fallback entropy when Switchboard is unavailable (testing / contingency).
pub fn generate_simple_randomness(ctx: Context<SimpleRandomness>) -> Result<()> {
    let randomness_state = &mut ctx.accounts.randomness_state;
    let clock = Clock::get()?;

    let blockhash = recent_blockhash_from_sysvar(&ctx.accounts.recent_blockhashes)?;

    let mut hasher = solana_program::keccak::Hasher::default();
    hasher.hash(&blockhash);
    hasher.hash(&clock.slot.to_le_bytes());
    hasher.hash(&clock.unix_timestamp.to_le_bytes());
    hasher.hash(&ctx.accounts.authority.key().to_bytes());

    let hash = hasher.result();
    randomness_state
        .revealed_value
        .copy_from_slice(&hash.to_bytes());
    randomness_state.last_update = clock.unix_timestamp;
    randomness_state.is_pending = false;

    msg!("Authority-generated fallback randomness set");
    Ok(())
}

/// Derive u64 randomness from VRF output and swap context.
pub fn generate_vrf_random(vrf_result: &[u8; 32], user: &Pubkey, nft_mint: &Pubkey) -> u64 {
    let mut hasher = solana_program::keccak::Hasher::default();
    hasher.hash(vrf_result);
    hasher.hash(&user.to_bytes());
    hasher.hash(&nft_mint.to_bytes());

    let hash = hasher.result();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash.to_bytes()[0..8]);
    u64::from_le_bytes(bytes)
}

/// Map uniform random value into inclusive bonus bps range.
pub fn calculate_random_bonus(random_value: u64, min_bonus: u16, max_bonus: u16) -> u16 {
    let bonus_range = max_bonus.saturating_sub(min_bonus);
    if bonus_range == 0 {
        min_bonus
    } else {
        let span = (bonus_range as u64).saturating_add(1);
        min_bonus.saturating_add((random_value % span) as u16)
    }
}

/// Fallback entropy for non-VRF swap paths (blockhash + clock + user + mint).
pub fn generate_secure_random(
    user: &Pubkey,
    nft_mint: &Pubkey,
    clock: &Clock,
    recent_blockhash: &[u8; 32],
) -> u64 {
    let mut hasher = solana_program::keccak::Hasher::default();
    hasher.hash(&user.to_bytes());
    hasher.hash(&nft_mint.to_bytes());
    hasher.hash(&clock.unix_timestamp.to_le_bytes());
    hasher.hash(&clock.slot.to_le_bytes());
    hasher.hash(recent_blockhash);
    hasher.hash(&clock.epoch.to_le_bytes());

    let hash = hasher.result();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash.to_bytes()[0..8]);
    u64::from_le_bytes(bytes)
}

/// Parse recent blockhash bytes from sysvar data.
pub fn recent_blockhash_from_sysvar(recent_blockhashes: &AccountInfo) -> Result<[u8; 32]> {
    let data = recent_blockhashes.try_borrow_data()?;
    require!(data.len() >= 40, RandomnessError::InvalidAccountData);
    let slice = &data[8..40];
    let mut out = [0u8; 32];
    out.copy_from_slice(slice);
    Ok(out)
}

#[cfg(test)]
mod randomness_tests {
    use super::*;

    fn account_info_with_data(key: Pubkey, data: Vec<u8>) -> AccountInfo<'static> {
        let key = Box::leak(Box::new(key));
        let lamports = Box::leak(Box::new(1_000_000u64));
        let data = Box::leak(data.into_boxed_slice());
        let owner = Box::leak(Box::new(Pubkey::default()));
        AccountInfo::new(key, false, false, lamports, data, owner, false, 0)
    }

    #[test]
    fn calculate_random_bonus_fixed_when_min_equals_max() {
        assert_eq!(calculate_random_bonus(999, 1500, 1500), 1500);
    }

    #[test]
    fn calculate_random_bonus_respects_inclusive_range() {
        let min = 1_500_u16;
        let max = 5_000_u16;
        for value in [0_u64, 1, 7, 100, u64::MAX] {
            let bonus = calculate_random_bonus(value, min, max);
            assert!(bonus >= min && bonus <= max);
        }
    }

    #[test]
    fn generate_vrf_random_is_deterministic() {
        let vrf = [7_u8; 32];
        let user = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let a = generate_vrf_random(&vrf, &user, &mint);
        let b = generate_vrf_random(&vrf, &user, &mint);
        assert_eq!(a, b);
    }

    #[test]
    fn generate_vrf_random_changes_with_different_mint() {
        let vrf = [7_u8; 32];
        let user = Pubkey::new_unique();
        let a = generate_vrf_random(&vrf, &user, &Pubkey::new_unique());
        let b = generate_vrf_random(&vrf, &user, &Pubkey::new_unique());
        assert_ne!(a, b);
    }

    #[test]
    fn generate_secure_random_is_deterministic_for_fixed_inputs() {
        let user = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let clock = Clock {
            slot: 42,
            epoch: 3,
            unix_timestamp: 1_700_000_000,
            epoch_start_timestamp: 0,
            leader_schedule_epoch: 0,
        };
        let blockhash = [9_u8; 32];
        let a = generate_secure_random(&user, &mint, &clock, &blockhash);
        let b = generate_secure_random(&user, &mint, &clock, &blockhash);
        assert_eq!(a, b);
    }

    #[test]
    fn recent_blockhash_from_sysvar_rejects_short_data() {
        let info = account_info_with_data(Pubkey::new_unique(), vec![0_u8; 16]);
        assert!(recent_blockhash_from_sysvar(&info).is_err());
    }

    #[test]
    fn recent_blockhash_from_sysvar_extracts_expected_slice() {
        let mut data = vec![0_u8; 64];
        for (idx, byte) in data[8..40].iter_mut().enumerate() {
            *byte = (idx + 1) as u8;
        }
        let info = account_info_with_data(Pubkey::new_unique(), data);
        let hash = recent_blockhash_from_sysvar(&info).unwrap();
        assert_eq!(hash[0], 1);
        assert_eq!(hash[31], 32);
    }
}

#[error_code]
pub enum RandomnessError {
    #[msg("No randomness commitment found")]
    NoCommitment,
    #[msg("Invalid randomness account")]
    InvalidRandomnessAccount,
    #[msg("Randomness account not owned by Switchboard On-Demand program")]
    InvalidRandomnessAccountOwner,
    #[msg("Invalid account data")]
    InvalidAccountData,
    #[msg("Randomness not resolved yet")]
    RandomnessNotResolved,
    #[msg("Unauthorized")]
    Unauthorized,
}
