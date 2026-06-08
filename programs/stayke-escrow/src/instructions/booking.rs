use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, CloseAccount, Mint, TokenAccount, TokenInterface, TransferChecked},
};
use stayke_core::{
    constants::{LISTING_SEED, REPUTATION_PROFILE_SEED, USER_PROFILE_SEED},
    Listing, UserProfile,
};

use stayke_config::{error::StaykeConfigError, GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::{BOOKING_DAYS_SEED, BOOKING_SEED, ESCROW_CONFIG_SEED, ESCROW_PDA_SEED},
    error::EscrowError,
    events::{BookingStatusUpdated, NewBookingEvent},
    state::{Booking, BookingDays, BookingStatus},
    utils::{derive_date, TimestampExt},
};
use crate::{utils::DateComponents, EscrowConfig};

// ---------------------------------------------------------------------------
// Helpers: bitmap operations for day-occupancy tracking
// ---------------------------------------------------------------------------

pub fn months_days(month: u32) -> Result<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Ok(31),
        4 | 6 | 9 | 11 => Ok(30),
        2 => Ok(28), // Not accounting for leap years for simplicity
        _ => err!(EscrowError::InvalidMonth),
    }
}

pub fn bitmap_days(start_day: u32, end_day: u32) -> u32 {
    let mut mask: u32 = 0;
    for day in start_day..=end_day {
        mask |= 1 << (day - 1) as usize;
    }
    mask
}

// ---------------------------------------------------------------------------
// Create booking
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(check_in: i64)]
pub struct CreateBooking<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    /// The guest's UserProfile from stayke-core.
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), client.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = client_profile.bump,
        constraint = client.key() != host_profile.owner @ EscrowError::HostCannotBookOwnProperty,
        constraint = client.key() == client_profile.owner @ EscrowError::UnauthorizedBooking,
        constraint = !client_profile.banned @ EscrowError::UserBanned,
        constraint = client_profile.is_verified @ EscrowError::UserNotVerified,
        constraint = client_profile.deposited >= global_config.minimum_deposit @ EscrowError::InsufficientDeposit,
    )]
    pub client_profile: Account<'info, UserProfile>,

    /// The host's UserProfile from stayke-core.
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.owner.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = !host_profile.banned @ EscrowError::UserBanned,
        constraint = host_profile.is_verified @ EscrowError::UserNotVerified,
        constraint = host_profile.deposited >= global_config.minimum_deposit @ EscrowError::InsufficientDeposit,
        )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    #[account(
        init,
        payer = client,
        space = 8 + Booking::INIT_SPACE,
        seeds = [BOOKING_SEED.as_bytes(), property.key().as_ref(), client_profile.key().as_ref(), check_in.to_le_bytes().as_ref()],
        bump
    )]
    pub booking: Account<'info, Booking>,

    #[account(seeds = [LISTING_SEED.as_bytes(), property.owner.key().as_ref(), property.listing_id.to_le_bytes().as_ref()], seeds::program = stayke_core::ID, bump = property.bump, constraint = property.owner == host_profile.key() @ EscrowError::InvalidBookingProperty)]
    pub property: Account<'info, Listing>,

    #[account(seeds = [ESCROW_CONFIG_SEED.as_bytes()], bump = escrow_config.bump)]
    pub escrow_config: Box<Account<'info, EscrowConfig>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
        constraint = escrow_config.global_config == global_config.key() @ StaykeConfigError::InvalidGlobalConfig,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    pub system_program: Program<'info, System>,

    #[account(
        init_if_needed,
        payer = client,
        space = 8 + BookingDays::INIT_SPACE,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), property.key().as_ref(), check_in.year_month().to_le_bytes().as_ref()],
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

    reserve_days(
        ctx.remaining_accounts,
        property_key,
        booking_days,
        &start_date,
        &end_date,
    )?;

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
        days,
        review: 0,
        check_in_date: start_date,
        check_out_date: end_date,
        deposit: 0,
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

// ---------------------------------------------------------------------------
// Reserve / Release day-bitmap helpers
// ---------------------------------------------------------------------------

