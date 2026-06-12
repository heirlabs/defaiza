use anchor_lang::prelude::*;

// Risk Management Settings for Trading
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub struct RiskManagementSettings {
    // Risk Limits
    pub max_drawdown_bps: u16,      // Basis points (2000 = 20%)
    pub max_daily_loss_bps: u16,    // Basis points (500 = 5%)
    pub max_position_size_bps: u16, // Basis points (3000 = 30%)
    pub min_liquidity_buffer: u64,  // In lamports (0.130 SOL = 130_000_000)

    // Strategy Allocation (must sum to 10000 = 100%)
    pub strategy_mix: StrategyMix,

    // Position Management
    pub max_open_positions: u8,      // Maximum concurrent positions
    pub position_timeout_hours: u32, // Auto-close after X hours
    pub use_stop_loss: bool,
    pub stop_loss_bps: u16, // Basis points for stop loss
    pub use_take_profit: bool,
    pub take_profit_bps: u16, // Basis points for take profit

    // Trading Hours (optional)
    pub trading_enabled_hours: Option<TradingHours>,

    // Risk Metrics Tracking
    pub current_drawdown_bps: u16,
    pub daily_loss_bps: u16,
    pub last_risk_reset: i64,
}

impl RiskManagementSettings {
    pub const LEN: usize = 2 + // max_drawdown_bps
        2 + // max_daily_loss_bps
        2 + // max_position_size_bps
        8 + // min_liquidity_buffer
        StrategyMix::LEN + // strategy_mix
        1 + // max_open_positions
        4 + // position_timeout_hours
        1 + // use_stop_loss
        2 + // stop_loss_bps
        1 + // use_take_profit
        2 + // take_profit_bps
        (1 + TradingHours::LEN) + // trading_enabled_hours Option
        2 + // current_drawdown_bps
        2 + // daily_loss_bps
        8; // last_risk_reset

    // Default conservative settings
    pub fn default_conservative() -> Self {
        Self {
            max_drawdown_bps: 2000,            // 20%
            max_daily_loss_bps: 500,           // 5%
            max_position_size_bps: 3000,       // 30%
            min_liquidity_buffer: 130_000_000, // 0.130 SOL
            strategy_mix: StrategyMix::conservative(),
            max_open_positions: 3,
            position_timeout_hours: 24,
            use_stop_loss: true,
            stop_loss_bps: 500, // 5%
            use_take_profit: true,
            take_profit_bps: 1500, // 15%
            trading_enabled_hours: None,
            current_drawdown_bps: 0,
            daily_loss_bps: 0,
            last_risk_reset: 0,
        }
    }

    pub fn default_balanced() -> Self {
        Self {
            max_drawdown_bps: 3000,            // 30%
            max_daily_loss_bps: 1000,          // 10%
            max_position_size_bps: 4000,       // 40%
            min_liquidity_buffer: 100_000_000, // 0.100 SOL
            strategy_mix: StrategyMix::balanced(),
            max_open_positions: 5,
            position_timeout_hours: 48,
            use_stop_loss: true,
            stop_loss_bps: 1000, // 10%
            use_take_profit: true,
            take_profit_bps: 3000, // 30%
            trading_enabled_hours: None,
            current_drawdown_bps: 0,
            daily_loss_bps: 0,
            last_risk_reset: 0,
        }
    }

    pub fn default_aggressive() -> Self {
        Self {
            max_drawdown_bps: 5000,           // 50%
            max_daily_loss_bps: 2000,         // 20%
            max_position_size_bps: 6000,      // 60%
            min_liquidity_buffer: 50_000_000, // 0.050 SOL
            strategy_mix: StrategyMix::aggressive(),
            max_open_positions: 10,
            position_timeout_hours: 72,
            use_stop_loss: true,
            stop_loss_bps: 2000, // 20%
            use_take_profit: false,
            take_profit_bps: 0,
            trading_enabled_hours: None,
            current_drawdown_bps: 0,
            daily_loss_bps: 0,
            last_risk_reset: 0,
        }
    }

    pub fn validate(&self) -> Result<()> {
        // Validate basis points don't exceed 100%
        require!(
            self.max_drawdown_bps <= 10000,
            crate::EstateError::InvalidRiskParameter
        );
        require!(
            self.max_daily_loss_bps <= 10000,
            crate::EstateError::InvalidRiskParameter
        );
        require!(
            self.max_position_size_bps <= 10000,
            crate::EstateError::InvalidRiskParameter
        );

        // Validate strategy mix
        self.strategy_mix.validate()?;

        // Validate position limits
        require!(
            self.max_open_positions > 0 && self.max_open_positions <= 20,
            crate::EstateError::InvalidRiskParameter
        );

        Ok(())
    }

