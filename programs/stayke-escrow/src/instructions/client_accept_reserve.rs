use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked},
};
use stayke_core::{
    constants::{LISTING_SEED, USER_PROFILE_SEED, CPI_AUTHORITY_SEED},
    cpi::accounts::SetListingOccupied,
    Listing, UserProfile,
};
use stayke_core::program::StaykeCore;

use stayke_config::{error::StaykeConfigError, GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::EscrowConfig;
use crate::{
    constants::{BOOKING_SEED, ESCROW_CONFIG_SEED, ESCROW_PDA_SEED},
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingStatus},
};

#[derive(Accounts)]
pub struct ClientAcceptReserve<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub client: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), client.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = client_profile.bump,
        constraint = client.key() == client_profile.authority @ EscrowError::UnauthorizedBooking,
        constraint = !client_profile.banned @ EscrowError::UserBanned,
        constraint = client_profile.identity.is_some() @ EscrowError::UserNotVerified,
        constraint = client_profile.deposited >= global_config.minimum_deposit @ EscrowError::InsufficientDeposit,
    )]
    pub client_profile: Box<Account<'info, UserProfile>>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.status == BookingStatus::HostAccepted @ EscrowError::InvalidBookingStatus,
        constraint = booking.guest == client_profile.key() @ EscrowError::UnauthorizedBooking,
    )]
    pub booking: Box<Account<'info, Booking>>,

    #[account(
        mut,
        seeds = [LISTING_SEED.as_bytes(), listing.owner.as_ref(), listing.listing_id.to_le_bytes().as_ref()],
        bump = listing.bump,
        constraint = booking.property == listing.key() @ EscrowError::InvalidBookingProperty
    )]
    pub listing: Box<Account<'info, Listing>>,

    /// Host profile that owns the listing (required for set_listing_occupied CPI seeds).
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = listing.owner == host_profile.key() @ EscrowError::InvalidHostBooking,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
        constraint = escrow_config.global_config == global_config.key() @ StaykeConfigError::InvalidGlobalConfig,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(seeds = [ESCROW_CONFIG_SEED.as_bytes()], bump = escrow_config.bump)]
    pub escrow_config: Box<Account<'info, EscrowConfig>>,

    #[account(mut, constraint = mint.key() == global_config.usdc_mint @ StaykeConfigError::InvalidTokenMint)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = client,
    )]
    pub client_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        payer = payer,
        token::mint = mint,
        token::authority = booking,
        seeds = [ESCROW_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump,
    )]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Escrow CPI authority PDA — signs privileged core mutators (A2).
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub stayke_core_program: Program<'info, StaykeCore>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler_client_accept_reserve(ctx: Context<ClientAcceptReserve>) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    let now = Clock::get()?.unix_timestamp;
    if now < booking.check_in - 86400 {
        return err!(EscrowError::TooEarlyToActivate);
    }

    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.client_token_account.to_account_info(),
                to: ctx.accounts.escrow_token_account.to_account_info(),
                authority: ctx.accounts.client.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        booking.total_price,
        ctx.accounts.mint.decimals,
    )?;

    let bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];
    let set_occupied_accounts = SetListingOccupied {
        user_profile: ctx.accounts.host_profile.to_account_info(),
        listing: ctx.accounts.listing.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    stayke_core::cpi::set_listing_occupied(
        CpiContext::new_with_signer(
            ctx.accounts.stayke_core_program.key(),
            set_occupied_accounts,
            signer_seeds,
        ),
        true,
    )?;

    let booking = &mut ctx.accounts.booking;
    booking.status = BookingStatus::Active;
    booking.escrow_bump = ctx.bumps.escrow_token_account;

    emit!(BookingStatusUpdated {
        status: BookingStatus::Active,
        booking: booking.key()
    });
    Ok(())
}
