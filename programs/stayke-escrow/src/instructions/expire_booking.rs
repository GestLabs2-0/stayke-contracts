/// Expire booking holds instructions to end the booking without expecting the host or the guest to
/// finalize it
use anchor_lang::prelude::*;

use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};
use stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use stayke_core::USER_PROFILE_SEED;
use stayke_core::{self, UserProfile};

use crate::events::BookingExpired;
use crate::{
    constants::{BOOKING_DAYS_SEED, BOOKING_SEED, ESCROW_PDA_SEED},
    utils::{derive_date, release_days_cross_years, release_days_single_year, TimestampExt},
    Booking, BookingDays, BookingStatus, EscrowError,
};

#[derive(Accounts)]
pub struct ExpireBooking<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    // We don't require to check if user is banned or not, if someone pays and its booking is cancelled, then money is returned
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), guest.authority.as_ref()],
        bump = guest.bump,
        seeds::program = stayke_core::ID,
        constraint = booking.guest == guest.key() @ EscrowError::WrongGuestPassed
    )]
    pub guest: Box<Account<'info, UserProfile>>,

    #[account(
        mut,
        // Maybe I should return this money to client
        close = payer,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.status == BookingStatus::Pending @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Box<Account<'info, Booking>>,

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
        token::mint = mint,
        token::authority = guest.authority,
        token::token_program = token_program
    )]
    pub guest_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking.property.as_ref(), booking.check_in.year().to_le_bytes().as_ref()],
        bump = booking_days.bump
    )]
    pub booking_days: Account<'info, BookingDays>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        constraint = global_config.usdc_mint == mint.key() @ EscrowError::InvalidTokenMint
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_expire_booking(ctx: Context<ExpireBooking>) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    require!(
        booking.updated_at + (24 * 60 * 60) < Clock::get()?.unix_timestamp,
        EscrowError::NotOver24Hours
    );
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

    emit!(BookingExpired {
        booking: booking.key(),
        guest: booking.guest,
        host: booking.host
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ExpireBookingCrossDays<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    // We don't require to check if user is banned or not, if someone pays and its booking is cancelled, then money is returned
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), guest.authority.as_ref()],
        bump = guest.bump,
        seeds::program = stayke_core::ID,
        constraint = booking.guest == guest.key() @ EscrowError::WrongGuestPassed
    )]
    pub guest: Box<Account<'info, UserProfile>>,

    #[account(
        mut,
        // Maybe I should return this money to client or add a check for this close.
        close = payer,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.status == BookingStatus::Pending @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Box<Account<'info, Booking>>,

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
        token::mint = mint,
        token::authority = guest.authority,
        token::token_program = token_program
    )]
    pub guest_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking.property.as_ref(), booking.check_in.year().to_le_bytes().as_ref()],
        bump = booking_days.bump
    )]
    pub booking_days: Account<'info, BookingDays>,

    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking.property.as_ref(), (booking.check_in.year() + 1).to_le_bytes().as_ref()],
        bump = booking_days_next.bump
    )]
    pub booking_days_next: Account<'info, BookingDays>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        constraint = global_config.usdc_mint == mint.key() @ EscrowError::InvalidTokenMint
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_expire_booking_crossday(ctx: Context<ExpireBookingCrossDays>) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    require!(
        booking.updated_at + (24 * 60 * 60) < Clock::get()?.unix_timestamp,
        EscrowError::NotOver24Hours
    );
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

    emit!(BookingExpired {
        booking: booking.key(),
        guest: booking.guest,
        host: booking.host
    });

    Ok(())
}
