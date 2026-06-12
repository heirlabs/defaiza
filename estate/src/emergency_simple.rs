use crate::{Estate, EstateError};
use anchor_lang::prelude::*;

// Simple emergency lock - no verification codes needed
// Owner proves identity via signature

#[derive(Accounts)]
pub struct EmergencyLockContext<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        has_one = owner,
        constraint = !estate.is_locked @ EstateError::AlreadyLocked,
    )]
    pub estate: Account<'info, Estate>,
}

#[derive(Accounts)]
pub struct EmergencyUnlockContext<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        has_one = owner,
        constraint = estate.is_locked @ EstateError::NotLocked,
    )]
    pub estate: Account<'info, Estate>,
}

// For multisig override
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
}

pub fn emergency_lock_impl(ctx: Context<EmergencyLockContext>, reason: String) -> Result<()> {
    let estate = &mut ctx.accounts.estate;

    // Validate reason
    require!(
        reason.len() > 5 && reason.len() <= 200,
        EstateError::InvalidLockReason
    );

    // Lock the estate
    estate.is_locked = true;

    // Pause trading if enabled
    if estate.trading_enabled {
        estate.trading_enabled = false;
        msg!("Trading automatically paused due to emergency lock");
    }

    // Emit event
    emit!(EstateLocked {
        estate_id: estate.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Estate {} emergency locked: {}",
        estate.estate_number,
        reason
    );

    Ok(())
}

pub fn emergency_unlock_impl(ctx: Context<EmergencyUnlockContext>) -> Result<()> {
    let estate = &mut ctx.accounts.estate;

    // Unlock the estate
    estate.is_locked = false;

    // Emit event
    emit!(EstateUnlocked {
        estate_id: estate.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("Estate {} emergency unlocked", estate.estate_number);

    Ok(())
}

pub fn force_unlock_by_multisig(ctx: Context<ForceUnlockByMultisig>) -> Result<()> {
    let estate = &mut ctx.accounts.estate;

    // Multisig validation is done through constraints
    // Just unlock the estate
    estate.is_locked = false;

    emit!(EstateUnlocked {
        estate_id: estate.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("Estate {} force unlocked by multisig", estate.estate_number);

    Ok(())
}

// Events
#[event]
pub struct EstateLocked {
    pub estate_id: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct EstateUnlocked {
    pub estate_id: Pubkey,
    pub timestamp: i64,
}