    pub fn check_risk_limits(&self, current_value: u64, initial_value: u64) -> Result<()> {
        // Check drawdown
        if current_value < initial_value {
            let loss_bps = ((initial_value - current_value) * 10000 / initial_value) as u16;
            require!(
                loss_bps <= self.max_drawdown_bps,
                crate::EstateError::MaxDrawdownExceeded
            );
        }

        // Check daily loss (would need to track daily P&L)
        require!(
            self.daily_loss_bps <= self.max_daily_loss_bps,
            crate::EstateError::MaxDailyLossExceeded
        );

        Ok(())
    }

    pub fn reset_daily_metrics(&mut self, clock: &Clock) {
        self.daily_loss_bps = 0;
        self.last_risk_reset = clock.unix_timestamp;
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub struct StrategyMix {
    // Conservative Strategy Allocations (bps)
    pub cash_and_carry_bps: u16,      // Arbitrage strategy
    pub covered_options_bps: u16,     // Options selling
    pub liquidity_provision_bps: u16, // LP strategies

    // Balanced Strategy Allocations (bps)
    pub options_trading_bps: u16,     // Active options
    pub lp_with_rebalancing_bps: u16, // Dynamic LP
    pub low_leverage_perps_bps: u16,  // Perpetuals with low leverage

    // Aggressive Strategy Allocations (bps)
    pub high_leverage_perps_bps: u16, // High leverage perpetuals
    pub naked_options_bps: u16,       // Uncovered options
    pub volatile_lp_pairs_bps: u16,   // High volatility LP
    pub arbitrage_bps: u16,           // MEV and arbitrage
}

impl StrategyMix {
    pub const LEN: usize = 10 * 2; // 10 strategies * 2 bytes each

    pub fn conservative() -> Self {
        Self {
            cash_and_carry_bps: 4000,      // 40%
            covered_options_bps: 3000,     // 30%
            liquidity_provision_bps: 3000, // 30%
            options_trading_bps: 0,
            lp_with_rebalancing_bps: 0,
            low_leverage_perps_bps: 0,
            high_leverage_perps_bps: 0,
            naked_options_bps: 0,
            volatile_lp_pairs_bps: 0,
            arbitrage_bps: 0,
        }
    }

    pub fn balanced() -> Self {
        Self {
            cash_and_carry_bps: 2500, // 25%
            covered_options_bps: 0,
            liquidity_provision_bps: 0,
            options_trading_bps: 2500,     // 25%
            lp_with_rebalancing_bps: 2500, // 25%
            low_leverage_perps_bps: 2500,  // 25%
            high_leverage_perps_bps: 0,
            naked_options_bps: 0,
            volatile_lp_pairs_bps: 0,
            arbitrage_bps: 0,
        }
    }

    pub fn aggressive() -> Self {
        Self {
            cash_and_carry_bps: 0,
            covered_options_bps: 0,
            liquidity_provision_bps: 0,
            options_trading_bps: 0,
            lp_with_rebalancing_bps: 0,
            low_leverage_perps_bps: 0,
            high_leverage_perps_bps: 4000, // 40%
            naked_options_bps: 3000,       // 30%
            volatile_lp_pairs_bps: 2000,   // 20%
            arbitrage_bps: 1000,           // 10%
        }
    }

    pub fn validate(&self) -> Result<()> {
        let total = self.cash_and_carry_bps
            + self.covered_options_bps
            + self.liquidity_provision_bps
            + self.options_trading_bps
            + self.lp_with_rebalancing_bps
            + self.low_leverage_perps_bps
            + self.high_leverage_perps_bps
            + self.naked_options_bps
            + self.volatile_lp_pairs_bps
            + self.arbitrage_bps;

        require!(
            total == 10000, // Must sum to 100%
            crate::EstateError::InvalidStrategyMix
        );

        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub struct TradingHours {
    pub start_hour_utc: u8, // 0-23
    pub end_hour_utc: u8,   // 0-23
    pub active_days: u8,    // Bitfield: 0b0111111 = Mon-Sun
}

impl TradingHours {
    pub const LEN: usize = 1 + 1 + 1;

    pub fn is_active(&self, _clock: &Clock) -> bool {
        // Implementation would check current time against trading hours
        // For now, return true (always active)
        true
    }
}

// Update contexts for risk management
#[derive(Accounts)]
pub struct UpdateRiskSettings<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        has_one = owner,
        constraint = estate.trading_enabled @ crate::EstateError::TradingNotEnabled,
    )]
    pub estate: Account<'info, crate::Estate>,
}

#[derive(Accounts)]
pub struct UpdateStrategyMix<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        has_one = owner,
        constraint = estate.trading_enabled @ crate::EstateError::TradingNotEnabled,
    )]
    pub estate: Account<'info, crate::Estate>,
}

// Events
#[event]
pub struct RiskSettingsUpdated {
    pub estate: Pubkey,
    pub max_drawdown_bps: u16,
    pub max_daily_loss_bps: u16,
    pub max_position_size_bps: u16,
    pub timestamp: i64,
}

