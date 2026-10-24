//! W6.2 fifth pass — cross-window position ledger API tests.
//!
//! These test the ledger itself, not the classifier: the fail-closed rules
//! are the feature. A removal with no add history, or one exceeding the
//! recorded adds, must read Unknown — the ledger never guesses a full
//! close, because a position opened before the observation window is the
//! common case, not an edge case.

use tracer_s13::position_state::{CloseReading, PositionLedger, PositionStatus};

#[test]
fn ledger_full_close_after_exact_removal() {
    let mut ledger = PositionLedger::new();
    ledger.record_add("user1", "pool1", "SOL", 500);
    assert_eq!(ledger.status("user1", "pool1", "SOL"), PositionStatus::Open { remaining: 500 });
    ledger.record_remove("user1", "pool1", "SOL", 500);
    assert_eq!(ledger.status("user1", "pool1", "SOL"), PositionStatus::Closed);
    assert_eq!(ledger.close_reading("user1", "pool1", "SOL"), CloseReading::Full);
}

#[test]
fn ledger_partial_removal_stays_open_with_remaining() {
    let mut ledger = PositionLedger::new();
    ledger.record_add("user1", "pool1", "SOL", 1_000);
    ledger.record_remove("user1", "pool1", "SOL", 400);
    assert_eq!(ledger.status("user1", "pool1", "SOL"), PositionStatus::Open { remaining: 600 });
    assert_eq!(ledger.close_reading("user1", "pool1", "SOL"), CloseReading::Partial);
}

#[test]
fn ledger_removal_without_add_history_is_unknown_never_full() {
    let mut ledger = PositionLedger::new();
    ledger.record_remove("user1", "pool1", "SOL", 700);
    assert_eq!(ledger.status("user1", "pool1", "SOL"), PositionStatus::Unknown);
    assert_eq!(ledger.close_reading("user1", "pool1", "SOL"), CloseReading::Unknown);
    // A never-seen key is equally unknown — absence is not a close.
    assert_eq!(ledger.status("ghost", "pool1", "SOL"), PositionStatus::Unknown);
    assert_eq!(ledger.close_reading("ghost", "pool1", "SOL"), CloseReading::Unknown);
}

#[test]
fn ledger_over_removal_is_unknown_incomplete_history() {
    let mut ledger = PositionLedger::new();
    ledger.record_add("user1", "pool1", "SOL", 300);
    ledger.record_remove("user1", "pool1", "SOL", 301);
    assert_eq!(ledger.status("user1", "pool1", "SOL"), PositionStatus::Unknown);
    assert_eq!(ledger.close_reading("user1", "pool1", "SOL"), CloseReading::Unknown);
}

#[test]
fn ledger_zero_amount_legs_do_not_create_a_position() {
    let mut ledger = PositionLedger::new();
    ledger.record_add("user1", "pool1", "SOL", 0);
    ledger.record_remove("user1", "pool1", "SOL", 0);
    assert_eq!(ledger.status("user1", "pool1", "SOL"), PositionStatus::Unknown);
    // A real add after the zero legs still works — zeros were ignored,
    // not recorded as a zero position.
    ledger.record_add("user1", "pool1", "SOL", 100);
    assert_eq!(ledger.status("user1", "pool1", "SOL"), PositionStatus::Open { remaining: 100 });
}

#[test]
fn ledger_keys_are_independent_across_wallets_pools_mints() {
    let mut ledger = PositionLedger::new();
    ledger.record_add("user1", "pool1", "SOL", 100);
    ledger.record_add("user1", "pool1", "USDC", 200);
    ledger.record_add("user2", "pool1", "SOL", 300);
    ledger.record_remove("user1", "pool1", "SOL", 100);
    assert_eq!(ledger.status("user1", "pool1", "SOL"), PositionStatus::Closed);
    assert_eq!(ledger.status("user1", "pool1", "USDC"), PositionStatus::Open { remaining: 200 });
    assert_eq!(ledger.status("user2", "pool1", "SOL"), PositionStatus::Open { remaining: 300 });
    assert_eq!(ledger.status("user1", "pool2", "SOL"), PositionStatus::Unknown);
}

#[test]
fn ledger_multiple_adds_accumulate_before_removal() {
    let mut ledger = PositionLedger::new();
    ledger.record_add("user1", "pool1", "SOL", 100);
    ledger.record_add("user1", "pool1", "SOL", 150);
    ledger.record_remove("user1", "pool1", "SOL", 100);
    assert_eq!(ledger.status("user1", "pool1", "SOL"), PositionStatus::Open { remaining: 150 });
    ledger.record_remove("user1", "pool1", "SOL", 150);
    assert_eq!(ledger.status("user1", "pool1", "SOL"), PositionStatus::Closed);
}
