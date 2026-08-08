//! Constraint / ownership regression tests for PR3–PR4 dispute+escrow hardening.

#[test]
fn open_dispute_auth_and_guilty_source_contract() {
    let src = include_str!("../src/instructions/open_dispute.rs");
    assert!(
        src.contains("initiator_profile.key()"),
        "open_dispute must auth via initiator_profile PDA key"
    );
    assert!(
        src.contains("guilty_counterparty"),
        "open_dispute must set guilty via counterparty helper"
    );
    assert!(
        !src.contains("initiator.key() == booking.guest"),
        "must not auth solely via wallet vs booking party"
    );
}

#[test]
fn resolve_dispute_typed_global_config_mint_vault_constraints() {
    let src = include_str!("../src/instructions/resolve_dispute.rs");
    assert!(src.contains("global_config: Box<Account<'info, GlobalConfig>>"));
    assert!(src.contains("InvalidTokenMint"));
    assert!(src.contains("InvalidVaultAccount"));
    assert!(src.contains("platform_vault"));
    assert!(src.contains("usdc_mint.key() == global_config.usdc_mint"));
}

#[test]
fn penalize_user_typed_treasury_link_and_mint_constraints() {
    let src = include_str!("../src/instructions/penalize_user.rs");
    assert!(src.contains("treasury_config: Box<Account<'info, TreasuryConfig>>"));
    assert!(src.contains("UnlinkedTreasuryConfig"));
    assert!(src.contains("InvalidTokenMint"));
    assert!(src.contains("treasury_config.global_config == global_config.key()"));
    assert!(src.contains("UnauthorizedAdmin"));
}

#[test]
fn docs_and_booking_status_allowlist_untouched_marker() {
    // Scope lock: this change must not implement cpi_update_booking_status allowlist
    // or edit docs/**. The allowlist for `cpi_update_booking_status` is handled in a separate change.
    let open = include_str!("../src/instructions/open_dispute.rs");
    assert!(
        open.contains("cpi_update_booking_status"),
        "open_dispute still CPIs booking status (allowlist hardening is out of scope)"
    );
}
