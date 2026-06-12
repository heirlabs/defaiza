use anchor_lang::prelude::*;
use crate::{Estate, EstateError};

// Constants for emergency lock
pub const EMERGENCY_LOCK_COOLDOWN: i64 = 3600; // 1 hour cooldown between locks
pub const MIN_UNLOCK_DELAY: i64 = 300; // 5 minutes minimum before unlock

#[account]
pub struct EmergencyLockState {
    pub estate: Pubkey,
    pub lock_timestamp: i64,
    pub unlock_timestamp: Option<i64>,
    pub lock_reason: String,
    pub lock_count: u32,
    pub last_lock_time: i64,
    pub verification_hash: [u8; 32],
    pub failed_unlock_attempts: u8,
    pub max_unlock_attempts: u8,
    pub lock_type: LockType,
    pub initiated_by: Pubkey,
    pub bump: u8,
}

impl EmergencyLockState {
    pub const LEN: usize = 8 + // discriminator
        32 + // estate
        8 + // lock_timestamp
        (1 + 8) + // unlock_timestamp Option
        (4 + 128) + // lock_reason String
        4 + // lock_count
        8 + // last_lock_time
        32 + // verification_hash
        1 + // failed_unlock_attempts
        1 + // max_unlock_attempts
        1 + // lock_type
        32 + // initiated_by
        1; // bump
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Debug)]
pub enum LockType {
    SecurityBreach,
    SuspiciousActivity,
    UserInitiated,
    MultisigInitiated,
    Recovery,
}

#[derive(Accounts)]
#[instruction(reason: String, lock_type: LockType)]
pub struct EmergencyLockContext<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        mut,
        constraint = validate_lock_authority(&estate, &authority.key(), &lock_type) @ EstateError::UnauthorizedAccess,
        constraint = !estate.is_locked @ EstateError::AlreadyLocked,
    )]
    pub estate: Account<'info, Estate>,
    
    #[account(
        init_if_needed,
        payer = authority,
        space = EmergencyLockState::LEN,
        seeds = [b"emergency_lock", estate.key().as_ref()],
        bump
    )]
    pub emergency_state: Account<'info, EmergencyLockState>,
    
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
#[instruction(verification_code: String)]
pub struct EmergencyUnlockContext<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        mut,
        constraint = estate.is_locked @ EstateError::NotLocked,
        constraint = validate_unlock_authority(&estate, &authority.key(), &emergency_state.lock_type) @ EstateError::UnauthorizedAccess,
    )]
    pub estate: Account<'info, Estate>,
    
    #[account(
        mut,
        seeds = [b"emergency_lock", estate.key().as_ref()],
        bump = emergency_state.bump,
        constraint = emergency_state.estate == estate.key() @ EstateError::InvalidEmergencyState,
    )]
    pub emergency_state: Account<'info, EmergencyLockState>,
    
    pub clock: Sysvar<'info, Clock>,
}

// Force Unlock by Multisig
#[derive(Accounts)]
pub struct ForceUnlockByMultisig<'info> {
    pub executor: Signer<'info>,
    
    #[account(
        mut,
        constraint = estate.is_locked @ EstateError::NotLocked,
        constraint = estate.multisig.is_some() @ EstateError::NoMultisigAttached,
    )]
    pub estate: Account<'info, Estate>,
    
    #[account(
        mut,
        seeds = [b"emergency_lock", estate.key().as_ref()],
        bump = emergency_state.bump,
    )]
    pub emergency_state: Account<'info, EmergencyLockState>,
    
    #[account(
        constraint = multisig.key() == estate.multisig.unwrap() @ EstateError::InvalidMultisig,
    )]
    pub multisig: Account<'info, crate::Multisig>,
    
    #[account(
        mut,
        constraint = proposal.multisig == multisig.key() @ EstateError::InvalidProposal,
        constraint = proposal.executed @ EstateError::ProposalNotExecuted,
        constraint = matches!(proposal.action, crate::ProposalAction::EmergencyUnlock { .. }) @ EstateError::InvalidProposalType,
        constraint = proposal.target_estate == estate.key() @ EstateError::InvalidProposalEstate,
        constraint = proposal.proposer == executor.key() @ EstateError::ProposerNotExecutor,
        constraint = proposal.approvals.len() >= multisig.threshold as usize @ EstateError::NotEnoughApprovals,
    )]
    pub proposal: Account<'info, crate::Proposal>,
    
    pub clock: Sysvar<'info, Clock>,
}

