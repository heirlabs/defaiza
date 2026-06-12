use anchor_lang::prelude::InterfaceAccount;
use anchor_lang::prelude::*;
use anchor_spl::{
    token::{self, Token, TokenAccount, Transfer},
    token_2022::{self as token22, Token2022},
    token_interface::{
        Burn, CloseAccount, Mint, TokenAccount as TokenAccount2022, TransferChecked,
    },
};
use defai_protocol_common::require_protocol_governor;
use solana_program::program_option::COption;

// Old VRF modules removed - using randomness_v2 only
pub mod randomness_v2;
use randomness_v2::*;

declare_id!("DB9Zvhdp5xh853d2Tr2HBkRDDaCSioD7vwchhcGaXCw3");

// Tax configuration constants (basis points = parts per 10_000)
const INITIAL_TAX_BPS: u16 = 500; // 5%
const TAX_INCREMENT_BPS: u16 = 100; // 1% each swap
const TAX_CAP_BPS: u16 = 3000; // 30% maximum tax
const TAX_RESET_DURATION: i64 = 24 * 60 * 60; // 24 hours in seconds

// Timelock constants
const ADMIN_TIMELOCK_DURATION: i64 = 48 * 60 * 60; // 48 hours for admin actions

// OG NFT Whitelist Merkle Root
const WHITELIST_ROOT: [u8; 32] = [
    75, 45, 118, 95, 221, 195, 106, 5, 187, 186, 56, 74, 112, 138, 19, 108, 59, 243, 44, 140, 228,
    10, 199, 125, 41, 242, 223, 102, 191, 115, 73, 142,
];

// Vesting constants
const VESTING_DURATION: i64 = 90 * 24 * 60 * 60; // 90 days in seconds
const CLIFF_DURATION: i64 = 2 * 24 * 60 * 60; // 2 days in seconds

// ============================================
// LOCKED CONTEXT - DO NOT CHANGE THESE BONUS RANGES EVER
// These bonus ranges are FINAL and IMMUTABLE
// ============================================
// Bonus ranges per tier (basis points)
const TIER_0_MIN_BONUS: u16 = 0; // 0%  - OG tier (No bonus)
const TIER_0_MAX_BONUS: u16 = 0; // 0%  - OG tier (No bonus)
const TIER_1_MIN_BONUS: u16 = 0; // 0%  - Train
const TIER_1_MAX_BONUS: u16 = 1500; // 15% - Train
const TIER_2_MIN_BONUS: u16 = 1500; // 15% - Boat
const TIER_2_MAX_BONUS: u16 = 5000; // 50% - Boat
const TIER_3_MIN_BONUS: u16 = 2000; // 20% - Plane
const TIER_3_MAX_BONUS: u16 = 10000; // 100% - Plane
const TIER_4_MIN_BONUS: u16 = 5000; // 50% - Rocket
const TIER_4_MAX_BONUS: u16 = 30000; // 300% - Rocket
                                     // ============================================
                                     // END LOCKED CONTEXT
                                     // ============================================

#[program]
pub mod defai_swap {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, prices: Vec<u64>) -> Result<()> {
        require_protocol_governor(&ctx.accounts.admin.key())?;
        require!(prices.len() == 5, ErrorCode::InvalidInput);

        let cfg = &mut ctx.accounts.config;
        cfg.admin = ctx.accounts.admin.key();
        cfg.old_mint = *ctx.accounts.old_mint.key;
        cfg.new_mint = *ctx.accounts.new_mint.key;
        cfg.collection = *ctx.accounts.collection.key;
        cfg.treasury = *ctx.accounts.treasury.key;
        cfg.prices = [prices[0], prices[1], prices[2], prices[3], prices[4]];
        cfg.paused = false;
        cfg.pending_admin = None;
        cfg.admin_change_timestamp = 0;
        // Auto-enable VRF by default; ensure VRF state is initialized and randomness consumed before swaps
        cfg.vrf_enabled = true;

        // Persist escrow bump for later signer seeds
        let escrow = &mut ctx.accounts.escrow;
        escrow.bump = ctx.bumps.escrow;

