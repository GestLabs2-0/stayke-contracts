use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, CloseAccount, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use stayke_config::{GlobalConfig, CPI_AUTHORITY_SEED, GLOBAL_CONFIG_SEED};
use stayke_core::{
    constants::{REPUTATION_PROFILE_SEED, USER_PROFILE_SEED},
    program::StaykeCore,
    ReputationProfile, UserProfile,
};
use stayke_treasury::{program::StaykeTreasury, TreasuryConfig, TREASURY_CONFIG_SEED};

use crate::{
    constants::{
        BOOKING_DAYS_SEED, BOOKING_SEED, CANCELLATION_WINDOW_HOURS, ESCROW_PDA_SEED,
        HOST_CANCELLATION_PENALTY_PERCENTAGE,
    },
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingDays, BookingStatus},
    utils::{derive_date, release_days_cross_years, release_days_single_year, TimestampExt},
};

/// Number of seconds in an hour.
const SECONDS_PER_HOUR: i64 = 3_600;

// ---------------------------------------------------------------------------
// Host cancel booking — cancellation initiated by the booking's host.
//
// Full refund to the guest. Inside the cancellation window (less than
// `CANCELLATION_WINDOW_HOURS` before check-in) the host's available deposit is
// also slashed and paid to the guest (capped at that deposit); outside the
// window there is no slash. The slash is settled through a stayke-treasury
// penalize CPI and the host's tracked deposit is decremented through
// stayke-core.
//
// The escrow token account is closed, the reserved days are released, and the
// host's `host_cancellations` counter is incremented via CPI.
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct HostCancelBooking<'info> {
    /// The host of the booking. Validated against the host profile and also
    /// receives the escrow account rent on closure.
    #[account(
        mut,
        constraint = caller.key() == host_profile.authority @ EscrowError::UnauthorizedHost
    )]
    pub caller: Signer<'info>,

    /// The guest's UserProfile — anchors the refund destination to the guest's
    /// identity stored in the booking.
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), guest_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = guest_profile.bump,
        constraint = guest_profile.key() == booking.guest @ EscrowError::WrongGuestPassed,
    )]
    pub guest_profile: Box<Account<'info, UserProfile>>,

    /// The host's UserProfile — mutable for the deposit slash.
    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = host_profile.key() == booking.host @ EscrowError::InvalidHostBooking,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    /// The host's ReputationProfile — mutable for `host_cancellations`.
    #[account(
        mut,
        seeds = [REPUTATION_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_reputation.bump,
    )]
    pub host_reputation: Box<Account<'info, ReputationProfile>>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), guest_profile.key().as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.status == BookingStatus::Pending
            || booking.status == BookingStatus::HostAccepted @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Box<Account<'info, Booking>>,

    /// The listing the booking belongs to — validated against `booking.property`.
    // #[account(
    //     constraint = property.key() == booking.property @ EscrowError::InvalidBookingProperty,
    // )]
    // pub property: Box<Account<'info, Listing>>,

    /// The property's availability bitmap for the check-in year — the reserved
    /// days are released on cancellation.
    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking.property.as_ref(), booking.check_in.year().to_le_bytes().as_ref()],
        bump = booking_days.bump,
    )]
    pub booking_days: Box<Account<'info, BookingDays>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [ESCROW_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump = booking.escrow_bump,
        token::mint = mint,
        token::authority = booking,
        token::token_program = token_program,
    )]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The guest's USDC token account — destination for the refund and the
    /// host's deposit slash, bound to the guest's authority.
    #[account(
        mut,
        token::mint = mint,
        token::authority = guest_profile.authority,
        token::token_program = token_program,
    )]
    pub guest_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = mint.key() == global_config.usdc_mint @ EscrowError::InvalidTokenMint,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Escrow CPI authority PDA — signs privileged core/treasury mutators.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [TREASURY_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_treasury::ID,
        bump = treasury_config.bump,
    )]
    pub treasury_config: Box<Account<'info, TreasuryConfig>>,

    /// Treasury vault holding user guarantee deposits.
    #[account(
        mut,
        constraint = treasury_vault.key() == treasury_config.treasury_vault @ EscrowError::InvalidVaultAccount,
    )]
    pub treasury_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Treasury PDA — signs the CPI transfer out of the vault. The PDA
    /// is re-validated inside stayke-treasury's penalize CPI.
    pub treasury_pda: UncheckedAccount<'info>,

    pub stayke_core_program: Program<'info, StaykeCore>,
    pub stayke_treasury_program: Program<'info, StaykeTreasury>,
    pub token_program: Interface<'info, TokenInterface>,
}

