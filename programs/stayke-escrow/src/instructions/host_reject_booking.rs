use anchor_lang::prelude::*;

use anchor_spl::token_interface::{
    self, CloseAccount, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use stayke_config::{self, GlobalConfig, GLOBAL_CONFIG_SEED};
use stayke_core::{constants::USER_PROFILE_SEED, UserProfile};

use crate::{
    constants::{BOOKING_DAYS_SEED, BOOKING_SEED, ESCROW_PDA_SEED},
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingDays, BookingStatus},
    utils::{derive_date, release_days_cross_years, release_days_single_year, TimestampExt},
};

// TODO: change reputation if host rejects inside a 48 hours frame

// ---------------------------------------------------------------------------
// Host: reject pending booking (releases days, closes booking account)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct HostRejectBooking<'info> {
    // Payer is only a referenced for transaction paid using the relayer
    // I think I should delete this and I will after MVP
    pub payer: Signer<'info>,

    #[account(
        constraint = host_profile.authority == host.key() @ EscrowError::UnauthorizedHost
    )]
    pub host: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = !host_profile.banned @ EscrowError::UserBanned,
        constraint = host_profile.identity.is_some() @ EscrowError::UserNotVerified,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    // TODO: check if lamports for closing booking goes to payer
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), guest.authority.as_ref()],
        bump = guest.bump,
        seeds::program = stayke_core::ID,
        constraint = booking.guest == guest.key() @ EscrowError::WrongGuestPassed
    )]
    pub guest: Box<Account<'info, UserProfile>>,

    #[account(
        mut,
        // TODO: check if money should go to host
        close = payer,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.host == host_profile.key() @ EscrowError::InvalidBookingProperty,
        constraint = booking.status == BookingStatus::Pending @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Box<Account<'info, Booking>>,

    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking.property.as_ref(), &booking.check_in.year().to_le_bytes()],
        bump = booking_days.bump,
    )]
    pub booking_days: Account<'info, BookingDays>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [ESCROW_PDA_SEED.as_bytes(), booking.key().as_ref()],
        token::mint = mint,
        token::authority = booking,
        token::token_program = token_program,
        bump = booking.escrow_bump
    )]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::authority = guest.authority,
        token::mint = mint,
        token::token_program = token_program
    )]
    pub guest_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = global_config.usdc_mint == mint.key() @ EscrowError::InvalidTokenMint
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_host_reject_booking(ctx: Context<HostRejectBooking>) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    booking.status = BookingStatus::Cancelled;

    let start_date = derive_date(booking.check_in);
    let end_date = derive_date(booking.check_out);
    let booking_days = &mut ctx.accounts.booking_days;
    release_days_single_year(booking_days, &start_date, &end_date)?;

    // Even though we won't move money from escrow account, I added an extra check
    require!(
        ctx.accounts.escrow_token_account.amount >= booking.total_price,
        EscrowError::InsufficientFunds
    );

    let mint = &ctx.accounts.mint;

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
                authority: booking.to_account_info(),
                from: ctx.accounts.escrow_token_account.to_account_info(),
                mint: mint.to_account_info(),
                to: ctx.accounts.guest_token_account.to_account_info(),
            },
            booking_seeds,
        ),
        booking.total_price,
        mint.decimals,
    )?;

    token_interface::close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        CloseAccount {
            account: ctx.accounts.escrow_token_account.to_account_info(),
            destination: ctx.accounts.payer.to_account_info(),
            authority: booking.to_account_info(),
        },
        booking_seeds,
    ))?;

    emit!(BookingStatusUpdated {
        status: BookingStatus::Cancelled,
        booking: booking.key()
    });
    Ok(())
}

#[derive(Accounts)]
pub struct HostRejectBookingCrossYear<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub host: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = host.key() == host_profile.authority @ EscrowError::UnauthorizedHost,
        constraint = !host_profile.banned @ EscrowError::UserBanned,
        constraint = host_profile.identity.is_some() @ EscrowError::UserNotVerified,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), guest.authority.as_ref()],
        bump = guest.bump,
        seeds::program = stayke_core::ID,
        constraint = booking.guest == guest.key() @ EscrowError::WrongGuestPassed
    )]
    pub guest: Box<Account<'info, UserProfile>>,

    #[account(
        mut,
        // TODO: check if money should go to host
        close = payer,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.host == host_profile.key() @ EscrowError::InvalidBookingProperty,
        constraint = booking.status == BookingStatus::Pending @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Box<Account<'info, Booking>>,

    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking.property.as_ref(), booking.check_in.year().to_le_bytes().as_ref()],
        bump = booking_days.bump,
    )]
    pub booking_days: Account<'info, BookingDays>,

    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking.property.as_ref(), (booking.check_in.year() + 1).to_le_bytes().as_ref()],
        bump = booking_days_next.bump,
    )]
    pub booking_days_next: Account<'info, BookingDays>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [ESCROW_PDA_SEED.as_bytes(), booking.key().as_ref()],
        token::mint = mint,
        token::authority = booking,
        token::token_program = token_program,
        bump = booking.escrow_bump
    )]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::authority = guest.authority,
        token::mint = mint,
        token::token_program = token_program
    )]
    pub guest_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = global_config.usdc_mint == mint.key() @ EscrowError::InvalidTokenMint
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_host_reject_booking_cross_year(
    ctx: Context<HostRejectBookingCrossYear>,
) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    booking.status = BookingStatus::Cancelled;

    let start_date = derive_date(booking.check_in);
    let end_date = derive_date(booking.check_out);

    let booking_days = &mut ctx.accounts.booking_days;
    let booking_days_next = &mut ctx.accounts.booking_days_next;
    release_days_cross_years(booking_days, booking_days_next, &start_date, &end_date)?;

    // Even though we won't move money from escrow account, I added an extra check
    require!(
        ctx.accounts.escrow_token_account.amount >= booking.total_price,
        EscrowError::InsufficientFunds
    );

    let mint = &ctx.accounts.mint;

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
                authority: booking.to_account_info(),
                from: ctx.accounts.escrow_token_account.to_account_info(),
                mint: mint.to_account_info(),
                to: ctx.accounts.guest_token_account.to_account_info(),
            },
            booking_seeds,
        ),
        booking.total_price,
        mint.decimals,
    )?;

    token_interface::close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        CloseAccount {
            account: ctx.accounts.escrow_token_account.to_account_info(),
            destination: ctx.accounts.payer.to_account_info(),
            authority: booking.to_account_info(),
        },
        booking_seeds,
    ))?;

    emit!(BookingStatusUpdated {
        status: BookingStatus::Cancelled,
        booking: booking.key()
    });

    Ok(())
}