        // Initialize tax state
        let tax_state = &mut ctx.accounts.tax_state;
        tax_state.current_bps = INITIAL_TAX_BPS;
        tax_state.bump = ctx.bumps.tax_state;
        tax_state.last_reset_ts = Clock::get()?.unix_timestamp;
        Ok(())
    }

    // Initialize the legacy OLD token escrow account owned by the escrow PDA.
    // This creates the SPL-Token account that will receive OLD tokens during swaps.
    pub fn init_escrow_old(ctx: Context<InitEscrowOld>) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );
        msg!("Initialized OLD token escrow account");
        Ok(())
    }

    pub fn update_prices(ctx: Context<UpdateConfig>, prices: Vec<u64>) -> Result<()> {
        require!(prices.len() == 5, ErrorCode::InvalidInput);
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        let cfg = &mut ctx.accounts.config;
        cfg.prices = [prices[0], prices[1], prices[2], prices[3], prices[4]];

        Ok(())
    }

    pub fn update_treasury(ctx: Context<UpdateConfig>, new_treasury: Pubkey) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        let cfg = &mut ctx.accounts.config;
        cfg.treasury = new_treasury;

        Ok(())
    }

    pub fn pause(ctx: Context<UpdateConfig>) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );
        require!(!ctx.accounts.config.paused, ErrorCode::AlreadyPaused);

        ctx.accounts.config.paused = true;

        // Emit admin action event
        emit!(AdminAction {
            admin: ctx.accounts.admin.key(),
            action: "Pause protocol".to_string(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    pub fn unpause(ctx: Context<UpdateConfig>) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );
        require!(ctx.accounts.config.paused, ErrorCode::NotPaused);

        ctx.accounts.config.paused = false;

        // Emit admin action event
        emit!(AdminAction {
            admin: ctx.accounts.admin.key(),
            action: "Unpause protocol".to_string(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    pub fn initialize_whitelist(ctx: Context<InitializeWhitelist>) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        let whitelist = &mut ctx.accounts.whitelist;
        whitelist.root = WHITELIST_ROOT;
        whitelist.claimed_count = 0;
        Ok(())
    }

    pub fn propose_admin_change(ctx: Context<UpdateConfig>, new_admin: Pubkey) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        let cfg = &mut ctx.accounts.config;
        cfg.pending_admin = Some(new_admin);
        cfg.admin_change_timestamp = Clock::get()?.unix_timestamp + ADMIN_TIMELOCK_DURATION;

        msg!(
            "Admin change proposed. Can be executed after {}",
            cfg.admin_change_timestamp
        );

        // Emit admin action event
        emit!(AdminAction {
            admin: ctx.accounts.admin.key(),
            action: format!("Propose admin change to {}", new_admin),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    pub fn accept_admin_change(ctx: Context<UpdateConfig>) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        let cfg = &mut ctx.accounts.config;
        require!(cfg.pending_admin.is_some(), ErrorCode::NoPendingAdminChange);
        require!(
            Clock::get()?.unix_timestamp >= cfg.admin_change_timestamp,
            ErrorCode::TimelockNotExpired
        );

        let old_admin = cfg.admin;
        let new_admin = cfg.pending_admin.unwrap();
        cfg.admin = new_admin;
        cfg.pending_admin = None;
        cfg.admin_change_timestamp = 0;

        msg!("Admin changed from {} to {}", old_admin, new_admin);

        // Emit admin action event
        emit!(AdminAction {
            admin: old_admin,
            action: format!("Admin changed to {}", new_admin),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    // Old VRF functions removed - use randomness_v2 functions instead

    // New Switchboard On-Demand Randomness Instructions
    pub fn initialize_randomness_v2(ctx: Context<InitializeRandomness>) -> Result<()> {
        randomness_v2::initialize_randomness(ctx)
    }

    pub fn commit_randomness_v2(ctx: Context<CommitRandomness>) -> Result<()> {
        randomness_v2::commit_randomness(ctx)
    }

    pub fn reveal_randomness_v2(ctx: Context<RevealRandomness>) -> Result<()> {
        randomness_v2::reveal_randomness(ctx)
    }

    pub fn generate_simple_randomness(ctx: Context<SimpleRandomness>) -> Result<()> {
        randomness_v2::generate_simple_randomness(ctx)
    }

    pub fn initialize_user_tax(ctx: Context<InitializeUserTax>) -> Result<()> {
        let user_tax_state = &mut ctx.accounts.user_tax_state;
        user_tax_state.user = ctx.accounts.user.key();
        user_tax_state.tax_rate_bps = INITIAL_TAX_BPS;
        user_tax_state.last_swap_timestamp = Clock::get()?.unix_timestamp;
        user_tax_state.swap_count = 0;
        Ok(())
    }

    pub fn reset_user_tax(ctx: Context<ResetUserTax>) -> Result<()> {
        let user_tax_state = &mut ctx.accounts.user_tax_state;
        let now = Clock::get()?.unix_timestamp;

        require!(
            now >= user_tax_state.last_swap_timestamp + TAX_RESET_DURATION,
            ErrorCode::TaxResetTooEarly
        );

        let old_rate = user_tax_state.tax_rate_bps;
        user_tax_state.tax_rate_bps = INITIAL_TAX_BPS;
        user_tax_state.swap_count = 0;

        // Emit tax reset event
        emit!(TaxReset {
            user: ctx.accounts.user.key(),
            old_rate_bps: old_rate,
            new_rate_bps: INITIAL_TAX_BPS,
            timestamp: now,
        });

        Ok(())
    }

    pub fn initialize_collection(
        ctx: Context<InitializeCollection>,
        tier_names: Vec<String>,
        tier_symbols: Vec<String>,
        tier_prices: [u64; 5],
        tier_supplies: [u16; 5],
        tier_uri_prefixes: Vec<String>,
        og_tier_0_merkle_root: [u8; 32], // For MAY20DEFAIHolders.csv - NFT minting with 1:1 vesting
        airdrop_merkle_root: [u8; 32],   // For 10_1AIR-Sheet1.csv - Pure vesting, no NFT
        og_tier_0_supply: u16,           // Reserved supply for OG holders
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.authority.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        let collection_config = &mut ctx.accounts.collection_config;
        collection_config.authority = ctx.accounts.authority.key();
        collection_config.collection_mint = ctx.accounts.collection_mint.key();
        collection_config.treasury = ctx.accounts.treasury.key();
        collection_config.defai_mint = ctx.accounts.defai_mint.key();
        collection_config.old_defai_mint = ctx.accounts.old_defai_mint.key();

        for i in 0..5 {
            collection_config.tier_names[i] = tier_names.get(i).cloned().unwrap_or_default();
            collection_config.tier_symbols[i] = tier_symbols.get(i).cloned().unwrap_or_default();
            collection_config.tier_uri_prefixes[i] =
                tier_uri_prefixes.get(i).cloned().unwrap_or_default();
        }

        collection_config.tier_prices = tier_prices;
        collection_config.tier_supplies = tier_supplies;
        collection_config.tier_minted = [0; 5];
        collection_config.og_tier_0_merkle_root = og_tier_0_merkle_root; // MAY20DEFAIHolders merkle root
        collection_config.airdrop_merkle_root = airdrop_merkle_root; // 10_1AIR merkle root
        collection_config.og_tier_0_supply = og_tier_0_supply; // Reserved supply for OG holders
        collection_config.og_tier_0_minted = 0; // Initialize OG claims counter

        Ok(())
    }

    /// Function 1: For MAY20DEFAIHolders.csv - Mints NFT and provides 1:1 vesting from Quantity column
    pub fn swap_og_tier0_for_pnft_v6(
        ctx: Context<SwapOgTier0ForPnftV6>,
        vesting_amount: u64, // The Quantity from MAY20DEFAIHolders.csv for 1:1 vesting
        merkle_proof: Vec<[u8; 32]>,
        _metadata_uri: String,
        _name: String,
        _symbol: String,
    ) -> Result<()> {
        msg!("=== SWAP OG TIER 0 FOR PNFT V6 START ===");
        let config_global = load_config(&ctx.accounts.config)?;
        require!(!config_global.paused, ErrorCode::ProtocolPaused);
        verify_nft_mint_bound_to_collection(
            &ctx.accounts.nft_mint,
            &ctx.accounts.collection_config,
        )?;
        verify_user_nft_token_account(
            &ctx.accounts.nft_token_account,
            &ctx.accounts.nft_mint.key(),
            &ctx.accounts.user.key(),
        )?;

        let config = &ctx.accounts.collection_config;
        let og_claim = &mut ctx.accounts.og_tier0_claim;
        let clock = Clock::get()?;

        // For MAY20DEFAIHolders.csv: OG Tier 0 holders mint NFT and get 1:1 vesting
        // Verify user hasn't already claimed their OG tier 0 NFT
        require!(!og_claim.claimed, ErrorCode::OgTier0AlreadyClaimed);

        // Verify merkle proof for OG tier 0 whitelist
        let user_key = ctx.accounts.user.key();
        let amount_bytes = vesting_amount.to_le_bytes();
        let leaf_data = [user_key.as_ref(), &amount_bytes].concat();
        let leaf = solana_program::keccak::hash(&leaf_data);

        let is_valid = merkle_proof.iter().fold(leaf.0, |acc, proof_elem| {
            let mut combined = vec![];
            if acc <= *proof_elem {
                combined.extend_from_slice(&acc);
                combined.extend_from_slice(proof_elem);
            } else {
                combined.extend_from_slice(proof_elem);
                combined.extend_from_slice(&acc);
            }
            solana_program::keccak::hash(&combined).0
        }) == config.og_tier_0_merkle_root;

        require!(is_valid, ErrorCode::NotOnOgWhitelist);

        // Check OG tier 0 supply (separate from regular tier 0)
        require!(
            config.og_tier_0_minted < config.og_tier_0_supply,
            ErrorCode::NoLiquidity
        );

        // No tax for OG tier 0 holders - they mint for free
        // Generate random bonus using secure randomness / VRF when enabled
        let (min_bonus, max_bonus) = get_tier_bonus_range(0);
        let random_value = if config_global.vrf_enabled {
            let randomness_state = load_randomness_state(&ctx.accounts.randomness_state)?;
            require!(
                !randomness_state.is_pending && randomness_state.revealed_value != [0u8; 32],
                ErrorCode::RandomnessNotReady
            );
            generate_vrf_random(
                &randomness_state.revealed_value,
                &ctx.accounts.user.key(),
                &ctx.accounts.nft_mint.key(),
            )
        } else {
            let blockhash_bytes = recent_blockhash_from_sysvar(&ctx.accounts.recent_blockhashes)?;
            generate_secure_random(
                &ctx.accounts.user.key(),
                &ctx.accounts.nft_mint.key(),
                &clock,
                &blockhash_bytes,
            )
        };
        let random_bonus = calculate_random_bonus(random_value, min_bonus, max_bonus);

        // Set up bonus state
        let bonus_state = &mut ctx.accounts.bonus_state;
        bonus_state.mint = ctx.accounts.nft_mint.key();
        bonus_state.tier = 0;
        bonus_state.bonus_bps = random_bonus;
        bonus_state.vesting_start = clock.unix_timestamp;
        bonus_state.vesting_duration = VESTING_DURATION;
        bonus_state.claimed = false;
        bonus_state.fee_deducted = 0;

        // Set up vesting state with the verified vesting amount
        let vesting_state = &mut ctx.accounts.vesting_state;
        vesting_state.mint = ctx.accounts.nft_mint.key();
        vesting_state.total_amount = vesting_amount;
        vesting_state.released_amount = 0;
        vesting_state.start_timestamp = clock.unix_timestamp;
        vesting_state.end_timestamp = clock.unix_timestamp + VESTING_DURATION;
        vesting_state.last_claimed_timestamp = clock.unix_timestamp;

        // Mark as claimed for this user
        og_claim.claimer = ctx.accounts.user.key();
        og_claim.claimed = true;

        // Update OG tier 0 minted count (separate from regular tier 0)
        let config = &mut ctx.accounts.collection_config;
        config.og_tier_0_minted += 1;

        // Emit swap event
        emit!(SwapExecuted {
            user: ctx.accounts.user.key(),
            tier: 0,
            price: 0, // Free for OG holders
            tax_amount: 0,
            bonus_bps: bonus_state.bonus_bps,
            nft_mint: ctx.accounts.nft_mint.key(),
            timestamp: clock.unix_timestamp,
        });

        msg!("=== SWAP OG TIER 0 FOR PNFT V6 COMPLETE ===");
        Ok(())
    }

    /// Initialize the per-user OG tier-0 claim PDA (call once before swap_og_tier0_for_pnft_v6).
    pub fn init_og_tier0_claim(ctx: Context<InitOgTier0Claim>) -> Result<()> {
        ctx.accounts.og_tier0_claim.claimer = Pubkey::default();
        ctx.accounts.og_tier0_claim.claimed = false;
        msg!(
            "OG tier-0 claim account initialized for {}",
            ctx.accounts.user.key()
        );
        Ok(())
    }

    pub fn swap_defai_for_pnft_v6(
        ctx: Context<SwapDefaiForPnftV6>,
        tier: u8,
        _metadata_uri: String,
        _name: String,
        _symbol: String,
    ) -> Result<()> {
        msg!("=== SWAP DEFAI FOR PNFT V6 START ===");
        require!(tier < 5, ErrorCode::InvalidTier);
        require!(!ctx.accounts.config.paused, ErrorCode::ProtocolPaused);
        verify_nft_mint_bound_to_collection(
            &ctx.accounts.nft_mint,
            &ctx.accounts.collection_config,
        )?;

        let config = &mut ctx.accounts.collection_config;
        let user_tax = &mut ctx.accounts.user_tax_state;
        let clock = Clock::get()?;

        // Check supply - for tier 0, check remaining supply after reserving for OG holders
        if tier == 0 {
            let remaining_supply = config.tier_supplies[0].saturating_sub(config.og_tier_0_supply);
            require!(
                config.tier_minted[0] < remaining_supply,
                ErrorCode::NoLiquidity
            );
        } else {
            require!(
                config.tier_minted[tier as usize] < config.tier_supplies[tier as usize],
                ErrorCode::NoLiquidity
            );
        }

        // Check and reset tax if 24 hours passed
        if clock.unix_timestamp - user_tax.last_swap_timestamp >= TAX_RESET_DURATION {
            user_tax.tax_rate_bps = INITIAL_TAX_BPS;
            user_tax.swap_count = 0;
        }

        // Calculate amounts
        let price = config.tier_prices[tier as usize];
        let tax_amount = (price as u128)
            .checked_mul(user_tax.tax_rate_bps as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::MathOverflow)? as u64;
        let net_amount = price
            .checked_sub(tax_amount)
            .ok_or(ErrorCode::MathOverflow)?;

        // Transfer tax to treasury
        let cpi_ctx_tax = CpiContext::new(
            ctx.accounts.token_program_2022.to_account_info(),
            TransferChecked {
                from: ctx.accounts.user_defai_ata.to_account_info(),
                to: ctx.accounts.treasury_defai_ata.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
                mint: ctx.accounts.defai_mint.to_account_info(),
            },
        );
        token22::transfer_checked(cpi_ctx_tax, tax_amount, 6)?;

        // Transfer net to escrow
        let cpi_ctx_net = CpiContext::new(
            ctx.accounts.token_program_2022.to_account_info(),
            TransferChecked {
                from: ctx.accounts.user_defai_ata.to_account_info(),
                to: ctx.accounts.escrow_defai_ata.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
                mint: ctx.accounts.defai_mint.to_account_info(),
            },
        );
        token22::transfer_checked(cpi_ctx_net, net_amount, 6)?;

        // Generate random bonus using VRF when enabled; otherwise fallback
        let (min_bonus, max_bonus) = get_tier_bonus_range(tier);
        let random_value = if ctx.accounts.config.vrf_enabled {
            require!(
                !ctx.accounts.randomness_state.is_pending
                    && ctx.accounts.randomness_state.revealed_value != [0u8; 32],
                ErrorCode::RandomnessNotReady
            );
            generate_vrf_random(
                &ctx.accounts.randomness_state.revealed_value,
                &ctx.accounts.user.key(),
                &ctx.accounts.nft_mint.key(),
            )
        } else {
            let blockhash_bytes = recent_blockhash_from_sysvar(&ctx.accounts.recent_blockhashes)?;
            generate_secure_random(
                &ctx.accounts.user.key(),
                &ctx.accounts.nft_mint.key(),
                &clock,
                &blockhash_bytes,
            )
        };
        let random_bonus = calculate_random_bonus(random_value, min_bonus, max_bonus);

        // Set up bonus state
        let bonus_state = &mut ctx.accounts.bonus_state;
        bonus_state.mint = ctx.accounts.nft_mint.key();
        bonus_state.tier = tier;
        bonus_state.bonus_bps = random_bonus;
        bonus_state.vesting_start = clock.unix_timestamp;
        bonus_state.vesting_duration = VESTING_DURATION;
        bonus_state.claimed = false;
        bonus_state.fee_deducted = 0;

        // Set up vesting state
        let vesting_state = &mut ctx.accounts.vesting_state;
        let vesting_amount = (price as u128)
            .checked_mul(bonus_state.bonus_bps as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::MathOverflow)? as u64;

        vesting_state.mint = ctx.accounts.nft_mint.key();
        vesting_state.total_amount = vesting_amount;
        vesting_state.released_amount = 0;
        vesting_state.start_timestamp = clock.unix_timestamp;
        vesting_state.end_timestamp = clock.unix_timestamp + VESTING_DURATION;
        vesting_state.last_claimed_timestamp = clock.unix_timestamp;

        // Update user tax for next swap
        user_tax.tax_rate_bps = user_tax
            .tax_rate_bps
            .saturating_add(TAX_INCREMENT_BPS)
            .min(TAX_CAP_BPS);
        user_tax.swap_count += 1;
        user_tax.last_swap_timestamp = clock.unix_timestamp;

        config.tier_minted[tier as usize] += 1;

        // Emit swap event
        emit!(SwapExecuted {
            user: ctx.accounts.user.key(),
            tier,
            price,
            tax_amount,
            bonus_bps: bonus_state.bonus_bps,
            nft_mint: ctx.accounts.nft_mint.key(),
            timestamp: clock.unix_timestamp,
        });

        msg!("=== SWAP DEFAI FOR PNFT V6 COMPLETE ===");
        Ok(())
    }

    pub fn swap_old_defai_for_pnft_v6(
        ctx: Context<SwapOldDefaiForPnftV6>,
        tier: u8,
        _metadata_uri: String,
        _name: String,
        _symbol: String,
    ) -> Result<()> {
        msg!("=== SWAP OLD DEFAI FOR PNFT V6 START ===");
        require!(tier < 5, ErrorCode::InvalidTier);
        require!(!ctx.accounts.config.paused, ErrorCode::ProtocolPaused);
        verify_nft_mint_bound_to_collection(
            &ctx.accounts.nft_mint,
            &ctx.accounts.collection_config,
        )?;

        let config = &mut ctx.accounts.collection_config;
        let user_tax = &mut ctx.accounts.user_tax_state;
        let clock = Clock::get()?;

        // Check supply - for tier 0, check remaining supply after reserving for OG holders
        if tier == 0 {
            let remaining_supply = config.tier_supplies[0].saturating_sub(config.og_tier_0_supply);
            require!(
                config.tier_minted[0] < remaining_supply,
                ErrorCode::NoLiquidity
            );
        } else {
            require!(
                config.tier_minted[tier as usize] < config.tier_supplies[tier as usize],
                ErrorCode::NoLiquidity
            );
        }

        let price = config.tier_prices[tier as usize];

        // Transfer OLD tokens into program-controlled escrow (not burn)
        // This enables the team to later sell on DEX and route liquidity into the new token.
        let cpi_ctx_old = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_old.to_account_info(),
                to: ctx.accounts.escrow_old.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );
        token::transfer(cpi_ctx_old, price)?;

        // Generate random bonus using VRF when enabled; otherwise fallback
        let (min_bonus, max_bonus) = get_tier_bonus_range(tier);
        let random_value = if ctx.accounts.config.vrf_enabled {
            require!(
                !ctx.accounts.randomness_state.is_pending
                    && ctx.accounts.randomness_state.revealed_value != [0u8; 32],
                ErrorCode::RandomnessNotReady
            );
            generate_vrf_random(
                &ctx.accounts.randomness_state.revealed_value,
                &ctx.accounts.user.key(),
                &ctx.accounts.nft_mint.key(),
            )
        } else {
            let blockhash_bytes = recent_blockhash_from_sysvar(&ctx.accounts.recent_blockhashes)?;
            generate_secure_random(
                &ctx.accounts.user.key(),
                &ctx.accounts.nft_mint.key(),
                &clock,
                &blockhash_bytes,
            )
        };
        let random_bonus = calculate_random_bonus(random_value, min_bonus, max_bonus);

        // Set up bonus state
        let bonus_state = &mut ctx.accounts.bonus_state;
        bonus_state.mint = ctx.accounts.nft_mint.key();
        bonus_state.tier = tier;
        bonus_state.bonus_bps = random_bonus;
        bonus_state.vesting_start = clock.unix_timestamp;
        bonus_state.vesting_duration = VESTING_DURATION;
        bonus_state.claimed = false;
        bonus_state.fee_deducted = 0;

        // Set up vesting state
        let vesting_state = &mut ctx.accounts.vesting_state;
        let vesting_amount = (price as u128)
            .checked_mul(bonus_state.bonus_bps as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::MathOverflow)? as u64;

        vesting_state.mint = ctx.accounts.nft_mint.key();
        vesting_state.total_amount = vesting_amount;
        vesting_state.released_amount = 0;
        vesting_state.start_timestamp = clock.unix_timestamp;
        vesting_state.end_timestamp = clock.unix_timestamp + VESTING_DURATION;
        vesting_state.last_claimed_timestamp = clock.unix_timestamp;

        // OLD DEFAI swaps are tax-free and should not affect tax state
        // Only increment swap count for tracking purposes
        user_tax.swap_count += 1;
        // Do NOT update last_swap_timestamp to avoid breaking the tax reset mechanism

        config.tier_minted[tier as usize] += 1;

        // Emit swap event
        emit!(SwapExecuted {
            user: ctx.accounts.user.key(),
            tier,
            price,
            tax_amount: 0, // No tax for old DEFAI swaps
            bonus_bps: bonus_state.bonus_bps,
            nft_mint: ctx.accounts.nft_mint.key(),
            timestamp: clock.unix_timestamp,
        });

        msg!("=== SWAP OLD DEFAI FOR PNFT V6 COMPLETE ===");
        Ok(())
    }

    pub fn redeem_v6(ctx: Context<RedeemV6>) -> Result<()> {
        msg!("=== REDEEM V6 START ===");
        require!(!ctx.accounts.config.paused, ErrorCode::ProtocolPaused);

        let bonus_state = &mut ctx.accounts.bonus_state;
        let vesting_state = &ctx.accounts.vesting_state;
        let cfg = &ctx.accounts.config;

        // Verify NFT not already redeemed
        require!(!bonus_state.claimed, ErrorCode::NftAlreadyRedeemed);

        // Get base price from tier
        let base_price = cfg.prices[bonus_state.tier as usize];

        // Calculate total amount (base + bonus)
        let bonus_amount = vesting_state.total_amount;
        let _total_amount = base_price + bonus_amount;

        // Deduct accumulated fees from base price
        let amount_to_transfer = base_price.saturating_sub(bonus_state.fee_deducted);

        // Transfer base amount minus fees
        let escrow_seeds = &[b"escrow" as &[u8], &[ctx.accounts.escrow.bump][..]];
        let signer_seeds = &[&escrow_seeds[..]];

        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program_2022.to_account_info(),
            TransferChecked {
                from: ctx.accounts.escrow_defai_ata.to_account_info(),
                to: ctx.accounts.user_defai_ata.to_account_info(),
                authority: ctx.accounts.escrow.to_account_info(),
                mint: ctx.accounts.defai_mint.to_account_info(),
            },
            signer_seeds,
        );

        token22::transfer_checked(transfer_ctx, amount_to_transfer, 6)?;

        // BURN THE NFT - prevent any future use
        let burn_ctx = CpiContext::new(
            ctx.accounts.token_program_2022.to_account_info(),
            Burn {
                mint: ctx.accounts.nft_mint.to_account_info(),
                from: ctx.accounts.user_nft_ata.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );
        token22::burn(burn_ctx, 1)?;

        // CLOSE THE NFT TOKEN ACCOUNT - reclaim rent to user
        let close_ctx = CpiContext::new(
            ctx.accounts.token_program_2022.to_account_info(),
            CloseAccount {
                account: ctx.accounts.user_nft_ata.to_account_info(),
                destination: ctx.accounts.user.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );
        token22::close_account(close_ctx)?;

        // Mark as claimed
        bonus_state.claimed = true;

        // Emit redemption event
        emit!(RedemptionExecuted {
            user: ctx.accounts.user.key(),
            nft_mint: ctx.accounts.nft_mint.key(),
            amount_returned: amount_to_transfer,
            fees_deducted: bonus_state.fee_deducted,
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!(
            "Redeemed NFT: base {} DEFAI, fees deducted {} DEFAI, received {} DEFAI",
            base_price,
            bonus_state.fee_deducted,
            amount_to_transfer
        );
        msg!("NFT burned and account closed - redemption complete and irreversible");
        msg!("=== REDEEM V6 COMPLETE ===");
        Ok(())
    }

    /// Function 2: For 10_1AIR-Sheet1.csv - NO NFT minting, only vesting of AIRDROP column amount
    /// This is separate from OG tier 0 and doesn't involve any NFT minting
    pub fn claim_airdrop(
        ctx: Context<ClaimAirdrop>,
        amount: u64, // The AIRDROP column amount from 10_1AIR-Sheet1.csv
        merkle_proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        msg!("=== CLAIM AIRDROP START (10:1 Air Recipients - No NFT) ===");
        require!(!ctx.accounts.config.paused, ErrorCode::ProtocolPaused);

        let airdrop_vesting = &mut ctx.accounts.airdrop_vesting;
        let clock = Clock::get()?;
        let config = &ctx.accounts.collection_config;

        // Verify user hasn't already claimed
        require!(
            airdrop_vesting.beneficiary == Pubkey::default(),
            ErrorCode::AlreadyClaimed
        );

        // Verify merkle proof
        let user_key = ctx.accounts.user.key();
        let amount_bytes = amount.to_le_bytes();
        let leaf_data = [user_key.as_ref(), &amount_bytes].concat();
        let leaf = solana_program::keccak::hash(&leaf_data);

        let is_valid = merkle_proof.iter().fold(leaf.0, |acc, proof_elem| {
            let mut combined = vec![];
            if acc <= *proof_elem {
                combined.extend_from_slice(&acc);
                combined.extend_from_slice(proof_elem);
            } else {
                combined.extend_from_slice(proof_elem);
                combined.extend_from_slice(&acc);
            }
            solana_program::keccak::hash(&combined).0
        }) == config.airdrop_merkle_root;

        require!(is_valid, ErrorCode::InvalidMerkleProof);

        // Initialize vesting state
        airdrop_vesting.beneficiary = ctx.accounts.user.key();
        airdrop_vesting.total_amount = amount;
        airdrop_vesting.released_amount = 0;
        airdrop_vesting.start_timestamp = clock.unix_timestamp;
        airdrop_vesting.end_timestamp = clock.unix_timestamp + VESTING_DURATION;
        airdrop_vesting.last_claimed_timestamp = clock.unix_timestamp;

        // Emit event
        emit!(AirdropClaimed {
            user: ctx.accounts.user.key(),
            amount,
            vesting_start: clock.unix_timestamp,
            vesting_end: clock.unix_timestamp + VESTING_DURATION,
        });

        msg!("=== CLAIM AIRDROP COMPLETE ===");
        Ok(())
    }

    pub fn claim_vested_airdrop(ctx: Context<ClaimVestedAirdrop>) -> Result<()> {
        msg!("=== CLAIM VESTED AIRDROP START ===");
        require!(!ctx.accounts.config.paused, ErrorCode::ProtocolPaused);

        let airdrop_vesting = &mut ctx.accounts.airdrop_vesting;
        let now = Clock::get()?.unix_timestamp;

        // Check cliff period
        let cliff_end = airdrop_vesting.start_timestamp + CLIFF_DURATION;
        require!(now >= cliff_end, ErrorCode::StillInCliff);

        // Calculate vested amount
        let elapsed = now - airdrop_vesting.start_timestamp;
        let duration = airdrop_vesting.end_timestamp - airdrop_vesting.start_timestamp;

        let vested_amount = if elapsed >= duration {
            airdrop_vesting.total_amount
        } else {
            (airdrop_vesting.total_amount as u128)
                .checked_mul(elapsed as u128)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(duration as u128)
                .ok_or(ErrorCode::MathOverflow)? as u64
        };

        let claimable = vested_amount.saturating_sub(airdrop_vesting.released_amount);
        require!(claimable > 0, ErrorCode::NothingToClaim);

        // Transfer from escrow to user
        let escrow_seeds = &[b"escrow" as &[u8], &[ctx.accounts.escrow.bump][..]];
        let signer_seeds = &[&escrow_seeds[..]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.escrow_token_account.to_account_info(),
                to: ctx.accounts.user_defai_ata.to_account_info(),
                authority: ctx.accounts.escrow.to_account_info(),
                mint: ctx.accounts.defai_mint.to_account_info(),
            },
            signer_seeds,
        );
        token22::transfer_checked(cpi_ctx, claimable, 6)?;

        // Update released amount
        airdrop_vesting.released_amount += claimable;
        airdrop_vesting.last_claimed_timestamp = now;

        // Emit event
        emit!(AirdropVestingClaimed {
            user: ctx.accounts.user.key(),
            amount_claimed: claimable,
            total_vested: vested_amount,
            timestamp: now,
        });

        msg!("=== CLAIM VESTED AIRDROP COMPLETE ===");
        Ok(())
    }

    pub fn claim_vested_v6(ctx: Context<ClaimVestedV6>) -> Result<()> {
        msg!("=== CLAIM VESTED V6 START ===");
        require!(!ctx.accounts.config.paused, ErrorCode::ProtocolPaused);

        // NFT ownership and mint validation is now done in the account constraints

        let vesting_state = &mut ctx.accounts.vesting_state;
        let clock = Clock::get()?;

        // Check cliff period
        let cliff_end = vesting_state.start_timestamp + CLIFF_DURATION;
        require!(clock.unix_timestamp >= cliff_end, ErrorCode::StillInCliff);

        // Calculate vested amount
        let elapsed = clock
            .unix_timestamp
            .saturating_sub(vesting_state.start_timestamp);
        let duration = vesting_state
            .end_timestamp
            .saturating_sub(vesting_state.start_timestamp);

        let vested_amount = if elapsed >= duration {
            vesting_state.total_amount
        } else {
            vesting_state
                .total_amount
                .checked_mul(elapsed as u64)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(duration as u64)
                .ok_or(ErrorCode::MathOverflow)?
        };

        let claimable = vested_amount.saturating_sub(vesting_state.released_amount);
        require!(claimable > 0, ErrorCode::NothingToClaim);

        // Transfer vested amount
        let escrow_seeds = &[b"escrow" as &[u8], &[ctx.accounts.escrow.bump][..]];
        let signer_seeds = &[&escrow_seeds[..]];
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program_2022.to_account_info(),
            TransferChecked {
                from: ctx.accounts.escrow_defai_ata.to_account_info(),
                to: ctx.accounts.user_defai_ata.to_account_info(),
                authority: ctx.accounts.escrow.to_account_info(),
                mint: ctx.accounts.defai_mint.to_account_info(),
            },
            signer_seeds,
        );
        token22::transfer_checked(cpi_ctx, claimable, 6)?;

        // Update state
        vesting_state.released_amount += claimable;
        vesting_state.last_claimed_timestamp = clock.unix_timestamp;

        // Emit vesting claim event
        emit!(VestingClaimed {
            user: ctx.accounts.user.key(),
            nft_mint: ctx.accounts.nft_mint.key(),
            amount_claimed: claimable,
            total_vested: vested_amount,
            timestamp: clock.unix_timestamp,
        });

        msg!("Claimed {} tokens", claimable);
        msg!("=== CLAIM VESTED V6 COMPLETE ===");
        Ok(())
    }

    pub fn admin_withdraw(ctx: Context<AdminWithdraw>, amount: u64) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        let escrow_seeds = &[b"escrow" as &[u8], &[ctx.accounts.escrow.bump][..]];
        let signer_seeds = &[&escrow_seeds[..]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.source_vault.to_account_info(),
                to: ctx.accounts.dest.to_account_info(),
                authority: ctx.accounts.escrow.to_account_info(),
            },
            signer_seeds,
        );
        token::transfer(cpi_ctx, amount)?;

        // Emit admin action event
        emit!(AdminAction {
            admin: ctx.accounts.admin.key(),
            action: format!("Withdraw {} tokens", amount),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    pub fn admin_withdraw_token2022(
        ctx: Context<AdminWithdrawToken2022>,
        amount: u64,
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            ErrorCode::Unauthorized
        );

        let escrow_seeds = &[b"escrow" as &[u8], &[ctx.accounts.escrow.bump][..]];
        let signer_seeds = &[&escrow_seeds[..]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program_2022.to_account_info(),
            TransferChecked {
                from: ctx.accounts.source_vault.to_account_info(),
                to: ctx.accounts.dest.to_account_info(),
                authority: ctx.accounts.escrow.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
            signer_seeds,
        );

        // Transfer with 6 decimals (standard for DEFAI tokens)
        token22::transfer_checked(cpi_ctx, amount, 6)?;

        // Emit admin action event
        emit!(AdminAction {
            admin: ctx.accounts.admin.key(),
            action: format!("Withdraw {} Token-2022 tokens", amount),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    pub fn reroll_bonus_v6(ctx: Context<RerollBonusV6>) -> Result<()> {
        msg!("=== REROLL BONUS V6 START ===");
        require!(!ctx.accounts.config.paused, ErrorCode::ProtocolPaused);

        // NFT ownership and mint validation is now done in the account constraints

        let bonus_state = &mut ctx.accounts.bonus_state;
        let vesting_state = &mut ctx.accounts.vesting_state;
        let user_tax = &mut ctx.accounts.user_tax_state;
        let config = &ctx.accounts.config;

        // Check user has sufficient DEFAI balance (base price for their tier)
        let base_price = config.prices[bonus_state.tier as usize];
        require!(
            ctx.accounts.user_defai_ata.amount >= base_price,
            ErrorCode::InsufficientDefaiForReroll
        );

        msg!(
            "User has {} DEFAI, required: {} DEFAI",
            ctx.accounts.user_defai_ata.amount,
            base_price
        );

        let clock = Clock::get()?;

        // Calculate vested amount
        let elapsed = clock
            .unix_timestamp
            .saturating_sub(vesting_state.start_timestamp);
        let duration = vesting_state
            .end_timestamp
            .saturating_sub(vesting_state.start_timestamp);

        let vested_amount = if elapsed >= duration {
            vesting_state.total_amount
        } else {
            vesting_state
                .total_amount
                .checked_mul(elapsed as u64)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(duration as u64)
                .ok_or(ErrorCode::MathOverflow)?
        };

        // Get unreleased amount
        let unreleased = vested_amount.saturating_sub(vesting_state.released_amount);
        require!(unreleased > 0, ErrorCode::NothingToClaim);

        // Calculate tax based on base price (not including bonus)
        let tax_amount = (base_price as u128)
            .checked_mul(user_tax.tax_rate_bps as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::MathOverflow)? as u64;

        // Store old bonus for logging
        let old_bonus_bps = bonus_state.bonus_bps;

        // Generate new random bonus
        let tier = bonus_state.tier;
        let (min_bonus, max_bonus) = get_tier_bonus_range(tier);

        // Use VRF randomness when enabled; otherwise fallback
        let random_value = if ctx.accounts.config.vrf_enabled {
            require!(
                !ctx.accounts.randomness_state.is_pending
                    && ctx.accounts.randomness_state.revealed_value != [0u8; 32],
                ErrorCode::RandomnessNotReady
            );
            generate_vrf_random(
                &ctx.accounts.randomness_state.revealed_value,
                &ctx.accounts.user.key(),
                &ctx.accounts.nft_mint.key(),
            )
        } else {
            let blockhash_bytes = recent_blockhash_from_sysvar(&ctx.accounts.recent_blockhashes)?;
            generate_secure_random(
                &ctx.accounts.user.key(),
                &ctx.accounts.nft_mint.key(),
                &clock,
                &blockhash_bytes,
            )
        };
        let random_bonus = calculate_random_bonus(random_value, min_bonus, max_bonus);

        if tax_amount > 0 {
            let cpi_ctx_tax = CpiContext::new(
                ctx.accounts.token_program_2022.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.user_defai_ata.to_account_info(),
                    to: ctx.accounts.treasury_defai_ata.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                    mint: ctx.accounts.defai_mint.to_account_info(),
                },
            );
            token22::transfer_checked(cpi_ctx_tax, tax_amount, 6)?;
        }

        // Update bonus state
        bonus_state.bonus_bps = random_bonus;
        bonus_state.vesting_start = clock.unix_timestamp;
        bonus_state.vesting_duration = VESTING_DURATION;
        bonus_state.fee_deducted = bonus_state
            .fee_deducted
            .checked_add(tax_amount)
            .ok_or(ErrorCode::MathOverflow)?;

        // Update vesting state
        let new_vesting_amount = (base_price as u128)
            .checked_mul(random_bonus as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::MathOverflow)? as u64;

        vesting_state.total_amount = new_vesting_amount;
        vesting_state.released_amount = 0;
        vesting_state.start_timestamp = clock.unix_timestamp;
        vesting_state.end_timestamp = clock.unix_timestamp + VESTING_DURATION;
        vesting_state.last_claimed_timestamp = clock.unix_timestamp;

        // Increment user's tax rate for next time (max 3000 bps = 30%)
        user_tax.tax_rate_bps = user_tax
            .tax_rate_bps
            .saturating_add(TAX_INCREMENT_BPS)
            .min(TAX_CAP_BPS);

        msg!(
            "Rerolled NFT {} from {}% to {}% bonus (fee: {} DEFAI deducted from future redemption)",
            ctx.accounts.nft_mint.key(),
            old_bonus_bps as f64 / 100.0,
            random_bonus as f64 / 100.0,
            tax_amount
        );

        msg!(
            "User tax rate increased to {}%",
            user_tax.tax_rate_bps as f64 / 100.0
        );

        // Emit reroll event
        emit!(BonusRerolled {
            user: ctx.accounts.user.key(),
            nft_mint: ctx.accounts.nft_mint.key(),
            old_bonus_bps,
            new_bonus_bps: random_bonus,
            tax_paid: tax_amount,
            timestamp: clock.unix_timestamp,
        });

        msg!("=== REROLL BONUS V6 COMPLETE ===");
        Ok(())
    }

    pub fn update_nft_metadata_v6(ctx: Context<UpdateNftMetadataV6>) -> Result<()> {
        msg!("=== UPDATE NFT METADATA V6 START ===");

        let bonus_state = &ctx.accounts.bonus_state;
        let vesting_state = &ctx.accounts.vesting_state;
        let clock = Clock::get()?;

        // Calculate current vesting info
        let elapsed = clock
            .unix_timestamp
            .saturating_sub(vesting_state.start_timestamp);
        let duration = vesting_state
            .end_timestamp
            .saturating_sub(vesting_state.start_timestamp);

        let vested_amount = if elapsed >= duration {
            vesting_state.total_amount
        } else {
            vesting_state
                .total_amount
                .checked_mul(elapsed as u64)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(duration as u64)
                .ok_or(ErrorCode::MathOverflow)?
        };

        let remaining_vested = vested_amount.saturating_sub(vesting_state.released_amount);
        let days_remaining = if elapsed >= duration {
            0
        } else {
            (duration - elapsed) / (24 * 60 * 60)
        };

        msg!("NFT Metadata Update for {}", ctx.accounts.nft_mint.key());
        msg!("Tier: {}", bonus_state.tier);
        msg!("Bonus: {}%", bonus_state.bonus_bps as f64 / 100.0);
        msg!("Redeemed: {}", bonus_state.claimed);
        msg!("Fee Deducted: {} DEFAI", bonus_state.fee_deducted);
        msg!("Vesting Total: {} DEFAI", vesting_state.total_amount);
        msg!("Vesting Released: {} DEFAI", vesting_state.released_amount);
        msg!("Vesting Remaining: {} DEFAI", remaining_vested);
        msg!("Days Remaining: {}", days_remaining);

        // Note: Actual metadata update would require Token Metadata Program CPI
        // This instruction logs the data that should be included in metadata

        msg!("=== UPDATE NFT METADATA V6 COMPLETE ===");
        Ok(())
    }
}

// Helper function to get bonus range for a tier
fn get_tier_bonus_range(tier: u8) -> (u16, u16) {
    match tier {
        0 => (TIER_0_MIN_BONUS, TIER_0_MAX_BONUS),
        1 => (TIER_1_MIN_BONUS, TIER_1_MAX_BONUS),
        2 => (TIER_2_MIN_BONUS, TIER_2_MAX_BONUS),
        3 => (TIER_3_MIN_BONUS, TIER_3_MAX_BONUS),
        4 => (TIER_4_MIN_BONUS, TIER_4_MAX_BONUS),
        _ => (0, 0),
    }
}

/// Verify an NFT mint is bound to the configured collection via mint authority.
fn verify_nft_mint_bound_to_collection(
    nft_mint: &AccountInfo,
    collection_config: &CollectionConfig,
) -> Result<()> {
    let mint = Mint::try_deserialize(&mut &nft_mint.try_borrow_data()?[..])?;
    require!(
        mint.decimals == 0 && mint.supply == 1,
        ErrorCode::InvalidNft
    );
    require_keys_neq!(
        nft_mint.key(),
        collection_config.collection_mint,
        ErrorCode::InvalidNft
    );
    let authority = match mint.mint_authority {
        COption::Some(a) => a,
        COption::None => return Err(ErrorCode::InvalidCollection.into()),
    };
    require_keys_eq!(
        authority,
        collection_config.authority,
        ErrorCode::InvalidCollection
    );
    Ok(())
}

fn verify_user_nft_token_account(
    nft_token_account: &AccountInfo,
    expected_mint: &Pubkey,
    expected_owner: &Pubkey,
) -> Result<()> {
    let token = TokenAccount2022::try_deserialize(&mut &nft_token_account.try_borrow_data()?[..])?;
    require_keys_eq!(token.mint, *expected_mint, ErrorCode::InvalidNft);
    require_keys_eq!(token.owner, *expected_owner, ErrorCode::NoNft);
    require!(token.amount == 1, ErrorCode::NoNft);
    Ok(())
}

fn load_config(config: &AccountInfo) -> Result<Config> {
    Config::try_deserialize(&mut &config.try_borrow_data()?[..]).map_err(Into::into)
}

fn load_randomness_state(randomness_state: &AccountInfo) -> Result<RandomnessState> {
    RandomnessState::try_deserialize(&mut &randomness_state.try_borrow_data()?[..])
        .map_err(Into::into)
}

// Account structures
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    /// CHECK: legacy SPL-Token mint (read-only)
    pub old_mint: AccountInfo<'info>,
    /// CHECK: DEFAI Token-2022 mint (read-only)
    pub new_mint: AccountInfo<'info>,
    /// CHECK: Bonus-NFT collection mint (read-only)
    pub collection: AccountInfo<'info>,
    /// CHECK: Treasury wallet that will receive tax (read-only)
    pub treasury: AccountInfo<'info>,
    #[account(
        init,
        payer = admin,
        space = 8 + Config::LEN,
        seeds = [b"config"],
        bump,
    )]
    pub config: Account<'info, Config>,
    #[account(
        init,
        payer = admin,
        space = 8 + Escrow::LEN,
        seeds = [b"escrow"],
        bump,
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(
        init,
        payer = admin,
        space = 8 + TaxState::LEN,
        seeds = [b"tax_state"],
        bump,
    )]
    pub tax_state: Account<'info, TaxState>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitEscrowOld<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        seeds = [b"escrow"],
        bump
    )]
    pub escrow: Account<'info, Escrow>,
    /// CHECK: OLD DEFAI mint
    pub old_mint: AccountInfo<'info>,
    #[account(
        init,
        payer = admin,
        token::mint = old_mint,
        token::authority = escrow,
        seeds = [b"escrow_old"],
        bump
    )]
    pub escrow_old: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        mut,
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,
}

#[derive(Accounts)]
pub struct InitializeWhitelist<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        init,
        payer = admin,
        space = 8 + Whitelist::LEN,
        seeds = [b"whitelist"],
        bump,
    )]
    pub whitelist: Account<'info, Whitelist>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeUserTax<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        init,
        payer = user,
        space = 8 + UserTaxState::LEN,
        seeds = [b"user_tax", user.key().as_ref()],
        bump
    )]
    pub user_tax_state: Account<'info, UserTaxState>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResetUserTax<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        seeds = [b"user_tax", user.key().as_ref()],
        bump
    )]
    pub user_tax_state: Account<'info, UserTaxState>,
}

