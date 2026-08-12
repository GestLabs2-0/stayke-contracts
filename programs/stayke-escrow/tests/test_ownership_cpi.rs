//! Escrow ownership CPI regressions (occupied + review via core CPI).

#[test]
fn accept_reserve_sets_occupied_via_core_cpi_not_direct_write() {
    let src = include_str!("../src/instructions/client_accept_reserve.rs");
    assert!(
        src.contains("set_listing_occupied"),
        "accept-reserve must CPI set_listing_occupied"
    );
    assert!(
        !src.contains("listing.is_occupied = true"),
        "accept-reserve must not write listing.is_occupied directly"
    );
    assert!(
        src.contains("CPI_AUTHORITY_SEED"),
        "accept-reserve must sign with escrow cpi_authority PDA"
    );
}

#[test]
fn close_booking_reviews_via_core_cpi_with_reputation_seeds() {
    let src = include_str!("../src/instructions/close_booking.rs");
    assert!(
        src.contains("update_host_review"),
        "close_booking must CPI update_host_review"
    );
    assert!(
        src.contains("REPUTATION_PROFILE_SEED"),
        "review accounts must use reputation profile seeds"
    );
    assert!(
        !src.contains("host_reputation.total_score"),
        "escrow must not directly mutate reputation fields"
    );
}
