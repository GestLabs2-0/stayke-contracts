use anchor_lang::prelude::*;
use stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use stayke_core::{state::UserProfile, USER_PROFILE_SEED};

use stayke_escrow::{
    cpi::{accounts::UpdateBookingStatusCpi, cpi_update_booking_status},
    program::StaykeEscrow,
    state::Booking,
    BookingStatus,
};

use crate::{
    constants::{CPI_AUTHORITY_SEED, DISPUTE_PDA_SEED},
    error::DisputeError,
    events::DisputeOpened,
    state::{Dispute, DisputeReason, DisputeStatus},
};

#[derive(Accounts)]
pub struct OpenDispute<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub initiator: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), initiator.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = initiator_profile.bump,
        constraint = !initiator_profile.banned @ DisputeError::UserBanned,
        constraint = initiator_profile.identity.is_some() @ DisputeError::UserNotVerified,
    )]
    pub initiator_profile: Account<'info, UserProfile>,

    #[account(mut)]
    pub booking: Box<Account<'info, Booking>>,

    #[account(
        init,
        payer = payer,
        space = 8 + Dispute::INIT_SPACE,
        seeds = [DISPUTE_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump,
    )]
    pub dispute: Box<Account<'info, Dispute>>,

    /// CHECK: CPI authority PDA of an allowlisted Stayke program.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_config::ID,
        bump = global_config.bump,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    pub stayke_escrow_program: Program<'info, StaykeEscrow>,
    pub system_program: Program<'info, System>,
}

pub fn handler_open_dispute(ctx: Context<OpenDispute>, reason: DisputeReason) -> Result<()> {
    let initiator_profile_key = ctx.accounts.initiator_profile.key();
    require!(
        ctx.accounts.booking.guest == initiator_profile_key
            || ctx.accounts.booking.host == initiator_profile_key,
        DisputeError::UnauthorizedDisputeInitiator
    );
    require!(
        ctx.accounts.booking.status == BookingStatus::Active,
        DisputeError::BookingNotActive
    );

    let booking = &ctx.accounts.booking;

    // Guilty at open = accused counterparty (non-initiator).
    let guilty = guilty_counterparty(initiator_profile_key, booking.guest, booking.host)?;

    let dispute = &mut ctx.accounts.dispute;
    dispute.booking = ctx.accounts.booking.key();
    dispute.property = ctx.accounts.booking.property;
    dispute.initiator = initiator_profile_key;
    dispute.guilty = guilty;
    dispute.reason = reason.clone();
    dispute.status = DisputeStatus::Open;
    dispute.created_at = Clock::get()?.unix_timestamp;
    dispute.resolved_at = None;
    dispute.bump = ctx.bumps.dispute;

    let bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];

    let cpi_accounts = UpdateBookingStatusCpi {
        booking: ctx.accounts.booking.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
    };
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.stayke_escrow_program.key(),
        cpi_accounts,
        signer_seeds,
    );
    cpi_update_booking_status(cpi_ctx, BookingStatus::Disputed)?;

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

/// Accused counterparty PDA at open: guest opens → host; host opens → guest.
pub(crate) fn guilty_counterparty(
    initiator_profile: Pubkey,
    guest: Pubkey,
    host: Pubkey,
) -> Result<Pubkey> {
    require!(
        initiator_profile == guest || initiator_profile == host,
        DisputeError::UnauthorizedDisputeInitiator
    );
    Ok(if initiator_profile == guest {
        host
    } else {
        guest
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // guilty_counterparty
    // -----------------------------------------------------------------------

    #[test]
    fn guest_open_sets_guilty_to_host() {
        let guest = Pubkey::new_unique();
        let host = Pubkey::new_unique();
        assert_eq!(guilty_counterparty(guest, guest, host).unwrap(), host);
    }

    #[test]
    fn host_open_sets_guilty_to_guest() {
        let guest = Pubkey::new_unique();
        let host = Pubkey::new_unique();
        assert_eq!(guilty_counterparty(host, guest, host).unwrap(), guest);
    }

    #[test]
    fn non_party_open_fails() {
        let guest = Pubkey::new_unique();
        let host = Pubkey::new_unique();
        let stranger = Pubkey::new_unique();
        assert!(guilty_counterparty(stranger, guest, host).is_err());
    }

    #[test]
    fn non_party_open_zero_pubkey_fails() {
        let guest = Pubkey::new_unique();
        let host = Pubkey::new_unique();
        assert!(guilty_counterparty(Pubkey::default(), guest, host).is_err());
    }

    #[test]
    fn same_guest_and_host_initiator_is_guest_returns_host() {
        let party = Pubkey::new_unique();
        // Both guest and host are the same key (blocked in prod by
        // HostCannotBookOwnProperty — test that the pure function still
        // returns the counterparty consistently).
        let guilty = guilty_counterparty(party, party, party).unwrap();
        assert_eq!(guilty, party);
    }

    #[test]
    fn guest_open_with_zero_host_still_returns_host() {
        let guest = Pubkey::new_unique();
        let host = Pubkey::default();
        assert_eq!(guilty_counterparty(guest, guest, host).unwrap(), host);
    }

    #[test]
    fn host_open_with_zero_guest_still_returns_guest() {
        let guest = Pubkey::default();
        let host = Pubkey::new_unique();
        assert_eq!(guilty_counterparty(host, guest, host).unwrap(), guest);
    }

    #[test]
    fn guilty_counterparty_returns_unauthorized_error_code() {
        let err = guilty_counterparty(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            DisputeError::UnauthorizedDisputeInitiator.into()
        );
    }

    // -----------------------------------------------------------------------
    // DisputeReason — discriminant integrity
    // -----------------------------------------------------------------------

    #[test]
    fn dispute_reason_other_serializes_roundtrip() {
        let reason = DisputeReason::Other;
        let mut buf = Vec::new();
        reason.serialize(&mut buf).unwrap();
        let restored = DisputeReason::deserialize(&mut &buf[..]).unwrap();
        assert!(matches!(restored, DisputeReason::Other));
    }

    #[test]
    fn all_dispute_reasons_have_distinct_discriminants() {
        let reasons: [DisputeReason; 5] = [
            DisputeReason::PropertyNotAsDescribed,
            DisputeReason::HostUnreachable,
            DisputeReason::GuestDamagedProperty,
            DisputeReason::GuestBrokeRules,
            DisputeReason::Other,
        ];
        let mut disc_bytes = Vec::new();
        for r in &reasons {
            let mut buf = Vec::new();
            r.serialize(&mut buf).unwrap();
            // First byte is the variant discriminant for simple enums.
            let disc = buf[0];
            assert!(
                !disc_bytes.contains(&disc),
                "duplicate discriminant byte {disc}"
            );
            disc_bytes.push(disc);
        }
        assert_eq!(disc_bytes.len(), 5);
    }

    // -----------------------------------------------------------------------
    // DisputeStatus — round-trip integrity
    // -----------------------------------------------------------------------

    #[test]
    fn dispute_status_serializes_roundtrip() {
        for status in [
            DisputeStatus::Open,
            DisputeStatus::Resolved,
            DisputeStatus::Rejected,
        ] {
            let mut buf = Vec::new();
            status.serialize(&mut buf).unwrap();
            let restored = DisputeStatus::deserialize(&mut &buf[..]).unwrap();
            // restored should match the original status
            assert!(
                std::mem::discriminant(&restored) == std::mem::discriminant(&status)
            );
        }
    }
}
