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

use crate::{
    constants::{
        BOOKING_DAYS_SEED, BOOKING_SEED, CANCELLATION_HOST_SHARE_PERCENTAGE,
        CANCELLATION_REFUND_PERCENTAGE, CANCELLATION_WINDOW_HOURS, ESCROW_PDA_SEED,
    },
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingDays, BookingStatus},
    utils::{derive_date, release_days_cross_years, release_days_single_year, TimestampExt},
};

/// Number of seconds in an hour.
const SECONDS_PER_HOUR: i64 = 3_600;

// ---------------------------------------------------------------------------
// Guest cancel booking — cancellation initiated by the booking's guest.
//
// The cancellation policy depends on how much time remains before check-in:
//
// * Outside the window (more than `CANCELLATION_WINDOW_HOURS` before check-in):
//   full refund, no host payout, no platform fee.
// * Inside the window: refund split (guest / host / Stayke) with the Stayke
//   share derived as the remainder so no funds are lost to rounding.
//
// The escrow token account is closed, the reserved days are released, and the
// guest's `client_cancellations` counter is incremented via CPI.
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct GuestCancelBooking<'info> {
    /// The guest of the booking. Validated against the guest profile and also
    /// receives the escrow account rent on closure.
    #[account(
        mut,
        constraint = caller.key() == guest_profile.authority @ EscrowError::UnauthorizedCancellation
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

    /// The host's UserProfile — read-only, only anchors the host payout account
    /// (`host_token_account`) to the booking's host.
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = host_profile.key() == booking.host @ EscrowError::InvalidHostBooking,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    /// The guest's ReputationProfile — mutable for `client_cancellations`.
    #[account(
        mut,
        seeds = [REPUTATION_PROFILE_SEED.as_bytes(), guest_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = guest_reputation.bump,
    )]
    pub guest_reputation: Box<Account<'info, ReputationProfile>>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
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

    /// The guest's USDC token account — destination for the refund, bound to
    /// the guest's authority so the caller cannot redirect the funds.
    #[account(
        mut,
        token::mint = mint,
        token::authority = guest_profile.authority,
        token::token_program = token_program,
    )]
    pub guest_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The host's USDC token account — destination for the host's share when a
    /// guest cancels inside the window.
    #[account(
        mut,
        token::mint = mint,
        token::authority = host_profile.authority,
        token::token_program = token_program,
    )]
    pub host_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Platform fee vault.
    #[account(
        mut,
        constraint = platform_vault.key() == global_config.platform_vault @ EscrowError::InvalidVaultAccount,
    )]
    pub platform_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = mint.key() == global_config.usdc_mint @ EscrowError::InvalidTokenMint,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Escrow CPI authority PDA — signs the reputation CPI into
    /// stayke-core.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub stayke_core_program: Program<'info, StaykeCore>,
    pub token_program: Interface<'info, TokenInterface>,
}