#[derive(Accounts)]
pub struct InitializeCollection<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [b"config"],
        bump,
    )]
    pub config: Account<'info, Config>,
    /// CHECK: Collection mint
    pub collection_mint: AccountInfo<'info>,
    /// CHECK: Treasury
    pub treasury: AccountInfo<'info>,
    /// CHECK: DEFAI mint
    pub defai_mint: AccountInfo<'info>,
    /// CHECK: Old DEFAI mint
    pub old_defai_mint: AccountInfo<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + CollectionConfig::LEN,
        seeds = [b"collection_config"],
        bump
    )]
    pub collection_config: Account<'info, CollectionConfig>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SwapOgTier0ForPnftV6<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    /// CHECK: Global config PDA; deserialized in handler
    #[account(seeds = [b"config"], bump)]
    pub config: AccountInfo<'info>,
    /// CHECK: Randomness PDA; deserialized when VRF is enabled
    #[account(mut, seeds = [b"randomness_state"], bump)]
    pub randomness_state: AccountInfo<'info>,
    #[account(mut)]
    pub collection_config: Box<Account<'info, CollectionConfig>>,
    /// CHECK: NFT mint to be created
    /// CHECK: NFT mint validated in handler via verify_nft_mint_bound_to_collection
    pub nft_mint: AccountInfo<'info>,
    /// CHECK: User NFT token account (validated in handler)
    #[account(mut)]
    pub nft_token_account: AccountInfo<'info>,
    #[account(
        init,
        payer = user,
        space = 8 + BonusStateV6::LEN,
        seeds = [b"bonus_v6", nft_mint.key().as_ref()],
        bump
    )]
    pub bonus_state: Box<Account<'info, BonusStateV6>>,
    #[account(
        init,
        payer = user,
        space = 8 + VestingStateV6::LEN,
        seeds = [b"vesting_v6", nft_mint.key().as_ref()],
        bump
    )]
    pub vesting_state: Box<Account<'info, VestingStateV6>>,
    #[account(
        mut,
        seeds = [b"og_tier0_claim", user.key().as_ref()],
        bump
    )]
    pub og_tier0_claim: Box<Account<'info, OgTier0Claim>>,
    pub system_program: Program<'info, System>,
    /// CHECK: Sysvar for recent blockhashes
    #[account(address = solana_program::sysvar::recent_blockhashes::ID)]
    pub recent_blockhashes: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct InitOgTier0Claim<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        init,
        payer = user,
        space = 8 + OgTier0Claim::LEN,
        seeds = [b"og_tier0_claim", user.key().as_ref()],
        bump
    )]
    pub og_tier0_claim: Account<'info, OgTier0Claim>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SwapDefaiForPnftV6<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_defai_ata: Box<InterfaceAccount<'info, TokenAccount2022>>,
    #[account(
        mut,
        seeds = [b"randomness_state"],
        bump = randomness_state.bump
    )]
    pub randomness_state: Box<Account<'info, RandomnessState>>,
    #[account(
        mut,
        // Validate treasury ATA matches the configured treasury
        token::mint = defai_mint,
        token::authority = collection_config.treasury
    )]
    pub treasury_defai_ata: Box<InterfaceAccount<'info, TokenAccount2022>>,
    #[account(
        mut,
        // Validate escrow ATA is owned by the escrow PDA
        token::mint = defai_mint,
        token::authority = escrow
    )]
    pub escrow_defai_ata: Box<InterfaceAccount<'info, TokenAccount2022>>,
    /// CHECK: DEFAI mint
    pub defai_mint: AccountInfo<'info>,
    pub config: Box<Account<'info, Config>>,
    #[account(mut)]
    pub collection_config: Box<Account<'info, CollectionConfig>>,
    /// CHECK: NFT mint to be created
    /// CHECK: NFT mint validated in handler via verify_nft_mint_bound_to_collection
    pub nft_mint: AccountInfo<'info>,
    #[account(
        mut,
        constraint = nft_token_account.mint == nft_mint.key() @ ErrorCode::InvalidNft,
        constraint = nft_token_account.owner == user.key() @ ErrorCode::NoNft,
        constraint = nft_token_account.amount == 1 @ ErrorCode::NoNft,
    )]
    pub nft_token_account: Box<InterfaceAccount<'info, TokenAccount2022>>,
    #[account(
        init,
        payer = user,
        space = 8 + BonusStateV6::LEN,
        seeds = [b"bonus_v6", nft_mint.key().as_ref()],
        bump
    )]
    pub bonus_state: Box<Account<'info, BonusStateV6>>,
    #[account(
        init,
        payer = user,
        space = 8 + VestingStateV6::LEN,
        seeds = [b"vesting_v6", nft_mint.key().as_ref()],
        bump
    )]
    pub vesting_state: Box<Account<'info, VestingStateV6>>,
    #[account(
        seeds = [b"escrow"],
        bump
    )]
    pub escrow: Box<Account<'info, Escrow>>,
    #[account(
        mut,
        seeds = [b"user_tax", user.key().as_ref()],
        bump
    )]
    pub user_tax_state: Box<Account<'info, UserTaxState>>,
    pub system_program: Program<'info, System>,
    pub token_program_2022: Program<'info, Token2022>,
    /// CHECK: Sysvar for recent blockhashes
    #[account(address = solana_program::sysvar::recent_blockhashes::ID)]
    pub recent_blockhashes: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct SwapOldDefaiForPnftV6<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        constraint = user_old.owner == user.key() @ ErrorCode::Unauthorized,
        constraint = user_old.mint == old_defai_mint.key() @ ErrorCode::InvalidMint
    )]
    pub user_old: Box<Account<'info, TokenAccount>>,
    /// CHECK: The legacy OLD DEFAI mint; used to verify burn
    #[account(
        constraint = old_defai_mint.key() == collection_config.old_defai_mint @ ErrorCode::InvalidMint
    )]
    pub old_defai_mint: AccountInfo<'info>,
    #[account(
        mut,
        // Escrow for OLD tokens held by the program; must be owned by escrow PDA and match OLD mint
        constraint = escrow_old.owner == escrow.key() @ ErrorCode::Unauthorized,
        constraint = escrow_old.mint == old_defai_mint.key() @ ErrorCode::InvalidMint
    )]
    pub escrow_old: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        seeds = [b"randomness_state"],
        bump = randomness_state.bump
    )]
    pub randomness_state: Box<Account<'info, RandomnessState>>,
    pub config: Box<Account<'info, Config>>,
    #[account(mut)]
    pub collection_config: Box<Account<'info, CollectionConfig>>,
    /// CHECK: NFT mint to be created
    /// CHECK: NFT mint validated in handler via verify_nft_mint_bound_to_collection
    pub nft_mint: AccountInfo<'info>,
    #[account(
        mut,
        constraint = nft_token_account.mint == nft_mint.key() @ ErrorCode::InvalidNft,
        constraint = nft_token_account.owner == user.key() @ ErrorCode::NoNft,
        constraint = nft_token_account.amount == 1 @ ErrorCode::NoNft,
    )]
    pub nft_token_account: Box<InterfaceAccount<'info, TokenAccount2022>>,
    #[account(
        init,
        payer = user,
        space = 8 + BonusStateV6::LEN,
        seeds = [b"bonus_v6", nft_mint.key().as_ref()],
        bump
    )]
    pub bonus_state: Box<Account<'info, BonusStateV6>>,
    #[account(
        init,
        payer = user,
        space = 8 + VestingStateV6::LEN,
        seeds = [b"vesting_v6", nft_mint.key().as_ref()],
        bump
    )]
    pub vesting_state: Box<Account<'info, VestingStateV6>>,
    #[account(
        seeds = [b"escrow"],
        bump
    )]
    pub escrow: Box<Account<'info, Escrow>>,
    #[account(
        mut,
        seeds = [b"user_tax", user.key().as_ref()],
        bump
    )]
    pub user_tax_state: Box<Account<'info, UserTaxState>>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub token_program_2022: Program<'info, Token2022>,
    /// CHECK: Sysvar for recent blockhashes
    #[account(address = solana_program::sysvar::recent_blockhashes::ID)]
    pub recent_blockhashes: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct RedeemV6<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    /// CHECK: NFT mint - needs to be mutable for burn operation
    #[account(mut)]
    /// CHECK: NFT mint validated in handler via verify_nft_mint_bound_to_collection
    pub nft_mint: AccountInfo<'info>,
    #[account(
        mut,
        constraint = user_nft_ata.mint == nft_mint.key() @ ErrorCode::InvalidNft,
        constraint = user_nft_ata.owner == user.key() @ ErrorCode::NoNft,
        constraint = user_nft_ata.amount == 1 @ ErrorCode::NoNft
    )]
    pub user_nft_ata: Box<InterfaceAccount<'info, TokenAccount2022>>,
    #[account(
        mut,
        token::mint = defai_mint,
        token::authority = user
    )]
    pub user_defai_ata: Box<InterfaceAccount<'info, TokenAccount2022>>,
    #[account(
        mut,
        token::mint = defai_mint,
        token::authority = escrow
    )]
    pub escrow_defai_ata: Box<InterfaceAccount<'info, TokenAccount2022>>,
    /// CHECK: DEFAI mint
    pub defai_mint: AccountInfo<'info>,
    pub config: Box<Account<'info, Config>>,
    #[account(
        seeds = [b"escrow"],
        bump
    )]
    pub escrow: Box<Account<'info, Escrow>>,
    #[account(
        mut,
        seeds = [b"bonus_v6", nft_mint.key().as_ref()],
        bump,
        constraint = bonus_state.mint == nft_mint.key() @ ErrorCode::InvalidNft,
    )]
    pub bonus_state: Box<Account<'info, BonusStateV6>>,
    #[account(
        seeds = [b"vesting_v6", nft_mint.key().as_ref()],
        bump,
        constraint = vesting_state.mint == nft_mint.key() @ ErrorCode::InvalidNft,
    )]
    pub vesting_state: Box<Account<'info, VestingStateV6>>,
    pub system_program: Program<'info, System>,
    pub token_program_2022: Program<'info, Token2022>,
}

