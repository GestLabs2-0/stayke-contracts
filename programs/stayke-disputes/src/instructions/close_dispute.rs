use anchor_lang::prelude::*;
use stayke_config::{GlobalConfig, CPI_AUTHORITY_SEED, GLOBAL_CONFIG_SEED};
use stayke_core::{
    cpi::{
        accounts::UpdateUserProfile, clear_active_booking, increment_completed_stays,
        increment_hosted_stays,
    },
    program::StaykeCore,
    state::UserProfile,
    USER_PROFILE_SEED,
};
use stayke_escrow::{
    constants::BOOKING_SEED,
    cpi::{accounts::UpdateBookingStatusCpi, cpi_update_booking_status},
    program::StaykeEscrow,
    state::{Booking, BookingStatus},
};

use crate::{
    constants::DISPUTE_PDA_SEED,
    error::DisputeError,
    events::DisputeClosed,
    state::{DisputeAccount, DisputeParty, DisputeState},
};

// ---------------------------------------------------------------------------
// Close dispute — permissionless finalization of a resolved dispute.
//
// Anyone can invoke this once the dispute reached a terminal resolution state
// (ResolvedByAdmin or ResolvedByP2P). It archives the dispute (state -> Closed),
// performs the stay bookkeeping that only applies to the admin-resolved route,
// and closes the account returning the rent to the wallet of the party that
// opened the dispute.
//
// Settlement bookkeeping by route:
// - ResolvedByP2P: `solve_dispute_before_admin` already restored the booking to
//   its original status, so the normal escrow flow (release_funds) continues and
//   counts the stays. This instruction only closes the account.
// - ResolvedByAdmin:
//   * booking still `Disputed` (NoFaultFound outcome): the escrow was NOT
//     distributed; restore `original_booking_status` so the normal flow releases
//     the funds. Stays are counted there.
//   * booking already `DisputeResolved` (funds distributed by
//     `cpi_resolve_dispute_transfer`): `release_funds` will never run for this
//     booking, so the guest/host stays counters are settled here.
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct CloseDispute<'info> {
    /// Dispute PDA, seeded from the booking key. Only a resolved dispute can be closed.
    #[account(
        mut,
        seeds = [DISPUTE_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump = dispute.bump,
        constraint = dispute.state == DisputeState::ResolvedByAdmin
            || dispute.state == DisputeState::ResolvedByP2P @ DisputeError::DisputeNotResolved,
    )]
    pub dispute: Box<Account<'info, DisputeAccount>>,

    /// The booking this dispute was opened against.
    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        seeds::program = stayke_escrow::ID,
        bump = booking.bump,
        constraint = dispute.booking == booking.key() @ DisputeError::UnboundBooking,
    )]
    pub booking: Box<Account<'info, Booking>>,

    /// The guest's UserProfile — mutable for the `completed_stays` CPI on the
    /// admin-resolved route.
    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), guest_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = guest_profile.bump,
        constraint = guest_profile.key() == booking.guest @ DisputeError::UnboundBookingAccount,
    )]
    pub guest_profile: Box<Account<'info, UserProfile>>,

    /// The host's UserProfile — mutable for the `hosted_stays` CPI on the
    /// admin-resolved route.
    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = host_profile.key() == booking.host @ DisputeError::UnboundBookingAccount,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    /// Wallet of the party that opened the dispute — receives the account rent
    /// when the dispute account is closed.
    #[account(mut)]
    pub opener_wallet: SystemAccount<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_config::ID,
        bump = global_config.bump,
        constraint = global_config.is_initialized,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: Disputes CPI authority PDA — signs privileged core/escrow mutators.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub stayke_core_program: Program<'info, StaykeCore>,
    pub stayke_escrow_program: Program<'info, StaykeEscrow>,
}

pub fn handler_close_dispute(ctx: Context<CloseDispute>) -> Result<()> {
    let dispute = &mut ctx.accounts.dispute;

    // The rent destination must be the wallet of the party that opened the dispute.
    let opener_authority = match dispute.opened_by {
        DisputeParty::Guest => ctx.accounts.guest_profile.authority,
        DisputeParty::Host => ctx.accounts.host_profile.authority,
    };
    require!(
        ctx.accounts.opener_wallet.key() == opener_authority,
        DisputeError::InvalidOpenerWallet
    );

    let bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];

    if dispute.state == DisputeState::ResolvedByAdmin {
        match ctx.accounts.booking.status {
            // NoFaultFound: the escrow was not distributed, so restore the booking
            // to its pre-dispute status and let the normal flow release the funds
            // (and count the stays).
            BookingStatus::Disputed => {
                let cpi_accounts = UpdateBookingStatusCpi {
                    booking: ctx.accounts.booking.to_account_info(),
                    cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
                    global_config: ctx.accounts.global_config.to_account_info(),
                };
                cpi_update_booking_status(
                    CpiContext::new_with_signer(
                        ctx.accounts.stayke_escrow_program.key(),
                        cpi_accounts,
                        signer_seeds,
                    ),
                    dispute.original_booking_status.clone(),
                )?;
            }
            // Escrow distributed by `cpi_resolve_dispute_transfer` (booking already
            // DisputeResolved): `release_funds` will never run for this booking, so
            // the stays are settled here.
            _ => {
                let guest_cpi_accounts = UpdateUserProfile {
                    user_profile: ctx.accounts.guest_profile.to_account_info(),
                    global_config: ctx.accounts.global_config.to_account_info(),
                    cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
                };
                increment_completed_stays(CpiContext::new_with_signer(
                    ctx.accounts.stayke_core_program.key(),
                    guest_cpi_accounts,
                    signer_seeds,
                ))?;

                let guest_cpi_accounts = UpdateUserProfile {
                    user_profile: ctx.accounts.guest_profile.to_account_info(),
                    global_config: ctx.accounts.global_config.to_account_info(),
                    cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
                };

                clear_active_booking(CpiContext::new_with_signer(
                    ctx.accounts.stayke_core_program.key(),
                    guest_cpi_accounts,
                    signer_seeds,
                ))?;

                let host_cpi_accounts = UpdateUserProfile {
                    user_profile: ctx.accounts.host_profile.to_account_info(),
                    global_config: ctx.accounts.global_config.to_account_info(),
                    cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
                };
                increment_hosted_stays(CpiContext::new_with_signer(
                    ctx.accounts.stayke_core_program.key(),
                    host_cpi_accounts,
                    signer_seeds,
                ))?;
            }
        }
    }

    let closed_at = Clock::get()?.unix_timestamp;
    let opened_by = dispute.opened_by.clone();
    dispute.state = DisputeState::Closed;

    emit!(DisputeClosed {
        dispute: dispute.key(),
        booking: dispute.booking,
        closed_by: opened_by,
        closed_at,
    });

    // Close the dispute account, returning the rent to the dispute opener.
    ctx.accounts
        .dispute
        .close(ctx.accounts.opener_wallet.to_account_info())?;

    Ok(())
}