/// Cross-year variant: the booking spans two calendar years, so two
/// `BookingDays` accounts are released.
#[derive(Accounts)]
pub struct HostCancelBookingCrossYear<'info> {
    #[account(
        mut,
        constraint = caller.key() == host_profile.authority @ EscrowError::UnauthorizedHost
    )]
    pub caller: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), guest_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = guest_profile.bump,
        constraint = guest_profile.key() == booking.guest @ EscrowError::WrongGuestPassed,
    )]
    pub guest_profile: Box<Account<'info, UserProfile>>,

    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = host_profile.key() == booking.host @ EscrowError::InvalidHostBooking,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    #[account(
        mut,
        seeds = [REPUTATION_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_reputation.bump,
    )]
    pub host_reputation: Box<Account<'info, ReputationProfile>>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), guest_profile.key().as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.status == BookingStatus::Pending
            || booking.status == BookingStatus::HostAccepted @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Box<Account<'info, Booking>>,

    // #[account(
    //     constraint = property.key() == booking.property @ EscrowError::InvalidBookingProperty,
    // )]
    // pub property: Box<Account<'info, Listing>>,
    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking.property.as_ref(), booking.check_in.year().to_le_bytes().as_ref()],
        bump = booking_days.bump,
    )]
    pub booking_days: Box<Account<'info, BookingDays>>,

    /// Availability bitmap for the following year — released on cancellation.
    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking.property.as_ref(), (booking.check_in.year() + 1).to_le_bytes().as_ref()],
        bump = booking_days_next.bump,
    )]
    pub booking_days_next: Box<Account<'info, BookingDays>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [ESCROW_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump = booking.escrow_bump,
        token::mint = mint,
        token::authority = booking,
        token::token_program = token_program,
    )]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = guest_profile.authority,
        token::token_program = token_program,
    )]
    pub guest_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = mint.key() == global_config.usdc_mint @ EscrowError::InvalidTokenMint,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Escrow CPI authority PDA — signs privileged core/treasury mutators.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [TREASURY_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_treasury::ID,
        bump = treasury_config.bump,
    )]
    pub treasury_config: Box<Account<'info, TreasuryConfig>>,

    #[account(
        mut,
        constraint = treasury_vault.key() == treasury_config.treasury_vault @ EscrowError::InvalidVaultAccount,
    )]
    pub treasury_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Treasury PDA — signs the CPI transfer out of the vault. The PDA
    /// is re-validated inside stayke-treasury's penalize CPI.
    pub treasury_pda: UncheckedAccount<'info>,

    pub stayke_core_program: Program<'info, StaykeCore>,
    pub stayke_treasury_program: Program<'info, StaykeTreasury>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_host_cancel_booking(ctx: Context<HostCancelBooking>) -> Result<()> {
    validate_host_cancellation_config(
        CANCELLATION_WINDOW_HOURS,
        HOST_CANCELLATION_PENALTY_PERCENTAGE,
    )?;

    let now = Clock::get()?.unix_timestamp;
    let booking = &mut ctx.accounts.booking;

    require!(now < booking.check_in, EscrowError::CheckInPassed);

    require!(
        ctx.accounts.escrow_token_account.amount >= booking.total_price,
        EscrowError::InsufficientFunds
    );

    let decimals = ctx.accounts.mint.decimals;
    let total_price = booking.total_price;

    let start_date = derive_date(booking.check_in);
    let end_date = derive_date(booking.check_out);

    let booking_seeds: &[&[&[u8]]] = &[&[
        BOOKING_SEED.as_bytes(),
        booking.property.as_ref(),
        booking.guest.as_ref(),
        &booking.check_in.to_le_bytes(),
        &[booking.bump],
    ]];

    // Full refund to the guest.
    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.escrow_token_account.to_account_info(),
                to: ctx.accounts.guest_token_account.to_account_info(),
                authority: booking.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
            booking_seeds,
        ),
        total_price,
        decimals,
    )?;

    let cpi_bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[cpi_bump]]];

    let window_seconds = (CANCELLATION_WINDOW_HOURS as i64) * SECONDS_PER_HOUR;
    let time_until_check_in = booking.check_in - now;

    // The deposit slash only applies inside the cancellation window. Outside
    // it the host keeps their full deposit. The slash is capped at the
    // available deposit.
    let slash = if time_until_check_in > window_seconds {
        0
    } else {
        compute_host_slash(
            ctx.accounts.host_profile.deposited,
            HOST_CANCELLATION_PENALTY_PERCENTAGE,
        )?
    };

    if slash > 0 {
        let penalize_accounts = stayke_treasury::cpi::accounts::PenalizeTransferCpi {
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
            config: ctx.accounts.treasury_config.to_account_info(),
            global_config: ctx.accounts.global_config.to_account_info(),
            treasury_vault: ctx.accounts.treasury_vault.to_account_info(),
            treasury_pda: ctx.accounts.treasury_pda.to_account_info(),
            destination_token_account: ctx.accounts.guest_token_account.to_account_info(),
            usdc_mint: ctx.accounts.mint.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };
        stayke_treasury::cpi::cpi_penalize_transfer(
            CpiContext::new_with_signer(
                ctx.accounts.stayke_treasury_program.key(),
                penalize_accounts,
                signer_seeds,
            ),
            slash,
        )?;

        let update_deposit_accounts = stayke_core::cpi::accounts::UpdateUserProfile {
            user_profile: ctx.accounts.host_profile.to_account_info(),
            global_config: ctx.accounts.global_config.to_account_info(),
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
        };
        stayke_core::cpi::update_deposit(
            CpiContext::new_with_signer(
                ctx.accounts.stayke_core_program.key(),
                update_deposit_accounts,
                signer_seeds,
            ),
            slash,
            false,
        )?;
    }

    let host_reputation_accounts = stayke_core::cpi::accounts::UpdateReputationProfile {
        reputation_profile: ctx.accounts.host_reputation.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    stayke_core::cpi::increment_host_cancellations(CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        host_reputation_accounts,
        signer_seeds,
    ))?;

    // Close the escrow token account, returning rent to the caller.
    token_interface::close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        CloseAccount {
            account: ctx.accounts.escrow_token_account.to_account_info(),
            destination: ctx.accounts.caller.to_account_info(),
            authority: booking.to_account_info(),
        },
        booking_seeds,
    ))?;

    // Release the reserved days.
    release_days_single_year(&mut ctx.accounts.booking_days, &start_date, &end_date)?;
    booking.status = BookingStatus::Cancelled;

    emit!(BookingStatusUpdated {
        status: BookingStatus::Cancelled,
        booking: booking.key()
    });

    Ok(())
}

