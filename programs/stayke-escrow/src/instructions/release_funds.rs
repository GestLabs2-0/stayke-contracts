use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, CloseAccount, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use stayke_core::program::StaykeCore;
use stayke_core::{constants::USER_PROFILE_SEED, UserProfile};

use stayke_config::{GlobalConfig, CPI_AUTHORITY_SEED, GLOBAL_CONFIG_SEED};

use crate::{
    constants::{BOOKING_SEED, ESCROW_PDA_SEED},
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingStatus},
};

// ---------------------------------------------------------------------------
// Release funds — permissionless settlement of a completed stay once the
// dispute window closes.
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct ReleaseFunds<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The guest's UserProfile — mutable for the `completed_stays` CPI.
    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), guest_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = guest_profile.bump,
        constraint = guest_profile.key() == booking.guest @ EscrowError::UnauthorizedBooking,
    )]
    pub guest_profile: Box<Account<'info, UserProfile>>,

    /// The host's UserProfile — mutable for the `hosted_stays` CPI.
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
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.status == BookingStatus::Completed @ EscrowError::BookingNotCompleted,
    )]
    pub booking: Box<Account<'info, Booking>>,

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

    /// The host's USDC token account — the funds destination, bound to the
    /// host's authority so the caller cannot redirect the payment.
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

    /// CHECK: Escrow CPI authority PDA — signs privileged core mutators.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub stayke_core_program: Program<'info, StaykeCore>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_release_funds(ctx: Context<ReleaseFunds>) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    let now = Clock::get()?.unix_timestamp;

    // Dispute window: funds can only be released 24h after the stay completed.
    require!(
        now >= booking.updated_at + (24 * 60 * 60),
        EscrowError::ReleaseWindowNotElapsed
    );

    // A disputed booking can no longer be released here. The `Completed`
    // constraint already enforces this, but keep the invariant explicit.
    require!(
        booking.status != BookingStatus::Disputed,
        EscrowError::BookingNotDisputable
    );

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

    // Close the escrow token account, returning rent to the permissionless caller.
    token_interface::close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        CloseAccount {
            account: ctx.accounts.escrow_token_account.to_account_info(),
            destination: ctx.accounts.payer.to_account_info(),
            authority: booking.to_account_info(),
        },
        booking_seeds,
    ))?;

    booking.status = BookingStatus::Released;

    // CPI to stayke-core: increment the guest's completed_stays counter.
    let bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];

    let guest_cpi_accounts = stayke_core::cpi::accounts::UpdateUserProfile {
        user_profile: ctx.accounts.guest_profile.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    stayke_core::cpi::increment_completed_stays(CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        guest_cpi_accounts,
        signer_seeds,
    ))?;

    let guest_cpi_accounts = stayke_core::cpi::accounts::UpdateUserProfile {
        user_profile: ctx.accounts.guest_profile.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    stayke_core::cpi::clear_active_booking(CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        guest_cpi_accounts,
        signer_seeds,
    ))?;

    // CPI to stayke-core: increment the host's hosted_stays counter.
    let host_cpi_accounts = stayke_core::cpi::accounts::UpdateUserProfile {
        user_profile: ctx.accounts.host_profile.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    stayke_core::cpi::increment_hosted_stays(CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        host_cpi_accounts,
        signer_seeds,
    ))?;

    emit!(BookingStatusUpdated {
        status: BookingStatus::Released,
        booking: booking.key()
    });
    Ok(())
}