#[derive(Accounts)]
pub struct ClaimVestedV6<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    /// CHECK: NFT mint
    /// CHECK: NFT mint validated in handler via verify_nft_mint_bound_to_collection
    pub nft_mint: AccountInfo<'info>,
    #[account(
        constraint = user_nft_ata.mint == nft_mint.key() @ ErrorCode::InvalidNft,
        constraint = user_nft_ata.owner == user.key() @ ErrorCode::NoNft,
        constraint = user_nft_ata.amount == 1 @ ErrorCode::NoNft
    )]
    pub user_nft_ata: InterfaceAccount<'info, TokenAccount2022>,
    #[account(
        mut,
        token::mint = defai_mint,
        token::authority = user
    )]
    pub user_defai_ata: InterfaceAccount<'info, TokenAccount2022>,
    #[account(
        mut,
        token::mint = defai_mint,
        token::authority = escrow
    )]
    pub escrow_defai_ata: InterfaceAccount<'info, TokenAccount2022>,
    /// CHECK: DEFAI mint
    pub defai_mint: AccountInfo<'info>,
    pub config: Account<'info, Config>,
    #[account(
        seeds = [b"escrow"],
        bump
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(
        mut,
        seeds = [b"vesting_v6", nft_mint.key().as_ref()],
        bump,
        constraint = vesting_state.mint == nft_mint.key() @ ErrorCode::InvalidNft,
    )]
    pub vesting_state: Account<'info, VestingStateV6>,
    pub token_program_2022: Program<'info, Token2022>,
}

