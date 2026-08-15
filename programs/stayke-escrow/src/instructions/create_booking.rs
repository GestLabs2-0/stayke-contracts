use anchor_lang::prelude::*;
use stayke_core::{
    constants::{LISTING_SEED, USER_PROFILE_SEED},
    Listing, UserProfile,
};

use stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::{BOOKING_DAYS_SEED, BOOKING_SEED},
    error::EscrowError,
    events::NewBookingEvent,
    state::{Booking, BookingDays, BookingStatus},
    utils::{derive_date, reserve_days_cross_years, reserve_days_single_year, TimestampExt},
};
// ---------------------------------------------------------------------------
// Create booking
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
        // Free tier: deposit check is bypassed while (completed + hosted) < free_ops.
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
        // Free tier: deposit check is bypassed while (completed + hosted) < free_ops.
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
}

pub fn handler_create_booking(
    ctx: Context<CreateBooking>,
    check_in: i64,
    check_out: i64,
) -> Result<()> {
    require!(check_in < check_out, EscrowError::InvalidBookingDates);
    require!(
        check_in > Clock::get()?.unix_timestamp,
        EscrowError::InvalidBookingDates
    );

    let start_date = derive_date(check_in);
    let end_date = derive_date(check_out);
    let days = ((check_out - check_in) / 86400) as u64;

    let property = &ctx.accounts.property;
    let property_key = property.key();
    let booking_days = &mut ctx.accounts.booking_days;

    reserve_days_single_year(booking_days, &start_date, &end_date)?;

    let booking = &mut ctx.accounts.booking;
    let client_profile = &ctx.accounts.client_profile;
    let host_profile = &ctx.accounts.host_profile;

    let status = BookingStatus::Pending;
    booking.set_inner(Booking {
        guest: client_profile.key(),
        host: host_profile.key(),
        property: property_key,
        status: status.clone(),
        total_price: property.price * days,
        is_deposit: false,
        check_in,
        check_out,
        bump: ctx.bumps.booking,
        escrow_bump: 0,
    });

    emit!(NewBookingEvent {
        guest: client_profile.key(),
        property: property_key,
        booking: booking.key(),
        host: host_profile.key(),
        check_in,
        check_out,
        status
    });

    Ok(())
}

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
        // Free tier: deposit check is bypassed while (completed + hosted) < free_ops.
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
        // Free tier: deposit check is bypassed while (completed + hosted) < free_ops.
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
}

pub fn handler_create_booking_cross_year(
    ctx: Context<CreateBookingCrossYear>,
    check_in: i64,
    check_out: i64,
) -> Result<()> {
    require!(check_in < check_out, EscrowError::InvalidBookingDates);
    require!(
        check_in > Clock::get()?.unix_timestamp,
        EscrowError::InvalidBookingDates
    );

    let start_date = derive_date(check_in);
    let end_date = derive_date(check_out);
    let days = ((check_out - check_in) / 86400) as u64;

    let property = &ctx.accounts.property;
    let property_key = property.key();
    let booking_days = &mut ctx.accounts.booking_days;
    let booking_days_next = &mut ctx.accounts.booking_days_next;

    reserve_days_cross_years(booking_days, booking_days_next, &start_date, &end_date)?;

    let booking = &mut ctx.accounts.booking;
    let client_profile = &ctx.accounts.client_profile;
    let host_profile = &ctx.accounts.host_profile;

    let status = BookingStatus::Pending;
    booking.set_inner(Booking {
        guest: client_profile.key(),
        host: host_profile.key(),
        property: property_key,
        status: status.clone(),
        total_price: property.price * days,
        is_deposit: false,
        check_in,
        check_out,
        bump: ctx.bumps.booking,
        escrow_bump: 0,
    });

    emit!(NewBookingEvent {
        guest: client_profile.key(),
        property: property_key,
        booking: booking.key(),
        host: host_profile.key(),
        check_in,
        check_out,
        status
    });

    Ok(())
}