// View Emergency Lock Status
#[derive(Accounts)]
pub struct ViewEmergencyStatus<'info> {
    pub estate: Account<'info, Estate>,
    
    #[account(
        seeds = [b"emergency_lock", estate.key().as_ref()],
        bump = emergency_state.bump,
    )]
    pub emergency_state: Account<'info, EmergencyLockState>,
}

// Helper functions
pub fn validate_lock_authority(estate: &Estate, authority: &Pubkey, lock_type: &LockType) -> bool {
    match lock_type {
        LockType::UserInitiated | LockType::SecurityBreach | LockType::SuspiciousActivity => {
            estate.owner == *authority
        },
        LockType::MultisigInitiated => {
            if let Some(_multisig) = estate.multisig {
                // Will be validated through proposal system
                true
            } else {
                false
            }
        },
        LockType::Recovery => {
            // Only during recovery process
            false // Handle in recovery module
        }
    }
}

pub fn validate_unlock_authority(estate: &Estate, authority: &Pubkey, lock_type: &LockType) -> bool {
    match lock_type {
        LockType::UserInitiated => estate.owner == *authority,
        LockType::SecurityBreach | LockType::SuspiciousActivity => {
            // Requires owner with additional verification
            estate.owner == *authority
        },
        LockType::MultisigInitiated => {
            // Requires multisig approval
            false // Handle through ForceUnlockByMultisig
        },
        LockType::Recovery => {
            // Handle in recovery module
            false
        }
    }
}

pub fn generate_verification_hash(
    estate_key: &Pubkey,
    owner_email_hash: &[u8; 32],
    lock_timestamp: i64,
    code: &str,
) -> [u8; 32] {
    use anchor_lang::solana_program::hash::hash;
    
    let mut data = Vec::new();
    data.extend_from_slice(estate_key.as_ref());
    data.extend_from_slice(owner_email_hash);
    data.extend_from_slice(&lock_timestamp.to_le_bytes());
    data.extend_from_slice(code.as_bytes());
    
    let hash_result = hash(&data);
    let mut result = [0u8; 32];
    result.copy_from_slice(&hash_result.to_bytes()[..32]);
    result
}

// Events
#[event]
pub struct EmergencyLockInitiated {
    pub estate: Pubkey,
    pub lock_type: LockType,
    pub reason: String,
    pub initiated_by: Pubkey,
    pub lock_timestamp: i64,
    pub lock_count: u32,
}

#[event]
pub struct EmergencyUnlockSuccessful {
    pub estate: Pubkey,
    pub unlocked_by: Pubkey,
    pub unlock_timestamp: i64,
    pub lock_duration: i64,
}

#[event]
pub struct EmergencyUnlockFailed {
    pub estate: Pubkey,
    pub attempted_by: Pubkey,
    pub failed_attempts: u8,
    pub timestamp: i64,
}

#[event]
pub struct EmergencyForceUnlock {
    pub estate: Pubkey,
    pub multisig: Pubkey,
    pub proposal: Pubkey,
    pub timestamp: i64,
}

