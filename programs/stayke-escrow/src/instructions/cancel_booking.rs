use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, CloseAccount, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use stayke_config::{GlobalConfig, CPI_AUTHORITY_SEED, GLOBAL_CONFIG_SEED};
use stayke_core::{
    constants::{REPUTATION_PROFILE_SEED, USER_PROFILE_SEED},
    program::StaykeCore,
    Listing, ReputationProfile, UserProfile,
};
use stayke_treasury::{program::StaykeTreasury, TreasuryConfig, TREASURY_CONFIG_SEED};

use crate::{
    constants::{
        BOOKING_DAYS_SEED, BOOKING_SEED, CANCELLATION_HOST_SHARE_PERCENTAGE,
        CANCELLATION_REFUND_PERCENTAGE, CANCELLATION_WINDOW_HOURS, ESCROW_PDA_SEED,
        HOST_CANCELLATION_PENALTY_PERCENTAGE,
    },
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingDays, BookingStatus},
    utils::{derive_date, release_days_single_year, TimestampExt},
};

/// Number of seconds in an hour.
const SECONDS_PER_HOUR: i64 = 3_600;

// ---------------------------------------------------------------------------
// Cancel booking — guest or host cancellation before check-in.
//
// The caller must be the booking's guest or host. The cancellation policy
// differs by actor:
//
// * Guest outside the window (more than `CANCELLATION_WINDOW_HOURS` before
//   check-in): full refund, no host payout, no platform fee.
// * Guest inside the window: refund split (guest / host / Stayke) with the
//   Stayke share derived as the remainder so no funds are lost to rounding.
// * Host: full refund to the guest plus a deposit slash paid to the guest,
//   calculated over the host's available deposit (capped at that deposit).
//
// The escrow token account is closed and the reserved days are released.
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct CancelBooking<'info> {
    /// The guest or host of the booking. Validated against the two profiles and
    /// also receives the escrow account rent on closure.
    #[account(
        mut,
        constraint = caller.key() == guest_profile.authority
            || caller.key() == host_profile.authority @ EscrowError::UnauthorizedCancellation
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

    /// The host's UserProfile — mutable for the deposit slash on host
    /// cancellation.
    #[account(
        mut,
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
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.status == BookingStatus::Pending
            || booking.status == BookingStatus::HostAccepted @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Box<Account<'info, Booking>>,

    /// The listing the booking belongs to — validated against `booking.property`.
    #[account(
        constraint = property.key() == booking.property @ EscrowError::InvalidBookingProperty,
    )]
    pub property: Box<Account<'info, Listing>>,

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

    /// The guest's USDC token account — destination for the refund (and the
    /// host's deposit slash on host cancellation), bound to the guest's
    /// authority so the caller cannot redirect the funds.
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

pub fn handler_cancel_booking(ctx: Context<CancelBooking>) -> Result<()> {
    validate_cancellation_config(
        CANCELLATION_WINDOW_HOURS,
        CANCELLATION_REFUND_PERCENTAGE,
        HOST_CANCELLATION_PENALTY_PERCENTAGE,
    )?;

    let now = Clock::get()?.unix_timestamp;
    let booking = &mut ctx.accounts.booking;

    // check_in must still be in the future. The window is computed only from
    // `check_in` and `now` — never from `updated_at` or any creation timestamp.
    require!(now < booking.check_in, EscrowError::CheckInPassed);

    let is_guest = ctx.accounts.caller.key() == ctx.accounts.guest_profile.authority;
    let is_host = ctx.accounts.caller.key() == ctx.accounts.host_profile.authority;
    require!(is_guest || is_host, EscrowError::UnauthorizedCancellation);

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
    let (guest_amount, host_amount, stayke_amount) = if is_guest {
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
    } else {
        // Host cancellation: full refund to the guest, nothing to the host.
        (total_price, 0, 0)
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

    if is_host {
        // Host cancellation: slash the host's available deposit and pay it to
        // the guest. The slash is capped at the available deposit.
        let slash = compute_host_slash(
            ctx.accounts.host_profile.deposited,
            HOST_CANCELLATION_PENALTY_PERCENTAGE,
        )?;

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
    } else {
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
    }

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

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested)
// ---------------------------------------------------------------------------

/// Validates the hardcoded cancellation policy values so the logic is safe to
/// source from configuration in a future iteration.
fn validate_cancellation_config(
    window_hours: u32,
    refund_percentage: u8,
    penalty_percentage: u8,
) -> Result<()> {
    require!(window_hours > 0, EscrowError::InvalidCancellationWindow);
    require!(
        refund_percentage <= 100,
        EscrowError::InvalidCancellationPercentage
    );
    require!(
        penalty_percentage <= 100,
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
    use crate::constants::{
        CANCELLATION_REFUND_PERCENTAGE, CANCELLATION_WINDOW_HOURS,
        HOST_CANCELLATION_PENALTY_PERCENTAGE,
    };

    #[test]
    fn hardcoded_config_is_valid() {
        validate_cancellation_config(
            CANCELLATION_WINDOW_HOURS,
            CANCELLATION_REFUND_PERCENTAGE,
            HOST_CANCELLATION_PENALTY_PERCENTAGE,
        )
        .unwrap();
    }

    #[test]
    fn invalid_window_rejected() {
        assert_eq!(
            validate_cancellation_config(0, 60, 10).unwrap_err(),
            EscrowError::InvalidCancellationWindow.into()
        );
    }

    #[test]
    fn invalid_refund_percentage_rejected() {
        assert_eq!(
            validate_cancellation_config(72, 101, 10).unwrap_err(),
            EscrowError::InvalidCancellationPercentage.into()
        );
    }

    #[test]
    fn invalid_penalty_percentage_rejected() {
        assert_eq!(
            validate_cancellation_config(72, 60, 101).unwrap_err(),
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