#[event]
pub struct StrategyMixUpdated {
    pub estate: Pubkey,
    pub strategy_mix: StrategyMix,
    pub timestamp: i64,
}

#[event]
pub struct RiskLimitTriggered {
    pub estate: Pubkey,
    pub limit_type: RiskLimitType,
    pub current_value_bps: u16,
    pub limit_value_bps: u16,
    pub timestamp: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub enum RiskLimitType {
    MaxDrawdown,
    MaxDailyLoss,
    MaxPositionSize,
}

// Implementation functions
pub fn update_risk_settings(
    ctx: Context<UpdateRiskSettings>,
    settings: RiskManagementSettings,
) -> Result<()> {
    // Validate the new settings
    settings.validate()?;

    let estate = &mut ctx.accounts.estate;
    crate::require_direct_owner_mutation(estate, &ctx.accounts.owner.key())?;
    estate.risk_settings = Some(settings.clone());

    emit!(RiskSettingsUpdated {
        estate: estate.key(),
        max_drawdown_bps: settings.max_drawdown_bps,
        max_daily_loss_bps: settings.max_daily_loss_bps,
        max_position_size_bps: settings.max_position_size_bps,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("Risk settings updated for estate {}", estate.estate_number);

    Ok(())
}

pub fn update_strategy_mix(
    ctx: Context<UpdateStrategyMix>,
    strategy_mix: StrategyMix,
) -> Result<()> {
    // Validate the strategy mix
    strategy_mix.validate()?;

    let estate = &mut ctx.accounts.estate;
    crate::require_direct_owner_mutation(estate, &ctx.accounts.owner.key())?;

    // Get or create risk settings
    let mut risk_settings =
        estate
            .risk_settings
            .clone()
            .unwrap_or_else(|| match estate.trading_strategy {
                Some(crate::TradingStrategy::Conservative) => {
                    RiskManagementSettings::default_conservative()
                }
                Some(crate::TradingStrategy::Balanced) => {
                    RiskManagementSettings::default_balanced()
                }
                Some(crate::TradingStrategy::Aggressive) => {
                    RiskManagementSettings::default_aggressive()
                }
                None => RiskManagementSettings::default_balanced(),
            });

    risk_settings.strategy_mix = strategy_mix.clone();
    estate.risk_settings = Some(risk_settings);

    emit!(StrategyMixUpdated {
        estate: estate.key(),
        strategy_mix,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("Strategy mix updated for estate {}", estate.estate_number);

    Ok(())
}

#[cfg(test)]
mod risk_tests {
    use super::*;

    #[test]
    fn default_strategy_mixes_sum_to_one_hundred_percent() {
        for mix in [
            StrategyMix::conservative(),
            StrategyMix::balanced(),
            StrategyMix::aggressive(),
        ] {
            mix.validate().unwrap();
        }
    }

    #[test]
    fn default_risk_profiles_validate() {
        for settings in [
            RiskManagementSettings::default_conservative(),
            RiskManagementSettings::default_balanced(),
            RiskManagementSettings::default_aggressive(),
        ] {
            settings.validate().unwrap();
        }
    }

    #[test]
    fn strategy_mix_rejects_invalid_total() {
        let mut mix = StrategyMix::conservative();
        mix.cash_and_carry_bps = 5_000;
        assert!(mix.validate().is_err());
    }

    #[test]
    fn risk_settings_reject_excessive_drawdown_bps() {
        let mut settings = RiskManagementSettings::default_conservative();
        settings.max_drawdown_bps = 10_001;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn check_risk_limits_rejects_drawdown_above_cap() {
        let settings = RiskManagementSettings::default_conservative();
        assert!(settings.check_risk_limits(799, 1_000).is_err());
    }

    #[test]
    fn check_risk_limits_allows_drawdown_at_cap() {
        let settings = RiskManagementSettings::default_conservative();
        assert!(settings.check_risk_limits(800, 1_000).is_ok());
    }

    #[test]
    fn check_risk_limits_rejects_daily_loss_above_cap() {
        let mut settings = RiskManagementSettings::default_conservative();
        settings.daily_loss_bps = settings.max_daily_loss_bps.saturating_add(1);
        assert!(settings.check_risk_limits(1_000, 1_000).is_err());
    }

    #[test]
    fn reset_daily_metrics_clears_daily_loss() {
        let mut settings = RiskManagementSettings::default_conservative();
        settings.daily_loss_bps = 400;
        let clock = Clock {
            slot: 1,
            epoch: 1,
            unix_timestamp: 123,
            epoch_start_timestamp: 0,
            leader_schedule_epoch: 0,
        };
        settings.reset_daily_metrics(&clock);
        assert_eq!(settings.daily_loss_bps, 0);
        assert_eq!(settings.last_risk_reset, 123);
    }
}