pub fn reserve_days<'a>(
    remaining_accounts: &'a [AccountInfo<'a>],
    property: Pubkey,
    booking_days: &mut Account<'_, BookingDays>,
    check_in: &DateComponents,
    check_out: &DateComponents,
) -> Result<()> {
    if booking_days.initialized {
        require!(
            booking_days.property == property,
            EscrowError::InvalidBookingDaysAccount
        );
    } else {
        booking_days.property = property;
        booking_days.month = check_in.month;
        booking_days.year = check_in.year;
        booking_days.occupied_days = 0;
        booking_days.initialized = true;
    }

    require!(
        booking_days.month == check_in.month,
        EscrowError::InvalidBookingDaysAccount
    );
    require!(
        booking_days.year == check_in.year,
        EscrowError::InvalidBookingDaysAccount
    );

    if booking_days.month == check_out.month && booking_days.year == check_out.year {
        let mask = bitmap_days(check_in.day, check_out.day);
        require!(
            booking_days.occupied_days & mask == 0,
            EscrowError::DatesAlreadyBooked
        );
        booking_days.occupied_days |= mask;
    } else {
        let end_day = months_days(booking_days.month)?;
        let mask = bitmap_days(check_in.day, end_day);
        require!(
            booking_days.occupied_days & mask == 0,
            EscrowError::DatesAlreadyBooked
        );
        booking_days.occupied_days |= mask;

        let years_to_reserve = if check_out.year > check_in.year {
            check_out.year.saturating_sub(check_in.year)
        } else {
            0
        };

        for (account_info, month) in remaining_accounts
            .iter()
            .zip(check_in.month + 1..=check_out.month + 12 * years_to_reserve)
        {
            let actual_month = month % 12;
            let mut bd = Account::<BookingDays>::try_from(account_info)?;
            if !bd.initialized {
                bd.property = property;
                bd.month = actual_month;
                let actual_year = check_in.year + month.div_euclid(12);
                bd.year = actual_year;
                bd.occupied_days = 0;
                bd.initialized = true;
            }
            require!(
                bd.property == property,
                EscrowError::InvalidBookingDaysAccount
            );
            require!(
                bd.month == actual_month,
                EscrowError::InvalidBookingDaysAccount
            );

            let end = if actual_month == check_out.month {
                check_out.day
            } else {
                months_days(actual_month)?
            };
            let mask = bitmap_days(1, end);
            require!(
                bd.occupied_days & mask == 0,
                EscrowError::DatesAlreadyBooked
            );
            bd.occupied_days |= mask;
            bd.exit(&crate::ID)?;
        }
    }

    Ok(())
}

pub fn release_days<'a>(
    remaining_accounts: &'a [AccountInfo<'a>],
    booking_property: Pubkey,
    booking_days: &mut Account<'_, BookingDays>,
    check_in: &DateComponents,
    check_out: &DateComponents,
) -> Result<()> {
    require!(
        booking_days.initialized,
        EscrowError::UninitializedBookingDays
    );
    require!(
        booking_days.month == check_in.month,
        EscrowError::InvalidBookingDaysAccount
    );
    require!(
        booking_days.year == check_in.year,
        EscrowError::InvalidBookingDaysAccount
    );

    if booking_days.month == check_out.month && booking_days.year == check_out.year {
        let mask = bitmap_days(check_in.day, check_out.day);
        require!(
            booking_days.occupied_days & mask == mask,
            EscrowError::DatesUnbooked
        );
        booking_days.occupied_days &= !mask;
    } else {
        let end_day = months_days(booking_days.month)?;
        let mask = bitmap_days(check_in.day, end_day);
        require!(
            booking_days.occupied_days & mask == mask,
            EscrowError::DatesUnbooked
        );
        booking_days.occupied_days &= !mask;

        let years_to_reserve = if check_out.year > check_in.year {
            check_out.year.saturating_sub(check_in.year)
        } else {
            0
        };

        for (account_info, month) in remaining_accounts
            .iter()
            .zip(check_in.month + 1..=check_out.month + 12 * years_to_reserve)
        {
            let mut bd = Account::<BookingDays>::try_from(account_info)?;
            require!(bd.initialized, EscrowError::UninitializedBookingDays);
            require!(
                bd.property == booking_property,
                EscrowError::InvalidBookingDaysAccount
            );
            let actual_month = month % 12;

            require!(
                bd.month == actual_month,
                EscrowError::InvalidBookingDaysAccount
            );

            let end = if actual_month == check_out.month {
                check_out.day
            } else {
                months_days(actual_month)?
            };
            let mask = bitmap_days(1, end);
            require!(bd.occupied_days & mask == mask, EscrowError::DatesUnbooked);
            bd.occupied_days &= !mask;
            bd.exit(&crate::ID)?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Host: accept pending booking
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct HostAcceptBooking<'info> {
    #[account(mut)]
    pub host: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = host.key() == host_profile.owner @ EscrowError::UnauthorizedHost,
        constraint = !host_profile.banned @ EscrowError::UserBanned,
        constraint = host_profile.is_verified @ EscrowError::UserNotVerified,
        constraint = host_profile.deposited >= global_config.minimum_deposit @ EscrowError::InsufficientDeposit,
    )]
    pub host_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.host == host_profile.key() @ EscrowError::InvalidBookingProperty,
        constraint = booking.status == BookingStatus::Pending @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Account<'info, Booking>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
        constraint = escrow_config.global_config == global_config.key() @ StaykeConfigError::InvalidGlobalConfig,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(seeds = [ESCROW_CONFIG_SEED.as_bytes()], bump = escrow_config.bump)]
    pub escrow_config: Box<Account<'info, EscrowConfig>>,
}