/// Cross-year variant: the booking spans two calendar years, so two
/// `BookingDays` accounts are released.
#[derive(Accounts)]
pub struct GuestCancelBookingCrossYear<'info> {
    #[account(
        mut,
        constraint = caller.key() == guest_profile.authority @ EscrowError::UnauthorizedCancellation
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
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = host_profile.key() == booking.host @ EscrowError::InvalidHostBooking,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    #[account(
        mut,
        seeds = [REPUTATION_PROFILE_SEED.as_bytes(), guest_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = guest_reputation.bump,
    )]
    pub guest_reputation: Box<Account<'info, ReputationProfile>>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.status == BookingStatus::Pending
            || booking.status == BookingStatus::HostAccepted @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Box<Account<'info, Booking>>,

    /// In the future, property will store config data for slashing guest cancelation
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
        token::mint = mint,
        token::authority = host_profile.authority,
        token::token_program = token_program,
    )]
    pub host_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = platform_vault.key() == global_config.platform_vault @ EscrowError::InvalidVaultAccount,
    )]
    pub platform_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = mint.key() == global_config.usdc_mint @ EscrowError::InvalidTokenMint,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Escrow CPI authority PDA — signs the reputation CPI into
    /// stayke-core.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub stayke_core_program: Program<'info, StaykeCore>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_guest_cancel_booking(ctx: Context<GuestCancelBooking>) -> Result<()> {
    validate_guest_cancellation_config(
        CANCELLATION_WINDOW_HOURS,
        CANCELLATION_REFUND_PERCENTAGE,
        CANCELLATION_HOST_SHARE_PERCENTAGE,
    )?;

    let now = Clock::get()?.unix_timestamp;
    let booking = &mut ctx.accounts.booking;

    // check_in must still be in the future. The window is computed only from
    // `check_in` and `now` — never from `updated_at` or any creation timestamp.
    require!(now <= booking.check_in, EscrowError::CheckInPassed);

    // Double-spend guard: the escrow must still hold the full booking amount.
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

    // Amounts to route out of the escrow: (guest, host, stayke).
    let (guest_amount, host_amount, stayke_amount) = {
        let window_seconds = (CANCELLATION_WINDOW_HOURS as i64) * SECONDS_PER_HOUR;
        let time_until_check_in = booking.check_in - now;
        if time_until_check_in > window_seconds {
            // Outside the window: full refund, no host payout, no fee.
            (total_price, 0, 0)
        } else {
            compute_guest_inside_window_split(
                total_price,
                CANCELLATION_REFUND_PERCENTAGE,
                CANCELLATION_HOST_SHARE_PERCENTAGE,
            )?
        }
    };

    if guest_amount > 0 {
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
            guest_amount,
            decimals,
        )?;
    }

    if host_amount > 0 {
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.escrow_token_account.to_account_info(),
                    to: ctx.accounts.host_token_account.to_account_info(),
                    authority: booking.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                },
                booking_seeds,
            ),
            host_amount,
            decimals,
        )?;
    }

    if stayke_amount > 0 {
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.escrow_token_account.to_account_info(),
                    to: ctx.accounts.platform_vault.to_account_info(),
                    authority: booking.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                },
                booking_seeds,
            ),
            stayke_amount,
            decimals,
        )?;
    }

    let cpi_bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[cpi_bump]]];

    let guest_reputation_accounts = stayke_core::cpi::accounts::UpdateReputationProfile {
        reputation_profile: ctx.accounts.guest_reputation.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    stayke_core::cpi::increment_client_cancellations(CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        guest_reputation_accounts,
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

pub fn handler_guest_cancel_booking_cross_year(
    ctx: Context<GuestCancelBookingCrossYear>,
) -> Result<()> {
    validate_guest_cancellation_config(
        CANCELLATION_WINDOW_HOURS,
        CANCELLATION_REFUND_PERCENTAGE,
        CANCELLATION_HOST_SHARE_PERCENTAGE,
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

    let (guest_amount, host_amount, stayke_amount) = {
        let window_seconds = (CANCELLATION_WINDOW_HOURS as i64) * SECONDS_PER_HOUR;
        let time_until_check_in = booking.check_in - now;
        if time_until_check_in > window_seconds {
            (total_price, 0, 0)
        } else {
            compute_guest_inside_window_split(
                total_price,
                CANCELLATION_REFUND_PERCENTAGE,
                CANCELLATION_HOST_SHARE_PERCENTAGE,
            )?
        }
    };

    if guest_amount > 0 {
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
            guest_amount,
            decimals,
        )?;
    }

    if host_amount > 0 {
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.escrow_token_account.to_account_info(),
                    to: ctx.accounts.host_token_account.to_account_info(),
                    authority: booking.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                },
                booking_seeds,
            ),
            host_amount,
            decimals,
        )?;
    }

    if stayke_amount > 0 {
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.escrow_token_account.to_account_info(),
                    to: ctx.accounts.platform_vault.to_account_info(),
                    authority: booking.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                },
                booking_seeds,
            ),
            stayke_amount,
            decimals,
        )?;
    }

    let cpi_bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[cpi_bump]]];

    let guest_reputation_accounts = stayke_core::cpi::accounts::UpdateReputationProfile {
        reputation_profile: ctx.accounts.guest_reputation.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    stayke_core::cpi::increment_client_cancellations(CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        guest_reputation_accounts,
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

/// Validates the hardcoded guest-cancellation policy values so the logic is
/// safe to source from configuration in a future iteration.
fn validate_guest_cancellation_config(
    window_hours: u32,
    refund_percentage: u8,
    host_share_percentage: u8,
) -> Result<()> {
    require!(window_hours > 0, EscrowError::InvalidCancellationWindow);
    require!(
        refund_percentage <= 100,
        EscrowError::InvalidCancellationPercentage
    );
    require!(
        host_share_percentage <= 100,
        EscrowError::InvalidCancellationPercentage
    );
    Ok(())
}

/// Splits `total_price` when a guest cancels inside the window.
///
/// `guest_refund` is `refund_percentage`% of the total, `host_amount` is
/// `host_share_percentage`% of the remainder, and `stayke_amount` is the rest.
/// Because the Stayke share is derived as the remainder, the three amounts
/// always sum exactly to `total_price` — no funds are lost to rounding.
fn compute_guest_inside_window_split(
    total_price: u64,
    refund_percentage: u8,
    host_share_percentage: u8,
) -> Result<(u64, u64, u64)> {
    let guest_refund = (total_price as u128)
        .checked_mul(refund_percentage as u128)
        .ok_or(EscrowError::PriceOverflow)?
        .checked_div(100)
        .ok_or(EscrowError::PriceOverflow)? as u64;

    let remaining = total_price
        .checked_sub(guest_refund)
        .ok_or(EscrowError::PriceOverflow)?;

    let host_amount = (remaining as u128)
        .checked_mul(host_share_percentage as u128)
        .ok_or(EscrowError::PriceOverflow)?
        .checked_div(100)
        .ok_or(EscrowError::PriceOverflow)? as u64;

    let stayke_amount = remaining
        .checked_sub(host_amount)
        .ok_or(EscrowError::PriceOverflow)?;

    Ok((guest_refund, host_amount, stayke_amount))
}

// ---------------------------------------------------------------------------
// Unit tests — pure functions
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{CANCELLATION_REFUND_PERCENTAGE, CANCELLATION_WINDOW_HOURS};

    #[test]
    fn hardcoded_config_is_valid() {
        validate_guest_cancellation_config(
            CANCELLATION_WINDOW_HOURS,
            CANCELLATION_REFUND_PERCENTAGE,
            CANCELLATION_HOST_SHARE_PERCENTAGE,
        )
        .unwrap();
    }

    #[test]
    fn invalid_window_rejected() {
        assert_eq!(
            validate_guest_cancellation_config(0, 60, 75).unwrap_err(),
            EscrowError::InvalidCancellationWindow.into()
        );
    }

    #[test]
    fn invalid_refund_percentage_rejected() {
        assert_eq!(
            validate_guest_cancellation_config(72, 101, 75).unwrap_err(),
            EscrowError::InvalidCancellationPercentage.into()
        );
    }

    #[test]
    fn invalid_host_share_percentage_rejected() {
        assert_eq!(
            validate_guest_cancellation_config(72, 60, 101).unwrap_err(),
            EscrowError::InvalidCancellationPercentage.into()
        );
    }

    #[test]
    fn split_sums_exactly_to_total() {
        for total in [0u64, 1, 2, 3, 7, 99, 100, 101, 100_000, 1_000_001] {
            let (guest, host, stayke) = compute_guest_inside_window_split(total, 60, 75).unwrap();
            assert_eq!(guest + host + stayke, total, "total {total}");
        }
    }

    #[test]
    fn split_mvp_config_matches_expected() {
        let (guest, host, stayke) = compute_guest_inside_window_split(100_000, 60, 75).unwrap();
        assert_eq!(guest, 60_000);
        assert_eq!(host, 30_000);
        assert_eq!(stayke, 10_000);
    }

    #[test]
    fn split_rounding_remainder_goes_to_stayke() {
        // 101 * 60% = 60 (60.6 rounds down), remaining 41, host 75% of 41 = 30
        // (30.75 rounds down), stayke = 41 - 30 = 11.
        let (guest, host, stayke) = compute_guest_inside_window_split(101, 60, 75).unwrap();
        assert_eq!(guest, 60);
        assert_eq!(host, 30);
        assert_eq!(stayke, 11);
        assert_eq!(guest + host + stayke, 101);
    }

    #[test]
    fn split_max_u64_does_not_overflow() {
        let (guest, host, stayke) = compute_guest_inside_window_split(u64::MAX, 60, 75).unwrap();
        assert_eq!(guest + host + stayke, u64::MAX);
    }
}
