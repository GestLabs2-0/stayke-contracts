use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, CloseAccount, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use stayke_core::program::StaykeCore;
use stayke_core::{constants::USER_PROFILE_SEED, UserProfile};

use stayke_config::{
    error::StaykeConfigError, GlobalConfig, CPI_AUTHORITY_SEED, GLOBAL_CONFIG_SEED,
};

use crate::EscrowConfig;
use crate::{
    constants::{BOOKING_SEED, ESCROW_CONFIG_SEED, ESCROW_PDA_SEED},
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingStatus},
};

// ---------------------------------------------------------------------------
// Complete stay — distributes escrow to host (minus fee) and platform vault
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct CompleteStay<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub client: Signer<'info>,

    /// The guest's UserProfile — must be mutable for CPI to increment completed_stays.
    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), client.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = client_profile.bump,
        constraint = client.key() == client_profile.authority @ EscrowError::UnauthorizedBooking,
        constraint = !client_profile.banned @ EscrowError::UserBanned,
    )]
    pub client_profile: Box<Account<'info, UserProfile>>,

    /// The host's UserProfile — destination for the payment.
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
    )]
    pub host_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        close = client,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.guest == client_profile.key() @ EscrowError::UnauthorizedBooking,
        constraint = booking.host == host_profile.key() @ EscrowError::InvalidHostBooking,
        constraint = booking.status == BookingStatus::ReviewCompleted @ EscrowError::BookingNotReviewCompleted,
    )]
    pub booking: Box<Account<'info, Booking>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(seeds = [ESCROW_CONFIG_SEED.as_bytes()], bump = escrow_config.bump)]
    pub escrow_config: Box<Account<'info, EscrowConfig>>,

    #[account(
        mut,
        seeds = [ESCROW_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump = booking.escrow_bump,
        token::mint = mint,
        token::authority = booking,
    )]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Host payout ATA — must be a token account of `mint` owned by the host wallet.
    #[account(
        mut,
        constraint = host_token_account.mint == mint.key() @ EscrowError::InvalidTokenMint,
        constraint = host_token_account.owner == host_profile.authority @ EscrowError::InvalidPayoutTokenAccount,
    )]
    pub host_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Platform fee vault.
    #[account(
        mut,
        constraint = platform_vault.key() == global_config.platform_vault @ StaykeConfigError::InvalidVaultAccount,
    )]
    pub platform_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, constraint = mint.key() == global_config.usdc_mint @ StaykeConfigError::InvalidTokenMint)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Escrow CPI authority PDA — signs privileged core mutators.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub stayke_core_program: Program<'info, StaykeCore>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_complete_stay(ctx: Context<CompleteStay>) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    let config = &ctx.accounts.global_config;
    let decimals = ctx.accounts.mint.decimals;

    let fee = (booking.total_price as u128)
        .saturating_mul(config.fee_bps as u128)
        .saturating_div(10_000) as u64;
    let host_amount = booking.total_price.saturating_sub(fee);

    let booking_seeds: &[&[&[u8]]] = &[&[
        BOOKING_SEED.as_bytes(),
        booking.property.as_ref(),
        booking.guest.as_ref(),
        &booking.check_in.to_le_bytes(),
        &[booking.bump],
    ]];

    if host_amount > 0 {
        // TODO: should I implement somekind of conditional if the host is banned. What happens to the money if the host is banned after the stay is completed but before the booking is closed?
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

    if fee > 0 {
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
            fee,
            decimals,
        )?;
    }

    // Close the escrow token account, returning rent to the client.
    token_interface::close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        CloseAccount {
            account: ctx.accounts.escrow_token_account.to_account_info(),
            destination: ctx.accounts.client.to_account_info(),
            authority: booking.to_account_info(),
        },
        booking_seeds,
    ))?;

    booking.status = BookingStatus::Completed;

    emit!(BookingStatusUpdated {
        status: BookingStatus::Completed,
        booking: booking.key()
    });

    // CPI to stayke-core: increment the guest's completed_stays counter.
    // This feeds the free-tier deposit bypass logic.
    let bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];
    let increment_accounts = stayke_core::cpi::accounts::UpdateUserProfile {
        user_profile: ctx.accounts.client_profile.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    stayke_core::cpi::increment_completed_stays(CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        increment_accounts,
        signer_seeds,
    ))?;

    Ok(())
}