// Implementation functions
pub fn emergency_lock_impl(
    ctx: Context<EmergencyLockContext>,
    reason: String,
    lock_type: LockType,
    verification_code: String,
) -> Result<()> {
    let clock = &ctx.accounts.clock;
    let estate = &mut ctx.accounts.estate;
    let emergency_state = &mut ctx.accounts.emergency_state;
    
    // Check cooldown period
    if emergency_state.last_lock_time > 0 {
        require!(
            clock.unix_timestamp - emergency_state.last_lock_time >= EMERGENCY_LOCK_COOLDOWN,
            EstateError::EmergencyLockCooldown
        );
    }
    
    // Validate reason length
    require!(
        reason.len() > 10 && reason.len() <= 128,
        EstateError::InvalidLockReason
    );
    
    // Lock the estate
    estate.is_locked = true;
    
    // If trading is enabled, pause it
    if estate.trading_enabled {
        estate.trading_enabled = false;
        msg!("Trading automatically paused due to emergency lock");
    }
    
    // Update emergency state
    let is_new = emergency_state.lock_count == 0;
    
    emergency_state.estate = estate.key();
    emergency_state.lock_timestamp = clock.unix_timestamp;
    emergency_state.unlock_timestamp = None;
    emergency_state.lock_reason = reason.clone();
    emergency_state.lock_count = emergency_state.lock_count.saturating_add(1);
    emergency_state.last_lock_time = clock.unix_timestamp;
    emergency_state.verification_hash = generate_verification_hash(
        &estate.key(),
        &estate.owner_email_hash,
        clock.unix_timestamp,
        &verification_code,
    );
    emergency_state.failed_unlock_attempts = 0;
    emergency_state.max_unlock_attempts = 5;
    emergency_state.lock_type = lock_type;
    emergency_state.initiated_by = ctx.accounts.authority.key();
    
    if is_new {
        emergency_state.bump = ctx.bumps.emergency_state;
    }
    
    // Emit event
    emit!(EmergencyLockInitiated {
        estate: estate.key(),
        lock_type,
        reason,
        initiated_by: ctx.accounts.authority.key(),
        lock_timestamp: clock.unix_timestamp,
        lock_count: emergency_state.lock_count,
    });
    
    msg!(
        "Estate {} emergency locked. Type: {:?}, Count: {}",
        estate.estate_number,
        lock_type,
        emergency_state.lock_count
    );
    
    Ok(())
}

pub fn emergency_unlock_impl(
    ctx: Context<EmergencyUnlockContext>,
    verification_code: String,
) -> Result<()> {
    let clock = &ctx.accounts.clock;
    let estate = &mut ctx.accounts.estate;
    let emergency_state = &mut ctx.accounts.emergency_state;
    
    // Check minimum lock duration
    require!(
        clock.unix_timestamp - emergency_state.lock_timestamp >= MIN_UNLOCK_DELAY,
        EstateError::UnlockTooEarly
    );
    
    // Check max unlock attempts
    require!(
        emergency_state.failed_unlock_attempts < emergency_state.max_unlock_attempts,
        EstateError::MaxUnlockAttemptsExceeded
    );
    
    // Verify the code
    let expected_hash = generate_verification_hash(
        &estate.key(),
        &estate.owner_email_hash,
        emergency_state.lock_timestamp,
        &verification_code,
    );
    
    if expected_hash != emergency_state.verification_hash {
        emergency_state.failed_unlock_attempts += 1;
        
        emit!(EmergencyUnlockFailed {
            estate: estate.key(),
            attempted_by: ctx.accounts.authority.key(),
            failed_attempts: emergency_state.failed_unlock_attempts,
            timestamp: clock.unix_timestamp,
        });
        
        return Err(EstateError::InvalidVerificationCode.into());
    }
    
    // Unlock the estate
    estate.is_locked = false;
    
    // Update emergency state
    emergency_state.unlock_timestamp = Some(clock.unix_timestamp);
    emergency_state.failed_unlock_attempts = 0;
    
    let lock_duration = clock.unix_timestamp - emergency_state.lock_timestamp;
    
    // Emit event
    emit!(EmergencyUnlockSuccessful {
        estate: estate.key(),
        unlocked_by: ctx.accounts.authority.key(),
        unlock_timestamp: clock.unix_timestamp,
        lock_duration,
    });
    
    msg!(
        "Estate {} emergency unlocked after {} seconds",
        estate.estate_number,
        lock_duration
    );
    
    Ok(())
}

pub fn force_unlock_by_multisig(ctx: Context<ForceUnlockByMultisig>) -> Result<()> {
    let clock = &ctx.accounts.clock;
    let estate = &mut ctx.accounts.estate;
    let emergency_state = &mut ctx.accounts.emergency_state;
    
    // Unlock the estate
    estate.is_locked = false;
    
    // Update emergency state
    emergency_state.unlock_timestamp = Some(clock.unix_timestamp);
    emergency_state.failed_unlock_attempts = 0;
    
    // Emit event
    emit!(EmergencyForceUnlock {
        estate: estate.key(),
        multisig: ctx.accounts.multisig.key(),
        proposal: ctx.accounts.proposal.key(),
        timestamp: clock.unix_timestamp,
    });
    
    msg!(
        "Estate {} force unlocked by multisig",
        estate.estate_number
    );
    
    Ok(())
}