#[derive(Accounts)]
pub struct AdminWithdraw<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        mut,
        constraint = source_vault.owner == escrow.key() @ ErrorCode::Unauthorized,
    )]
    pub source_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub dest: Account<'info, TokenAccount>,
    pub config: Account<'info, Config>,
    #[account(
        seeds = [b"escrow"],
        bump
    )]
    pub escrow: Account<'info, Escrow>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct AdminWithdrawToken2022<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        mut,
        constraint = source_vault.owner == escrow.key() @ ErrorCode::Unauthorized,
    )]
    pub source_vault: InterfaceAccount<'info, TokenAccount2022>,
    #[account(mut)]
    pub dest: InterfaceAccount<'info, TokenAccount2022>,
    /// CHECK: Mint account for validation
    pub mint: AccountInfo<'info>,
    pub config: Account<'info, Config>,
    #[account(
        seeds = [b"escrow"],
        bump
    )]
    pub escrow: Account<'info, Escrow>,
    pub token_program_2022: Program<'info, Token2022>,
}

#[derive(Accounts)]
pub struct RerollBonusV6<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    /// CHECK: NFT mint
    /// CHECK: NFT mint validated in handler via verify_nft_mint_bound_to_collection
    pub nft_mint: AccountInfo<'info>,
    #[account(
        constraint = user_nft_ata.mint == nft_mint.key() @ ErrorCode::InvalidNft,
        constraint = user_nft_ata.owner == user.key() @ ErrorCode::NoNft,
        constraint = user_nft_ata.amount == 1 @ ErrorCode::NoNft
    )]
    pub user_nft_ata: Box<InterfaceAccount<'info, TokenAccount2022>>,
    #[account(
        token::mint = defai_mint,
        token::authority = user
    )]
    pub user_defai_ata: Box<InterfaceAccount<'info, TokenAccount2022>>,
    #[account(
        mut,
        token::mint = defai_mint,
        token::authority = config.treasury
    )]
    pub treasury_defai_ata: Box<InterfaceAccount<'info, TokenAccount2022>>,
    /// CHECK: DEFAI mint
    pub defai_mint: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [b"bonus_v6", nft_mint.key().as_ref()],
        bump,
        constraint = bonus_state.mint == nft_mint.key() @ ErrorCode::InvalidNft,
    )]
    pub bonus_state: Box<Account<'info, BonusStateV6>>,
    #[account(
        mut,
        seeds = [b"vesting_v6", nft_mint.key().as_ref()],
        bump,
        constraint = vesting_state.mint == nft_mint.key() @ ErrorCode::InvalidNft,
    )]
    pub vesting_state: Box<Account<'info, VestingStateV6>>,
    pub config: Box<Account<'info, Config>>,
    #[account(
        mut,
        seeds = [b"user_tax", user.key().as_ref()],
        bump
    )]
    pub user_tax_state: Box<Account<'info, UserTaxState>>,
    pub token_program_2022: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
    #[account(
        mut,
        seeds = [b"randomness_state"],
        bump = randomness_state.bump
    )]
    pub randomness_state: Box<Account<'info, RandomnessState>>,
    /// CHECK: Sysvar for recent blockhashes
    #[account(address = solana_program::sysvar::recent_blockhashes::ID)]
    pub recent_blockhashes: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct UpdateNftMetadataV6<'info> {
    /// CHECK: NFT mint
    /// CHECK: NFT mint validated in handler via verify_nft_mint_bound_to_collection
    pub nft_mint: AccountInfo<'info>,
    #[account(
        seeds = [b"bonus_v6", nft_mint.key().as_ref()],
        bump
    )]
    pub bonus_state: Account<'info, BonusStateV6>,
    #[account(
        seeds = [b"vesting_v6", nft_mint.key().as_ref()],
        bump
    )]
    pub vesting_state: Account<'info, VestingStateV6>,
}