pub fn handler_host_accept_booking(ctx: Context<HostAcceptBooking>) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    booking.status = BookingStatus::HostAccepted;
    emit!(BookingStatusUpdated {
        status: BookingStatus::HostAccepted,
        booking: booking.key()
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Host: reject pending booking (releases days, closes booking account)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct HostRejectBooking<'info> {
    #[account(mut)]
    pub host: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = host.key() == host_profile.owner @ EscrowError::UnauthorizedHost,
        constraint = !host_profile.banned @ EscrowError::UserBanned,
        constraint = host_profile.is_verified @ EscrowError::UserNotVerified,
    )]
    pub host_profile: Account<'info, UserProfile>,

    /// CHECK: The guest wallet — receives the rent lamports from the closed booking account.
    #[account(mut, constraint = guest.key() == booking.guest @ EscrowError::WrongGuestPassed)]
    pub guest: UncheckedAccount<'info>,

    #[account(
        mut,
        close = guest,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.host == host_profile.key() @ EscrowError::InvalidBookingProperty,
        constraint = booking.status == BookingStatus::Pending @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Account<'info, Booking>,

    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking_days.property.as_ref(), booking_days.year_month().to_le_bytes().as_ref()],
        bump,
        constraint = booking_days.property == booking.property @ EscrowError::InvalidBookingDaysAccount,
    )]
    pub booking_days: Account<'info, BookingDays>,
}

pub fn handler_host_reject_booking(ctx: Context<HostRejectBooking>) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    booking.status = BookingStatus::Cancelled;
    release_days(
        ctx.remaining_accounts,
        booking.property,
        &mut ctx.accounts.booking_days,
        &booking.check_in_date.clone(),
        &booking.check_out_date.clone(),
    )?;
    emit!(BookingStatusUpdated {
        status: BookingStatus::Cancelled,
        booking: booking.key()
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Client: accept (confirm) reservation — locks USDC into escrow
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct ClientAcceptReserve<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), client.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = client_profile.bump,
        constraint = client.key() == client_profile.owner @ EscrowError::UnauthorizedBooking,
        constraint = !client_profile.banned @ EscrowError::UserBanned,
        constraint = client_profile.is_verified @ EscrowError::UserNotVerified,
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
    pub listing: Account<'info, Listing>,

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
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = client,
    )]
    pub client_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init,
        payer = client,
        token::mint = mint,
        token::authority = booking,
        seeds = [ESCROW_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump,
    )]
    pub escrow_token_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler_client_accept_reserve(ctx: Context<ClientAcceptReserve>) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    let listing = &mut ctx.accounts.listing;
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

    listing.is_occupied = Some(booking.guest);
    booking.status = BookingStatus::Active;
    booking.escrow_bump = ctx.bumps.escrow_token_account;

    emit!(BookingStatusUpdated {
        status: BookingStatus::Active,
        booking: booking.key()
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Client: reject reservation (closes booking, releases days)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct ClientRejectReserve<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), client.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = client_profile.bump,
        constraint = client.key() == client_profile.owner @ EscrowError::UnauthorizedBooking,
        constraint = !client_profile.banned @ EscrowError::UserBanned,
        constraint = client_profile.is_verified @ EscrowError::UserNotVerified,
    )]
    pub client_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        close = client,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = (booking.status == BookingStatus::HostAccepted || booking.status == BookingStatus::Pending) @ EscrowError::InvalidBookingStatus,
        constraint = booking.guest == client_profile.key() @ EscrowError::UnauthorizedBooking,
    )]
    pub booking: Account<'info, Booking>,

    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking.property.as_ref(), booking_days.year_month().to_le_bytes().as_ref()],
        bump,
    )]
    pub booking_days: Account<'info, BookingDays>,
}

