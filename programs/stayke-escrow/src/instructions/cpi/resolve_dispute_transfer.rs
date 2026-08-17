use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, CloseAccount, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use stayke_config::{
    assert_cpi_authority, error::StaykeConfigError, AllowedCaller, GlobalConfig, GLOBAL_CONFIG_SEED,
};

use stayke_core::{constants::USER_PROFILE_SEED, UserProfile};

use crate::{
    constants::{BOOKING_SEED, ESCROW_PDA_SEED},
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingStatus},
};

// ---------------------------------------------------------------------------
// CPI Endpoint: Resolve Dispute Transfer (Used by stayke-disputes)
// ---------------------------------------------------------------------------
// This executes the actual fund transfers and closes the escrow account.

#[derive(Accounts)]
pub struct ResolveDisputeTransferCpi<'info> {
    // The authority invoking the CPI (typically stayke-disputes admin)
    pub cpi_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [
            BOOKING_SEED.as_bytes(),
            booking.property.as_ref(),
            booking.guest.as_ref(),
            booking.check_in.to_le_bytes().as_ref(),
        ],
        bump = booking.bump,
        constraint = booking.status == BookingStatus::Disputed @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Box<Account<'info, Booking>>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = booking.host == host_profile.key() @ EscrowError::InvalidHostBooking,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), guest_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = guest_profile.bump,
        constraint = booking.guest == guest_profile.key() @ EscrowError::UnauthorizedBooking,
    )]
    pub guest_profile: Box<Account<'info, UserProfile>>,

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
    )]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Host's USDC — must belong to the host wallet.
    #[account(
        mut,
        constraint = host_token_account.mint == mint.key() @ EscrowError::InvalidTokenMint,
        constraint = host_token_account.owner == host_profile.authority @ EscrowError::InvalidPayoutTokenAccount,
    )]
    pub host_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Guest's USDC — must belong to the guest wallet.
    #[account(
        mut,
        constraint = guest_token_account.mint == mint.key() @ EscrowError::InvalidTokenMint,
        constraint = guest_token_account.owner == guest_profile.authority @ EscrowError::InvalidPayoutTokenAccount,
    )]
    pub guest_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Platform vault
    #[account(
        mut,
        constraint = platform_vault_token_account.key() == global_config.platform_vault @ StaykeConfigError::InvalidVaultAccount,
    )]
    pub platform_vault_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = mint.key() == global_config.usdc_mint @ StaykeConfigError::InvalidTokenMint,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_cpi_resolve_dispute_transfer(
    ctx: Context<ResolveDisputeTransferCpi>,
    host_share_bps: u16,
    rejected: bool,
) -> Result<()> {
    require!(host_share_bps <= 10_000, StaykeConfigError::InvalidFeeBps);
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Disputes],
    )?;

    let booking = &mut ctx.accounts.booking;
    let config = &ctx.accounts.global_config;
    let decimals = ctx.accounts.mint.decimals;
    let total = booking.total_price;

    let fee = (total as u128)
        .saturating_mul(config.fee_bps as u128)
        .saturating_div(10_000) as u64;
    let distributable = total.saturating_sub(fee);

    let host_amount = if rejected {
        distributable
    } else {
        (distributable as u128)
            .saturating_mul(host_share_bps as u128)
            .saturating_div(10_000) as u64
    };
    let guest_amount = distributable.saturating_sub(host_amount);

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

    if fee > 0 {
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.escrow_token_account.to_account_info(),
                    to: ctx.accounts.platform_vault_token_account.to_account_info(),
                    authority: booking.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                },
                booking_seeds,
            ),
            fee,
            decimals,
        )?;
    }

    // Close the escrow account and return lamports to the admin authority (or platform vault owner)
    token_interface::close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        CloseAccount {
            account: ctx.accounts.escrow_token_account.to_account_info(),
            destination: ctx.accounts.platform_vault_token_account.to_account_info(),
            authority: booking.to_account_info(),
        },
        booking_seeds,
    ))?;

    let final_status = if rejected {
        BookingStatus::DisputeRejected
    } else {
        BookingStatus::DisputeResolved
    };

    booking.status = final_status.clone();

    emit!(BookingStatusUpdated {
        status: final_status,
        booking: booking.key()
    });

    Ok(())
}