#[derive(Accounts)]
pub struct ClaimAirdrop<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        init,
        payer = user,
        space = 8 + AirdropVesting::LEN,
        seeds = [b"airdrop_vesting", user.key().as_ref()],
        bump
    )]
    pub airdrop_vesting: Account<'info, AirdropVesting>,
    #[account(
        seeds = [b"collection_config"],
        bump
    )]
    pub collection_config: Account<'info, CollectionConfig>,
    #[account(
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimVestedAirdrop<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        seeds = [b"airdrop_vesting", user.key().as_ref()],
        bump,
        constraint = airdrop_vesting.beneficiary == user.key()
    )]
    pub airdrop_vesting: Box<Account<'info, AirdropVesting>>,
    #[account(
        mut,
        token::mint = defai_mint,
        token::authority = user
    )]
    pub user_defai_ata: Box<InterfaceAccount<'info, TokenAccount2022>>,
    #[account(
        mut,
        // Ensure escrow token account is owned by escrow PDA and is the DEFAI mint
        token::authority = escrow,
        token::mint = defai_mint
    )]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount2022>>,
    /// CHECK: DEFAI mint
    pub defai_mint: AccountInfo<'info>,
    #[account(
        seeds = [b"escrow"],
        bump
    )]
    pub escrow: Box<Account<'info, Escrow>>,
    #[account(
        seeds = [b"collection_config"],
        bump
    )]
    pub collection_config: Box<Account<'info, CollectionConfig>>,
    #[account(
        seeds = [b"config"],
        bump
    )]
    pub config: Box<Account<'info, Config>>,
    pub token_program: Program<'info, Token2022>,
}