pub fn handler_host_cancel_booking_cross_year(
    ctx: Context<HostCancelBookingCrossYear>,
) -> Result<()> {
    validate_host_cancellation_config(
        CANCELLATION_WINDOW_HOURS,
        HOST_CANCELLATION_PENALTY_PERCENTAGE,
    )?;

    let now = Clock::get()?.unix_timestamp;
    let booking = &mut ctx.accounts.booking;

    require!(now < booking.check_in, EscrowError::CheckInPassed);

    require!(
        ctx.accounts.escrow_token_account.amount >= booking.total_price,
        EscrowError::InsufficientFunds
    );

    let decimals = ctx.accounts.mint.decimals;
    let total_price = booking.total_price;

    let start_date = derive_date(booking.check_in);
    let end_date = derive_date(booking.check_out);

    let booking_seeds: &[&[&[u8]]] = &[&[
        BOOKING_SEED.as_bytes(),
        booking.property.as_ref(),
        booking.guest.as_ref(),
        &booking.check_in.to_le_bytes(),
        &[booking.bump],
    ]];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.escrow_token_account.to_account_info(),
                to: ctx.accounts.guest_token_account.to_account_info(),
                authority: booking.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
            booking_seeds,
        ),
        total_price,
        decimals,
    )?;

    let cpi_bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[cpi_bump]]];

    let window_seconds = (CANCELLATION_WINDOW_HOURS as i64) * SECONDS_PER_HOUR;
    let time_until_check_in = booking.check_in - now;

    // The deposit slash only applies inside the cancellation window. Outside
    // it the host keeps their full deposit. The slash is capped at the
    // available deposit.
    let slash = if time_until_check_in > window_seconds {
        0
    } else {
        compute_host_slash(
            ctx.accounts.host_profile.deposited,
            HOST_CANCELLATION_PENALTY_PERCENTAGE,
        )?
    };

    if slash > 0 {
        let penalize_accounts = stayke_treasury::cpi::accounts::PenalizeTransferCpi {
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
            config: ctx.accounts.treasury_config.to_account_info(),
            global_config: ctx.accounts.global_config.to_account_info(),
            treasury_vault: ctx.accounts.treasury_vault.to_account_info(),
            treasury_pda: ctx.accounts.treasury_pda.to_account_info(),
            destination_token_account: ctx.accounts.guest_token_account.to_account_info(),
            usdc_mint: ctx.accounts.mint.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };
        stayke_treasury::cpi::cpi_penalize_transfer(
            CpiContext::new_with_signer(
                ctx.accounts.stayke_treasury_program.key(),
                penalize_accounts,
                signer_seeds,
            ),
            slash,
        )?;

        let update_deposit_accounts = stayke_core::cpi::accounts::UpdateUserProfile {
            user_profile: ctx.accounts.host_profile.to_account_info(),
            global_config: ctx.accounts.global_config.to_account_info(),
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
        };
        stayke_core::cpi::update_deposit(
            CpiContext::new_with_signer(
                ctx.accounts.stayke_core_program.key(),
                update_deposit_accounts,
                signer_seeds,
            ),
            slash,
            false,
        )?;
    }

    let host_reputation_accounts = stayke_core::cpi::accounts::UpdateReputationProfile {
        reputation_profile: ctx.accounts.host_reputation.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    stayke_core::cpi::increment_host_cancellations(CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        host_reputation_accounts,
        signer_seeds,
    ))?;

    token_interface::close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        CloseAccount {
            account: ctx.accounts.escrow_token_account.to_account_info(),
            destination: ctx.accounts.caller.to_account_info(),
            authority: booking.to_account_info(),
        },
        booking_seeds,
    ))?;

    // Release the reserved days across both calendar years.
    release_days_cross_years(
        &mut ctx.accounts.booking_days,
        &mut ctx.accounts.booking_days_next,
        &start_date,
        &end_date,
    )?;
    booking.status = BookingStatus::Cancelled;

    emit!(BookingStatusUpdated {
        status: BookingStatus::Cancelled,
        booking: booking.key()
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested)
// ---------------------------------------------------------------------------

/// Validates the hardcoded host-cancellation policy values so the logic is
/// safe to source from configuration in a future iteration.
fn validate_host_cancellation_config(window_hours: u32, penalty_percentage: u8) -> Result<()> {
    require!(window_hours > 0, EscrowError::InvalidCancellationWindow);
    require!(
        penalty_percentage <= 100,
        EscrowError::InvalidCancellationPercentage
    );
    Ok(())
}

/// Slash applied to a host's deposit on host cancellation. Capped at the
/// available deposit so the effective slash never exceeds it.
fn compute_host_slash(host_deposit: u64, penalty_percentage: u8) -> Result<u64> {
    let calculated = (host_deposit as u128)
        .checked_mul(penalty_percentage as u128)
        .ok_or(EscrowError::PriceOverflow)?
        .checked_div(100)
        .ok_or(EscrowError::PriceOverflow)? as u64;

    Ok(calculated.min(host_deposit))
}

// ---------------------------------------------------------------------------
// Unit tests — pure functions
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{CANCELLATION_WINDOW_HOURS, HOST_CANCELLATION_PENALTY_PERCENTAGE};

    #[test]
    fn hardcoded_config_is_valid() {
        validate_host_cancellation_config(
            CANCELLATION_WINDOW_HOURS,
            HOST_CANCELLATION_PENALTY_PERCENTAGE,
        )
        .unwrap();
    }

    #[test]
    fn invalid_window_rejected() {
        assert_eq!(
            validate_host_cancellation_config(0, 10).unwrap_err(),
            EscrowError::InvalidCancellationWindow.into()
        );
    }

    #[test]
    fn invalid_penalty_percentage_rejected() {
        assert_eq!(
            validate_host_cancellation_config(72, 101).unwrap_err(),
            EscrowError::InvalidCancellationPercentage.into()
        );
    }

    #[test]
    fn slash_mvp_config() {
        assert_eq!(compute_host_slash(1_000_000, 10).unwrap(), 100_000);
    }

    #[test]
    fn slash_capped_at_deposit() {
        // 5 * 10% = 0.5 -> 0, so no slash below one full unit.
        assert_eq!(compute_host_slash(5, 10).unwrap(), 0);
        // Deposit smaller than the calculated slash is fully taken (capped).
        assert_eq!(compute_host_slash(9, 100).unwrap(), 9);
    }

    #[test]
    fn slash_max_u64_does_not_overflow() {
        assert_eq!(compute_host_slash(u64::MAX, 10).unwrap(), u64::MAX / 10);
    }
}