pub fn handler_client_reject_reserve(ctx: Context<ClientRejectReserve>) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    booking.status = BookingStatus::Cancelled;
    release_days(
        ctx.remaining_accounts,
        booking.property,
        &mut ctx.accounts.booking_days,
        &booking.check_in_date.clone(),
        &booking.check_out_date.clone(),
    )?;
    emit!(BookingStatusUpdated {
        status: BookingStatus::Cancelled,
        booking: booking.key()
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Complete stay — distributes escrow to host (minus fee) and platform vault
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct CompleteStay<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), client.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = client_profile.bump,
        constraint = client.key() == client_profile.owner @ EscrowError::UnauthorizedBooking,
        constraint = !client_profile.banned @ EscrowError::UserBanned,
    )]
    pub client_profile: Account<'info, UserProfile>,

    /// The host's UserProfile — destination for the payment.
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.owner.key().as_ref()],
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
        constraint = booking.status == BookingStatus::ReviewCompleted @ EscrowError::BookingNotReviewCompleted,
    )]
    pub booking: Box<Account<'info, Booking>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
        constraint = escrow_config.global_config == global_config.key() @ StaykeConfigError::InvalidGlobalConfig,
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

    /// The host's USDC token account.
    #[account(mut)]
    pub host_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Platform fee vault.
    #[account(
        mut,
        constraint = platform_vault.key() == global_config.platform_vault @ StaykeConfigError::InvalidVaultAccount,
    )]
    pub platform_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, constraint = mint.key() == global_config.usdc_mint @ StaykeConfigError::InvalidTokenMint)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

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

    Ok(())
}

// ---------------------------------------------------------------------------
// Close booking — leaves review, updates profile counters (via CPI to stayke-core)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct CloseBooking<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), client.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = client_profile.bump,
        constraint = client.key() == client_profile.owner @ EscrowError::UnauthorizedBooking,
        constraint = !client_profile.banned @ EscrowError::UserBanned,
    )]
    pub client_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.owner.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
    )]
    pub host_reputation: Account<'info, stayke_core::ReputationProfile>,

    /// Host's ReputationProfile from stayke-core (receives score update).
    #[account(
        seeds = [REPUTATION_PROFILE_SEED.as_bytes(), host_profile.owner.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump,
    )]
    pub host_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.guest == client_profile.key() @ EscrowError::UnauthorizedBooking,
        constraint = booking.status == BookingStatus::Active @ EscrowError::BookingNotActive,
    )]
    pub booking: Account<'info, Booking>,
}

pub fn handler_review_completed(ctx: Context<CloseBooking>, score: u8) -> Result<()> {
    require!((1..=5).contains(&score), EscrowError::InvalidScore);

    let booking = &mut ctx.accounts.booking;
    booking.status = BookingStatus::ReviewCompleted;

    // Update reputation counters directly (no CPI needed — we hold the account).
    let reputation = &mut ctx.accounts.host_reputation;
    reputation.host_reviews += 1;
    reputation.total_score_host += score as u64;
    reputation.hosted_stays += 1;

    emit!(BookingStatusUpdated {
        status: BookingStatus::ReviewCompleted,
        booking: booking.key(),
    });

    Ok(())
}
