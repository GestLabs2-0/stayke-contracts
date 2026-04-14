use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use stayke_core::{
    ClearListingBooking, cpi::{accounts::UpdateUserProfile, clear_active_booking, clear_listing_booking}, state::{Listing, UserProfile}
};
use stayke_escrow::{
    cpi::{
        accounts::{ResolveDisputeTransferCpi, UpdateBookingStatusCpi},
        cpi_resolve_dispute_transfer, cpi_update_booking_status,
    },
    program::StaykeEscrow,
    state::Booking,
};

use crate::{
    error::DisputeError,
    events::{DisputeOpened, DisputeResolved},
    state::{Dispute, DisputeConfig, DisputeReason, DisputeStatus},
};

// ---------------------------------------------------------------------------
// Open Dispute
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct OpenDispute<'info> {
    #[account(mut)]
    pub initiator: Signer<'info>,

    #[account(
        seeds = [b"user_profile", initiator.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = initiator_profile.bump,
        constraint = !initiator_profile.banned @ DisputeError::UserBanned,
        constraint = initiator_profile.is_verified @ DisputeError::UserNotVerified,
    )]
    pub initiator_profile: Account<'info, UserProfile>,

    /// We must mutate the booking state via CPI
    #[account(mut)]
    pub booking: Box<Account<'info, Booking>>,

    #[account(
        init,
        payer = initiator,
        space = 8 + Dispute::INIT_SPACE,
        seeds = [b"dispute", booking.key().as_ref()],
        bump,
    )]
    pub dispute: Box<Account<'info, Dispute>>,

    pub stayke_escrow_program: Program<'info, StaykeEscrow>,
    pub system_program: Program<'info, System>,
}

