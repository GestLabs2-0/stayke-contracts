use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};
use stayke_core::{
    constants::{LISTING_SEED, USER_PROFILE_SEED},
    Listing, UserProfile,
};

use stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::{BOOKING_DAYS_SEED, BOOKING_SEED, ESCROW_PDA_SEED},
    error::EscrowError,
    events::NewBookingEvent,
    state::{Booking, BookingDays, BookingStatus},
    utils::{derive_date, reserve_days_cross_years, reserve_days_single_year, TimestampExt},
};

/// Number of seconds in a calendar day (used to convert a stay duration into nights).
const SECONDS_PER_DAY: i64 = 86_400;

// ---------------------------------------------------------------------------
// Create booking — entry point to the booking flow.
//
// The guest funds the full stay price into the per-booking escrow token
// account, creates the `Booking` in `Pending`, blocks the reserved days in the
// property's availability bitmap, and records `updated_at` as the start of the
// 24 h host-response timer.
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(check_in: i64)]
pub struct CreateBooking<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub client: Signer<'info>,

    /// The guest's UserProfile from stayke-core.
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), client.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = client_profile.bump,
        constraint = client.key() != host_profile.authority @ EscrowError::HostCannotBookOwnProperty,
        constraint = client.key() == client_profile.authority @ EscrowError::UnauthorizedBooking,
        constraint = !client_profile.banned @ EscrowError::UserBanned,
        constraint = client_profile.identity.is_some() @ EscrowError::UserNotVerified,
        constraint = client_profile.active_booking.is_none() @ EscrowError::ActiveBookingExists,
        // Treasury guarantee: past `free_ops`, the guest must hold a deposit to
        // cover potential disputes. Bypassed while under the free tier.
        constraint = (client_profile.completed_stays + client_profile.hosted_stays) < global_config.free_ops as u32
            || client_profile.deposited >= global_config.minimum_deposit @ EscrowError::InsufficientDeposit,
    )]
    pub client_profile: Account<'info, UserProfile>,

    /// The host's UserProfile from stayke-core.
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = !host_profile.banned @ EscrowError::UserBanned,
        constraint = host_profile.identity.is_some() @ EscrowError::HostNotVerified,
        // Same treasury-guarantee gate, applied up-front so a host is never
        // asked to accept a booking it could not honour anyway.
        constraint = (host_profile.completed_stays + host_profile.hosted_stays) < global_config.free_ops as u32
            || host_profile.deposited >= global_config.minimum_deposit @ EscrowError::InsufficientDeposit,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    #[account(
        init,
        payer = payer,
        space = 8 + Booking::INIT_SPACE,
        seeds = [BOOKING_SEED.as_bytes(), property.key().as_ref(), client_profile.key().as_ref(), check_in.to_le_bytes().as_ref()],
        bump
    )]
    pub booking: Account<'info, Booking>,

    #[account(seeds = [LISTING_SEED.as_bytes(), host_profile.key().as_ref(), property.listing_id.to_le_bytes().as_ref()], seeds::program = stayke_core::ID, bump = property.bump)]
    pub property: Account<'info, Listing>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    pub system_program: Program<'info, System>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + BookingDays::INIT_SPACE,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), property.key().as_ref(), check_in.year().to_le_bytes().as_ref()],
        bump,
    )]
    pub booking_days: Account<'info, BookingDays>,

    /// Per-booking escrow token account (Token2022), owned by the booking PDA.
    #[account(
        init,
        payer = payer,
        seeds = [ESCROW_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = booking,
        token::token_program = token_program,
    )]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The guest's USDC token account funding the escrow.
    #[account(
        mut,
        token::mint = mint,
        token::authority = client,
        token::token_program = token_program,
    )]
    pub client_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = mint.key() == global_config.usdc_mint @ EscrowError::InvalidTokenMint,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_create_booking(
    ctx: Context<CreateBooking>,
    check_in: i64,
    check_out: i64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;

    require!(check_in < check_out, EscrowError::InvalidBookingDates);
    require!(check_in >= now, EscrowError::InvalidBookingDates);

    let start_date = derive_date(check_in);
    let end_date = derive_date(check_out);
    let days = ((check_out - check_in) / SECONDS_PER_DAY) as u64;

    let property = &ctx.accounts.property;
    let property_key = property.key();
    let total_price = property
        .price
        .checked_mul(days)
        .ok_or(EscrowError::PriceOverflow)?;

    // Block the requested days in the availability bitmap.
    reserve_days_single_year(&mut ctx.accounts.booking_days, &start_date, &end_date)?;

    // The guest must hold enough funds to cover the full stay before the transfer.
    require!(
        ctx.accounts.client_token_account.amount >= total_price,
        EscrowError::InsufficientFunds
    );

    // Fund the escrow with the full stay price.
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.client_token_account.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.escrow_token_account.to_account_info(),
                authority: ctx.accounts.client.to_account_info(),
            },
        ),
        total_price,
        ctx.accounts.mint.decimals,
    )?;

    let booking = &mut ctx.accounts.booking;
    let client_profile = &ctx.accounts.client_profile;
    let host_profile = &ctx.accounts.host_profile;

    booking.set_inner(Booking {
        guest: client_profile.key(),
        host: host_profile.key(),
        property: property_key,
        status: BookingStatus::Pending,
        total_price,
        host_review: 0,
        guest_review: 0,
        check_in,
        check_out,
        escrow_bump: ctx.bumps.escrow_token_account,
        updated_at: now,
        bump: ctx.bumps.booking,
    });

    emit!(NewBookingEvent {
        guest: client_profile.key(),
        booking: booking.key(),
        property: property_key,
        host: host_profile.key(),
        status: BookingStatus::Pending,
        check_in,
        check_out,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Create booking (cross-year) — same flow, spanning two `BookingDays` accounts
// for bookings that start late in one year and end early in the next.
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(check_in: i64)]
pub struct CreateBookingCrossYear<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub client: Signer<'info>,

    /// The guest's UserProfile from stayke-core.
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), client.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = client_profile.bump,
        constraint = client.key() != host_profile.authority @ EscrowError::HostCannotBookOwnProperty,
        constraint = client.key() == client_profile.authority @ EscrowError::UnauthorizedBooking,
        constraint = !client_profile.banned @ EscrowError::UserBanned,
        constraint = client_profile.identity.is_some() @ EscrowError::UserNotVerified,
        constraint = client_profile.active_booking.is_none() @ EscrowError::ActiveBookingExists,
        constraint = (client_profile.completed_stays + client_profile.hosted_stays) < global_config.free_ops as u32
            || client_profile.deposited >= global_config.minimum_deposit @ EscrowError::InsufficientDeposit,
    )]
    pub client_profile: Account<'info, UserProfile>,

    /// The host's UserProfile from stayke-core.
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = !host_profile.banned @ EscrowError::UserBanned,
        constraint = host_profile.identity.is_some() @ EscrowError::HostNotVerified,
        constraint = (host_profile.completed_stays + host_profile.hosted_stays) < global_config.free_ops as u32
            || host_profile.deposited >= global_config.minimum_deposit @ EscrowError::InsufficientDeposit,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    #[account(
        init,
        payer = payer,
        space = 8 + Booking::INIT_SPACE,
        seeds = [BOOKING_SEED.as_bytes(), property.key().as_ref(), client_profile.key().as_ref(), check_in.to_le_bytes().as_ref()],
        bump
    )]
    pub booking: Account<'info, Booking>,

    #[account(seeds = [LISTING_SEED.as_bytes(), host_profile.key().as_ref(), property.listing_id.to_le_bytes().as_ref()], seeds::program = stayke_core::ID, bump = property.bump)]
    pub property: Account<'info, Listing>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    pub system_program: Program<'info, System>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + BookingDays::INIT_SPACE,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), property.key().as_ref(), check_in.year().to_le_bytes().as_ref()],
        bump,
    )]
    pub booking_days: Account<'info, BookingDays>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + BookingDays::INIT_SPACE,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), property.key().as_ref(), (check_in.year() + 1).to_le_bytes().as_ref()],
        bump,
    )]
    pub booking_days_next: Account<'info, BookingDays>,

    /// Per-booking escrow token account (Token2022), owned by the booking PDA.
    #[account(
        init,
        payer = payer,
        seeds = [ESCROW_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = booking,
        token::token_program = token_program,
    )]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The guest's USDC token account funding the escrow.
    #[account(
        mut,
        token::mint = mint,
        token::authority = client,
        token::token_program = token_program,
    )]
    pub client_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = mint.key() == global_config.usdc_mint @ EscrowError::InvalidTokenMint,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_create_booking_cross_year(
    ctx: Context<CreateBookingCrossYear>,
    check_in: i64,
    check_out: i64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;

    require!(check_in < check_out, EscrowError::InvalidBookingDates);
    require!(check_in >= now, EscrowError::InvalidBookingDates);

    let start_date = derive_date(check_in);
    let end_date = derive_date(check_out);
    let days = ((check_out - check_in) / SECONDS_PER_DAY) as u64;

    let property = &ctx.accounts.property;
    let property_key = property.key();
    let total_price = property
        .price
        .checked_mul(days)
        .ok_or(EscrowError::PriceOverflow)?;

    // Block the requested days across both calendar years.
    reserve_days_cross_years(
        &mut ctx.accounts.booking_days,
        &mut ctx.accounts.booking_days_next,
        &start_date,
        &end_date,
    )?;

    require!(
        ctx.accounts.client_token_account.amount >= total_price,
        EscrowError::InsufficientFunds
    );

    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.client_token_account.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.escrow_token_account.to_account_info(),
                authority: ctx.accounts.client.to_account_info(),
            },
        ),
        total_price,
        ctx.accounts.mint.decimals,
    )?;

    let booking = &mut ctx.accounts.booking;
    let client_profile = &ctx.accounts.client_profile;
    let host_profile = &ctx.accounts.host_profile;

    booking.set_inner(Booking {
        guest: client_profile.key(),
        host: host_profile.key(),
        property: property_key,
        status: BookingStatus::Pending,
        total_price,
        host_review: 0,
        guest_review: 0,
        check_in,
        check_out,
        escrow_bump: ctx.bumps.escrow_token_account,
        updated_at: now,
        bump: ctx.bumps.booking,
    });

    emit!(NewBookingEvent {
        guest: client_profile.key(),
        booking: booking.key(),
        property: property_key,
        host: host_profile.key(),
        status: BookingStatus::Pending,
        check_in,
        check_out,
    });

    Ok(())
}