// State structs
#[account]
pub struct Config {
    pub admin: Pubkey,
    pub old_mint: Pubkey,
    pub new_mint: Pubkey,
    pub collection: Pubkey,
    pub treasury: Pubkey,
    pub prices: [u64; 5],
    pub paused: bool,
    pub pending_admin: Option<Pubkey>,
    pub admin_change_timestamp: i64,
    pub vrf_enabled: bool,
}

impl Config {
    pub const LEN: usize = 32 + 32 + 32 + 32 + 32 + (8 * 5) + 1 + 33 + 8 + 1;
}

#[account]
pub struct Escrow {
    pub bump: u8,
}

impl Escrow {
    pub const LEN: usize = 1;
}

#[account]
pub struct TaxState {
    pub current_bps: u16,
    pub bump: u8,
    pub last_reset_ts: i64,
}

impl TaxState {
    pub const LEN: usize = 2 + 1 + 8;
}

#[account]
pub struct TimelockProposal {
    pub proposer: Pubkey,
    pub proposal_type: ProposalType,
    pub execute_after: i64,
    pub executed: bool,
    pub cancelled: bool,
}

impl TimelockProposal {
    pub const LEN: usize = 32 + 64 + 8 + 1 + 1; // Adjust based on ProposalType size
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum ProposalType {
    UpdatePrices { prices: [u64; 5] },
    UpdateTreasury { new_treasury: Pubkey },
}

#[account]
pub struct CollectionConfig {
    pub authority: Pubkey,
    pub collection_mint: Pubkey,
    pub treasury: Pubkey,
    pub defai_mint: Pubkey,
    pub old_defai_mint: Pubkey,
    pub tier_names: [String; 5],
    pub tier_symbols: [String; 5],
    pub tier_prices: [u64; 5],
    pub tier_supplies: [u16; 5],
    pub tier_minted: [u16; 5],
    pub tier_uri_prefixes: [String; 5],
    // MAY20DEFAIHolders.csv: OG Tier 0 holders who can mint NFT + get 1:1 vesting from Quantity column
    pub og_tier_0_merkle_root: [u8; 32],
    // 10_1AIR-Sheet1.csv: Airdrop recipients who get vesting only (NO NFT) from AIRDROP column
    pub airdrop_merkle_root: [u8; 32],
    pub og_tier_0_supply: u16, // Reserved supply for OG holders
    pub og_tier_0_minted: u16, // Counter for OG claims
}

impl CollectionConfig {
    pub const LEN: usize = 32
        + 32
        + 32
        + 32
        + 32
        + (64 * 5)
        + (10 * 5)
        + (8 * 5)
        + (2 * 5)
        + (2 * 5)
        + (200 * 5)
        + 32
        + 32
        + 2
        + 2; // Added 4 bytes for og_tier_0_supply and og_tier_0_minted
}

#[account]
pub struct BonusStateV6 {
    pub mint: Pubkey,
    pub tier: u8,
    pub bonus_bps: u16,
    pub vesting_start: i64,
    pub vesting_duration: i64,
    pub claimed: bool,
    pub fee_deducted: u64, // Total fees deducted from rerolls
}

impl BonusStateV6 {
    pub const LEN: usize = 32 + 1 + 2 + 8 + 8 + 1 + 8;
}

#[account]
pub struct VestingStateV6 {
    pub mint: Pubkey,
    pub total_amount: u64,
    pub released_amount: u64,
    pub start_timestamp: i64,
    pub end_timestamp: i64,
    pub last_claimed_timestamp: i64,
}

impl VestingStateV6 {
    pub const LEN: usize = 32 + 8 + 8 + 8 + 8 + 8;
}

#[account]
pub struct UserTaxState {
    pub user: Pubkey,
    pub tax_rate_bps: u16,
    pub last_swap_timestamp: i64,
    pub swap_count: u32,
}

impl UserTaxState {
    pub const LEN: usize = 32 + 2 + 8 + 4;
}

#[account]
pub struct Whitelist {
    pub root: [u8; 32],
    pub claimed_count: u32,
}

impl Whitelist {
    pub const LEN: usize = 32 + 4;
}

#[account]
pub struct OgTier0Claim {
    pub claimer: Pubkey,
    pub claimed: bool,
}

impl OgTier0Claim {
    pub const LEN: usize = 32 + 1;
}

#[account]
pub struct AirdropVesting {
    pub beneficiary: Pubkey,
    pub total_amount: u64,
    pub released_amount: u64,
    pub start_timestamp: i64,
    pub end_timestamp: i64,
    pub last_claimed_timestamp: i64,
}

impl AirdropVesting {
    pub const LEN: usize = 32 + 8 + 8 + 8 + 8 + 8;
}

#[error_code]
pub enum ErrorCode {
    #[msg("Insufficient OLD tokens provided.")]
    InsufficientOldTokens,
    #[msg("Insufficient DEFAI tokens provided.")]
    InsufficientDefaiTokens,
    #[msg("Escrow is out of NFTs.")]
    NoLiquidity,
    #[msg("Invalid NFT collection.")]
    InvalidCollection,
    #[msg("Overflow in maths operation.")]
    MathOverflow,
    #[msg("NFT already redeemed.")]
    NftAlreadyRedeemed,
    #[msg("User does not hold the NFT.")]
    NoNft,
    #[msg("Invalid tier or input provided.")]
    InvalidInput,
    #[msg("Treasury token account does not match config.")]
    InvalidTreasury,
    #[msg("Unauthorized access.")]
    Unauthorized,
    #[msg("Invalid tier provided.")]
    InvalidTier,
    #[msg("Already claimed.")]
    AlreadyClaimed,
    #[msg("Invalid Merkle proof.")]
    InvalidMerkleProof,
    #[msg("Still in cliff period.")]
    StillInCliff,
    #[msg("Nothing to claim.")]
    NothingToClaim,
    #[msg("Tax reset too early.")]
    TaxResetTooEarly,
    #[msg("Already paused.")]
    AlreadyPaused,
    #[msg("Not paused.")]
    NotPaused,
    #[msg("Protocol paused.")]
    ProtocolPaused,
    #[msg("Invalid mint.")]
    InvalidMint,
    #[msg("Insufficient DEFAI balance for reroll. Must hold base tier amount.")]
    InsufficientDefaiForReroll,
    #[msg("No pending admin change")]
    NoPendingAdminChange,
    #[msg("Timelock not expired")]
    TimelockNotExpired,
    #[msg("User not on OG Tier 0 whitelist")]
    NotOnOgWhitelist,
    #[msg("OG Tier 0 NFT already claimed")]
    OgTier0AlreadyClaimed,
    #[msg("Invalid NFT - NFT mint does not match")]
    InvalidNft,
    #[msg("Randomness not ready - generate randomness first")]
    RandomnessNotReady,
}

// ===== Events =====

#[event]
pub struct SwapExecuted {
    pub user: Pubkey,
    pub tier: u8,
    pub price: u64,
    pub tax_amount: u64,
    pub bonus_bps: u16,
    pub nft_mint: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct RedemptionExecuted {
    pub user: Pubkey,
    pub nft_mint: Pubkey,
    pub amount_returned: u64,
    pub fees_deducted: u64,
    pub timestamp: i64,
}

#[event]
pub struct VestingClaimed {
    pub user: Pubkey,
    pub nft_mint: Pubkey,
    pub amount_claimed: u64,
    pub total_vested: u64,
    pub timestamp: i64,
}

#[event]
pub struct BonusRerolled {
    pub user: Pubkey,
    pub nft_mint: Pubkey,
    pub old_bonus_bps: u16,
    pub new_bonus_bps: u16,
    pub tax_paid: u64,
    pub timestamp: i64,
}

#[event]
pub struct AdminAction {
    pub admin: Pubkey,
    pub action: String,
    pub timestamp: i64,
}

#[event]
pub struct TaxReset {
    pub user: Pubkey,
    pub old_rate_bps: u16,
    pub new_rate_bps: u16,
    pub timestamp: i64,
}

#[event]
pub struct AirdropClaimed {
    pub user: Pubkey,
    pub amount: u64,
    pub vesting_start: i64,
    pub vesting_end: i64,
}

#[event]
pub struct AirdropVestingClaimed {
    pub user: Pubkey,
    pub amount_claimed: u64,
    pub total_vested: u64,
    pub timestamp: i64,
}

#[cfg(test)]
mod swap_logic_tests {
    use super::*;
    use anchor_lang::solana_program::program_pack::Pack;
    use solana_program::program_option::COption;
    use spl_token::state::Mint as SplMint;