pub fn handler_open_dispute(ctx: Context<OpenDispute>, reason: DisputeReason) -> Result<()> {
    require!(
        ctx.accounts.booking.guest == ctx.accounts.initiator.key()
            || ctx.accounts.booking.host == ctx.accounts.initiator.key(),
        DisputeError::UnauthorizedDisputeInitiator
    );
    require!(
        ctx.accounts.booking.status == stayke_escrow::state::BookingStatus::Active,
        DisputeError::BookingNotActive
    );

    let initiator_key = ctx.accounts.initiator_profile.key();
    let booking = &ctx.accounts.booking;

    let guilty = if initiator_key == booking.host {
        booking.host
    } else {
        booking.guest
    };

    let dispute = &mut ctx.accounts.dispute;
    dispute.booking = ctx.accounts.booking.key();
    dispute.property = ctx.accounts.booking.property;
    dispute.initiator = initiator_key;
    dispute.guilty = guilty;
    dispute.reason = reason.clone();
    dispute.status = DisputeStatus::Open;
    dispute.created_at = Clock::get()?.unix_timestamp;
    dispute.resolved_at = None;
    dispute.bump = ctx.bumps.dispute;

    // CPI to stayke-escrow to update booking status
    let cpi_accounts = UpdateBookingStatusCpi {
        booking: ctx.accounts.booking.to_account_info(),
        authority: ctx.accounts.initiator.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.stayke_escrow_program.key(), cpi_accounts);
    cpi_update_booking_status(cpi_ctx, stayke_escrow::state::BookingStatus::Disputed)?;

    emit!(DisputeOpened {
        dispute: dispute.key(),
        booking: dispute.booking,
        property: dispute.property,
        initiator: dispute.initiator,
        reason,
        timestamp: dispute.created_at,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Resolve Dispute (Admin determines blame and routes funds)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct ResolveDispute<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"dispute_config"],
        constraint = config.admins.contains(&admin.key()) @ DisputeError::UnauthorizedAdmin,
        bump = config.bump,
    )]
    pub config: Box<Account<'info, DisputeConfig>>,

    #[account(
        mut,
        seeds = [b"dispute", booking.key().as_ref()],
        bump = dispute.bump,
        constraint = dispute.status == DisputeStatus::Open @ DisputeError::DisputeNotOpen,
    )]
    pub dispute: Account<'info, Dispute>,

    #[account(mut)]
    pub booking: Box<Account<'info, Booking>>,

    // Escrow Accounts needed for the CPI:
    /// CHECK: Escrow config validated by stayke-escrow program during CPI.
    pub escrow_config: UncheckedAccount<'info>,

    // TODO: add validations for token accounts. Platform and usdc_mint need to be equal to the other config files
    #[account(mut)]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub host_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub guest_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub platform_vault_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    pub stayke_escrow_program: Program<'info, StaykeEscrow>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_resolve_dispute(
    ctx: Context<ResolveDispute>,
    host_share_bps: u16,
    rejected: bool,
) -> Result<()> {
    // Escrow transfer CPI
    let cpi_accounts = ResolveDisputeTransferCpi {
        authority: ctx.accounts.admin.to_account_info(),
        booking: ctx.accounts.booking.to_account_info(),
        escrow_config: ctx.accounts.escrow_config.to_account_info(),
        escrow_token_account: ctx.accounts.escrow_token_account.to_account_info(),
        host_token_account: ctx.accounts.host_token_account.to_account_info(),
        guest_token_account: ctx.accounts.guest_token_account.to_account_info(),
        platform_vault_token_account: ctx.accounts.platform_vault_token_account.to_account_info(),
        mint: ctx.accounts.usdc_mint.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.stayke_escrow_program.key(), cpi_accounts);
    cpi_resolve_dispute_transfer(cpi_ctx, host_share_bps, rejected)?;

    let dispute = &mut ctx.accounts.dispute;
    dispute.status = if rejected {
        DisputeStatus::Rejected
    } else {
        DisputeStatus::Resolved
    };
    dispute.resolved_at = Some(Clock::get()?.unix_timestamp);

    emit!(DisputeResolved {
        dispute: dispute.key(),
        booking: dispute.booking,
        host_share_bps,
        timestamp: dispute.resolved_at.unwrap(),
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Close Dispute (Closes dispute account and clears active_booking flags)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct CloseDispute<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"dispute_config"],
        constraint = config.admins.contains(&admin.key()) @ DisputeError::UnauthorizedAdmin,
        bump = config.bump,
    )]
    pub config: Account<'info, DisputeConfig>,

    #[account(
        mut,
        close = admin,
        seeds = [b"dispute", booking.key().as_ref()],
        bump = dispute.bump,
        constraint = dispute.status != DisputeStatus::Open @ DisputeError::DisputeNotOpen,
    )]
    pub dispute: Account<'info, Dispute>,

    /// CHECK: Read-only access here, trust it's the right booking if the dispute seed matches.
    pub booking: Account<'info, Booking>,

    // Note: The caller MUST pass the exact UserProfiles corresponding to the guest and host
    // of this booking, as well as the listing. We cannot enforce seeds entirely off-chain
    // unless we bring stayke-core profiles directly into scope but the instructions exist to do it dynamically.
    // So we just take mutable UserProfile accounts and let the caller invoke `clear_active_booking`.

    // Instead of forcing all 3 in the main struct if they are not always needed, we could use them directly.
    #[account(mut)]
    pub guest_profile: Account<'info, UserProfile>,

    #[account(mut)]
    pub host_profile: Account<'info, UserProfile>,

    #[account(mut)]
    pub listing: Account<'info, Listing>,

    // Currently we mapped Listing to have active_booking in the monolith, wait: In our new core design, Listing doesn't have active_booking, it has `is_occupied: Option<Pubkey>`.
    // We didn't create a mutator for `is_occupied`. Let's just clear the users since user_profile has `active_booking` and `active_stay`!
    pub stayke_core_program: Program<'info, stayke_core::program::StaykeCore>,
}

// TODO: instead of the admin users, we must only use the account PDA as the signer, but for simplicity we can just use the admin signer for now.
// We just need to make sure that only the admin can call this instruction, which is already enforced by the account constraint.
pub fn handler_close_dispute(ctx: Context<CloseDispute>) -> Result<()> {
    // 1. Clear Guest's active booking
    let guest_cpi_accounts = UpdateUserProfile {
        user_profile: ctx.accounts.guest_profile.to_account_info(),
        authority: ctx.accounts.admin.to_account_info(),
    };
    clear_active_booking(CpiContext::new(
        ctx.accounts.stayke_core_program.key(),
        guest_cpi_accounts,
    ))?;

    // 2. Clear Host's active stay
    let host_cpi_accounts = UpdateUserProfile {
        user_profile: ctx.accounts.host_profile.to_account_info(),
        authority: ctx.accounts.admin.to_account_info(),
    };
    clear_active_booking(CpiContext::new(
        ctx.accounts.stayke_core_program.key(),
        host_cpi_accounts,
    ))?;

    let 

    let clear_listing_accounts = ClearListingBooking {
        authority: ctx.accounts.admin.to_account_info(),
        listing: ctx.accounts.listing.to_account_info()
        authority: ctx.accounts.admin.to_account_info()
    };

    clear_listing_booking()

    Ok(())
}
