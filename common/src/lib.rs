//! Shared protocol constants for DEFAI Solana programs.

use anchor_lang::prelude::*;

/// Switchboard On-Demand program id (mainnet-beta).
pub const SWITCHBOARD_ON_DEMAND_PROGRAM_ID: Pubkey =
    pubkey!("SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv");

/// Minimum length of a Switchboard randomness account payload we accept.
pub const SWITCHBOARD_RANDOMNESS_MIN_LEN: usize = 72;

/// Byte offset of the 32-byte random value inside the Switchboard randomness account.
pub const SWITCHBOARD_RANDOMNESS_VALUE_OFFSET: usize = 40;

/// Production protocol governance (Squads multisig).
pub const PROTOCOL_GOVERNANCE: Pubkey = pubkey!("9Sg25FG7tNdqrx18LARXjLVmLomxqvKhN6TPGwP7nQzZ");

/// Returns true when `signer` may perform protocol-governance actions.
pub fn is_protocol_governor(signer: &Pubkey) -> bool {
    #[cfg(feature = "dev-governance-bypass")]
    {
        let _ = signer;
        return true;
    }
    #[cfg(not(feature = "dev-governance-bypass"))]
    {
        *signer == PROTOCOL_GOVERNANCE
    }
}

/// Enforces protocol governance on production builds.
pub fn require_protocol_governor(signer: &Pubkey) -> Result<()> {
    require!(
        is_protocol_governor(signer),
        ProtocolGovernanceError::UnauthorizedGovernor
    );
    Ok(())
}

/// Rent-safe minimum lamports for an account of `data_len` bytes.
pub fn minimum_rent_lamports(data_len: usize) -> Result<u64> {
    Ok(Rent::get()?.minimum_balance(data_len))
}

#[error_code]
pub enum ProtocolGovernanceError {
    #[msg("Signer is not the protocol governance authority")]
    UnauthorizedGovernor,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_governance_pubkey_is_fixed() {
        assert_eq!(
            PROTOCOL_GOVERNANCE.to_string(),
            "9Sg25FG7tNdqrx18LARXjLVmLomxqvKhN6TPGwP7nQzZ"
        );
    }

    #[test]
    #[cfg(not(feature = "dev-governance-bypass"))]
    fn non_governor_rejected_without_bypass() {
        let random = Pubkey::new_unique();
        assert!(!is_protocol_governor(&random));
    }

    #[test]
    fn governor_accepted_without_bypass() {
        assert!(is_protocol_governor(&PROTOCOL_GOVERNANCE));
    }

    #[test]
    fn switchboard_on_demand_program_id_is_valid() {
        assert_eq!(
            SWITCHBOARD_ON_DEMAND_PROGRAM_ID.to_string(),
            "SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv"
        );
    }

    #[test]
    #[cfg(not(feature = "dev-governance-bypass"))]
    fn require_protocol_governor_rejects_random_signer() {
        assert!(require_protocol_governor(&Pubkey::new_unique()).is_err());
    }

    #[test]
    fn require_protocol_governor_accepts_production_key() {
        assert!(require_protocol_governor(&PROTOCOL_GOVERNANCE).is_ok());
    }

    #[test]
    fn switchboard_randomness_layout_constants_are_consistent() {
        assert!(SWITCHBOARD_RANDOMNESS_VALUE_OFFSET >= 8);
        assert!(SWITCHBOARD_RANDOMNESS_MIN_LEN >= SWITCHBOARD_RANDOMNESS_VALUE_OFFSET + 32);
    }

    #[test]
    fn minimum_rent_lamports_requires_runtime_sysvar_off_chain() {
        // Rent::get() needs the Solana sysvar; host unit tests have no runtime.
        assert!(minimum_rent_lamports(256).is_err());
    }
}

#[cfg(all(test, feature = "dev-governance-bypass"))]
mod bypass_tests {
    use super::*;

    #[test]
    fn bypass_accepts_any_signer() {
        assert!(is_protocol_governor(&Pubkey::new_unique()));
        assert!(require_protocol_governor(&Pubkey::new_unique()).is_ok());
    }
}