    fn mint_account_info(key: Pubkey, mint: SplMint) -> AccountInfo<'static> {
        let mut data = vec![0u8; SplMint::LEN];
        mint.pack_into_slice(&mut data);
        let key = Box::leak(Box::new(key));
        let lamports = Box::leak(Box::new(1_000_000u64));
        let data = Box::leak(data.into_boxed_slice());
        let owner = Box::leak(Box::new(anchor_spl::token::ID));
        AccountInfo::new(key, false, false, lamports, data, owner, false, 0)
    }

    fn collection_config(authority: Pubkey, collection_mint: Pubkey) -> CollectionConfig {
        CollectionConfig {
            authority,
            collection_mint,
            treasury: Pubkey::new_unique(),
            defai_mint: Pubkey::new_unique(),
            old_defai_mint: Pubkey::new_unique(),
            tier_names: std::array::from_fn(|_| String::new()),
            tier_symbols: std::array::from_fn(|_| String::new()),
            tier_prices: [0; 5],
            tier_supplies: [0; 5],
            tier_minted: [0; 5],
            tier_uri_prefixes: std::array::from_fn(|_| String::new()),
            og_tier_0_merkle_root: [0; 32],
            airdrop_merkle_root: [0; 32],
            og_tier_0_supply: 0,
            og_tier_0_minted: 0,
        }
    }

    fn valid_nft_mint(authority: Pubkey) -> SplMint {
        SplMint {
            mint_authority: COption::Some(authority),
            supply: 1,
            decimals: 0,
            is_initialized: true,
            freeze_authority: COption::None,
        }
    }

    use spl_token::state::Account as SplTokenAccount;

    fn token_account_info(key: Pubkey, token: SplTokenAccount) -> AccountInfo<'static> {
        let mut data = vec![0u8; SplTokenAccount::LEN];
        token.pack_into_slice(&mut data);
        let key = Box::leak(Box::new(key));
        let lamports = Box::leak(Box::new(1_000_000u64));
        let data = Box::leak(data.into_boxed_slice());
        let owner = Box::leak(Box::new(anchor_spl::token::ID));
        AccountInfo::new(key, false, false, lamports, data, owner, false, 0)
    }

    fn valid_nft_token(mint: Pubkey, owner: Pubkey) -> SplTokenAccount {
        SplTokenAccount {
            mint,
            owner,
            amount: 1,
            delegate: COption::None,
            state: spl_token::state::AccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None,
        }
    }

    #[test]
    fn verify_user_nft_token_account_accepts_valid_holding() {
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let info = token_account_info(Pubkey::new_unique(), valid_nft_token(mint, owner));
        assert!(verify_user_nft_token_account(&info, &mint, &owner).is_ok());
    }

    #[test]
    fn verify_user_nft_token_account_rejects_wrong_owner() {
        let mint = Pubkey::new_unique();
        let info = token_account_info(
            Pubkey::new_unique(),
            valid_nft_token(mint, Pubkey::new_unique()),
        );
        assert!(verify_user_nft_token_account(&info, &mint, &Pubkey::new_unique()).is_err());
    }

    #[test]
    fn verify_user_nft_token_account_rejects_zero_balance() {
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let mut token = valid_nft_token(mint, owner);
        token.amount = 0;
        let info = token_account_info(Pubkey::new_unique(), token);
        assert!(verify_user_nft_token_account(&info, &mint, &owner).is_err());
    }

    #[test]
    fn get_tier_bonus_range_returns_locked_values() {
        assert_eq!(get_tier_bonus_range(0), (0, 0));
        assert_eq!(get_tier_bonus_range(1), (0, 1_500));
        assert_eq!(get_tier_bonus_range(4), (5_000, 30_000));
        assert_eq!(get_tier_bonus_range(99), (0, 0));
    }

    #[test]
    fn verify_nft_mint_accepts_valid_collection_mint() {
        let authority = Pubkey::new_unique();
        let collection_mint = Pubkey::new_unique();
        let nft_key = Pubkey::new_unique();
        let mint = valid_nft_mint(authority);
        let info = mint_account_info(nft_key, mint);
        let config = collection_config(authority, collection_mint);
        assert!(verify_nft_mint_bound_to_collection(&info, &config).is_ok());
    }

    #[test]
    fn verify_nft_mint_rejects_collection_mint_as_nft() {
        let authority = Pubkey::new_unique();
        let collection_mint = Pubkey::new_unique();
        let mint = valid_nft_mint(authority);
        let info = mint_account_info(collection_mint, mint);
        let config = collection_config(authority, collection_mint);
        assert!(verify_nft_mint_bound_to_collection(&info, &config).is_err());
    }

    #[test]
    fn verify_nft_mint_rejects_wrong_mint_authority() {
        let authority = Pubkey::new_unique();
        let collection_mint = Pubkey::new_unique();
        let mint = valid_nft_mint(Pubkey::new_unique());
        let info = mint_account_info(Pubkey::new_unique(), mint);
        let config = collection_config(authority, collection_mint);
        assert!(verify_nft_mint_bound_to_collection(&info, &config).is_err());
    }

    #[test]
    fn verify_nft_mint_rejects_fungible_supply() {
        let authority = Pubkey::new_unique();
        let mint = SplMint {
            mint_authority: COption::Some(authority),
            supply: 2,
            decimals: 0,
            is_initialized: true,
            freeze_authority: COption::None,
        };
        let info = mint_account_info(Pubkey::new_unique(), mint);
        let config = collection_config(authority, Pubkey::new_unique());
        assert!(verify_nft_mint_bound_to_collection(&info, &config).is_err());
    }

    #[test]
    fn verify_nft_mint_rejects_non_zero_decimals() {
        let authority = Pubkey::new_unique();
        let mint = SplMint {
            mint_authority: COption::Some(authority),
            supply: 1,
            decimals: 1,
            is_initialized: true,
            freeze_authority: COption::None,
        };
        let info = mint_account_info(Pubkey::new_unique(), mint);
        let config = collection_config(authority, Pubkey::new_unique());
        assert!(verify_nft_mint_bound_to_collection(&info, &config).is_err());
    }

    #[test]
    fn verify_nft_mint_rejects_revoked_mint_authority() {
        let authority = Pubkey::new_unique();
        let mint = SplMint {
            mint_authority: COption::None,
            supply: 1,
            decimals: 0,
            is_initialized: true,
            freeze_authority: COption::None,
        };
        let info = mint_account_info(Pubkey::new_unique(), mint);
        let config = collection_config(authority, Pubkey::new_unique());
        assert!(verify_nft_mint_bound_to_collection(&info, &config).is_err());
    }
}
