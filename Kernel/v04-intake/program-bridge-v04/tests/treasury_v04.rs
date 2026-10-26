use coordination_kernel::*;
use kernel_launch_v04::*;
use program_bridge_v04::treasury::*;
use program_bridge_v04::*;

fn tid(n: u8) -> TransitionId {
    let mut id = [0u8; 32];
    id[0] = n;
    id
}

fn committed(outcome: &ReplayOutcome) -> bool {
    matches!(outcome, ReplayOutcome::Committed { .. })
}

fn trace_of(outcome: &ReplayOutcome) -> &[&str] {
    match outcome {
        ReplayOutcome::Committed { trace, .. }
        | ReplayOutcome::Blocked { trace, .. }
        | ReplayOutcome::Refused { trace, .. } => trace,
    }
}

#[test]
fn t1_pay_tranche_commits_and_debits_the_tranche_stock_exactly() {
    let instr = TreasuryInstruction::PayTranche { amount: 100.0 };
    let (outcome, kernel) = instr.replay(tid(21)).expect("replay runs");
    assert!(committed(&outcome), "pay_tranche must commit: {outcome:?}");
    assert!(trace_contains_in_order(trace_of(&outcome), &ADDED_STEPS));
    assert_eq!(kernel.capacity_of(RES_TREASURY_TRANCHE), Some(amt(900.0)));
    // Other stocks untouched by a tranche payment.
    assert_eq!(kernel.capacity_of(RES_LENDING_BOOK), Some(amt(500.0)));
    assert_eq!(kernel.capacity_of(RES_EXIT_WINDOW), Some(amt(250.0)));
}

#[test]
fn t2_borrow_commits_and_debits_only_the_lending_book() {
    let instr = TreasuryInstruction::Borrow { amount: 100.0 };
    let (outcome, kernel) = instr.replay(tid(22)).expect("replay runs");
    assert!(committed(&outcome), "borrow must commit: {outcome:?}");
    assert_eq!(kernel.capacity_of(RES_LENDING_BOOK), Some(amt(400.0)));
    assert_eq!(kernel.capacity_of(RES_TREASURY_TRANCHE), Some(amt(1000.0)));
    assert_eq!(kernel.capacity_of(RES_EXIT_WINDOW), Some(amt(250.0)));
}

#[test]
fn t3_exit_commits_and_debits_only_the_exit_window_stock() {
    let instr = TreasuryInstruction::Exit { amount: 100.0 };
    let (outcome, kernel) = instr.replay(tid(23)).expect("replay runs");
    assert!(committed(&outcome), "exit must commit: {outcome:?}");
    assert_eq!(kernel.capacity_of(RES_EXIT_WINDOW), Some(amt(150.0)));
    assert_eq!(kernel.capacity_of(RES_TREASURY_TRANCHE), Some(amt(1000.0)));
    assert_eq!(kernel.capacity_of(RES_LENDING_BOOK), Some(amt(500.0)));
}

#[test]
fn t4_whale_tranche_at_the_stock_cap_commits_and_over_cap_blocks() {
    // At cap: the whole exit stock debits to zero — the largest single
    // order the period allows, and it still commits.
    let at_cap = TreasuryInstruction::ExecuteWhaleTranche { amount: 250.0 };
    let (outcome, kernel) = at_cap.replay(tid(24)).expect("replay runs");
    assert!(committed(&outcome), "at-cap tranche must commit: {outcome:?}");
    assert_eq!(kernel.capacity_of(RES_EXIT_WINDOW), Some(amt(0.0)));
    // Over cap: the D-10 hard bound in stock form — validate blocks and
    // nothing moves, exactly as slice 1's over-claim test.
    let over_cap = TreasuryInstruction::ExecuteWhaleTranche { amount: 251.0 };
    let (outcome, kernel) = over_cap.replay(tid(25)).expect("replay runs");
    assert!(
        matches!(outcome, ReplayOutcome::Blocked { .. }),
        "over-cap tranche must block: {outcome:?}"
    );
    assert_eq!(kernel.capacity_of(RES_EXIT_WINDOW), Some(amt(250.0)));
    assert_eq!(kernel.declared_state().index, 0);
}

#[test]
fn t5_reinvest_commits_and_moves_no_stock() {
    let instr = TreasuryInstruction::Reinvest;
    assert!(!instr.is_value_moving());
    assert_eq!(instr.stock(), None);
    let (outcome, kernel) = instr.replay(tid(26)).expect("replay runs");
    assert!(committed(&outcome), "reinvest must commit: {outcome:?}");
    assert_eq!(kernel.capacity_of(RES_TREASURY_TRANCHE), Some(amt(1000.0)));
    assert_eq!(kernel.capacity_of(RES_LENDING_BOOK), Some(amt(500.0)));
    assert_eq!(kernel.capacity_of(RES_EXIT_WINDOW), Some(amt(250.0)));
    // The head still advances: a claims-no-cash flow is a committed
    // transition, not a skipped one.
    assert_eq!(kernel.declared_state().index, 1);
}

#[test]
fn t6_settle_default_commits_on_the_pinned_pre_default_base() {
    let instr = TreasuryInstruction::SettleDefault { forfeited_base: ForfeitureBase::PreDefault };
    assert_eq!(instr.stock(), None);
    let (outcome, kernel) = instr.replay(tid(27)).expect("replay runs");
    assert!(committed(&outcome), "settle_default must commit: {outcome:?}");
    // Position-internal settlement: no kernel stock moves.
    assert_eq!(kernel.capacity_of(RES_TREASURY_TRANCHE), Some(amt(1000.0)));
    assert_eq!(kernel.capacity_of(RES_LENDING_BOOK), Some(amt(500.0)));
    assert_eq!(kernel.capacity_of(RES_EXIT_WINDOW), Some(amt(250.0)));
    // The record carries the D-3 pin, not a defaulted alternative.
    assert!(matches!(
        instr,
        TreasuryInstruction::SettleDefault { forfeited_base: ForfeitureBase::PreDefault }
    ));
}

#[test]
fn t7_collect_fees_commits_and_preserves_the_declared_total() {
    let instr = TreasuryInstruction::CollectFees { declared_total: 73.0 };
    assert_eq!(instr.stock(), None);
    let (outcome, kernel) = instr.replay(tid(28)).expect("replay runs");
    assert!(committed(&outcome), "collect_fees must commit: {outcome:?}");
    assert_eq!(kernel.capacity_of(RES_TREASURY_TRANCHE), Some(amt(1000.0)));
    assert_eq!(kernel.capacity_of(RES_LENDING_BOOK), Some(amt(500.0)));
    assert_eq!(kernel.capacity_of(RES_EXIT_WINDOW), Some(amt(250.0)));
    assert!(matches!(
        instr,
        TreasuryInstruction::CollectFees { declared_total } if declared_total == 73.0
    ));
}

#[test]
fn t8_open_window_is_the_authorizing_transition_not_a_debit() {
    let instr = TreasuryInstruction::OpenWindow { window_id: 4 };
    assert_eq!(instr.stock(), None);
    let (outcome, kernel) = instr.replay(tid(29)).expect("replay runs");
    assert!(committed(&outcome), "open_window must commit: {outcome:?}");
    assert_eq!(kernel.capacity_of(RES_TREASURY_TRANCHE), Some(amt(1000.0)));
    assert_eq!(kernel.declared_state().index, 1);
    assert!(matches!(instr, TreasuryInstruction::OpenWindow { window_id: 4 }));
}

#[test]
fn t9_every_value_moving_instruction_names_exactly_one_inventory_stock() {
    let moving = [
        TreasuryInstruction::PayTranche { amount: 1.0 },
        TreasuryInstruction::Borrow { amount: 1.0 },
        TreasuryInstruction::Exit { amount: 1.0 },
        TreasuryInstruction::ExecuteWhaleTranche { amount: 1.0 },
    ];
    for instr in moving {
        assert!(instr.is_value_moving());
        let stock = instr.stock().expect("value-moving names a stock");
        assert!(
            ACCOUNT_INVENTORY.iter().any(|e| e.stock == Some(stock)),
            "stock {stock:?} must appear in the §3 inventory"
        );
    }
    // The non-moving instructions name no stock at all — no silent debit.
    let still = [
        TreasuryInstruction::Reinvest,
        TreasuryInstruction::SettleDefault { forfeited_base: ForfeitureBase::PreDefault },
        TreasuryInstruction::CollectFees { declared_total: 0.0 },
        TreasuryInstruction::OpenWindow { window_id: 0 },
    ];
    for instr in still {
        assert!(!instr.is_value_moving());
        assert_eq!(instr.stock(), None);
    }
}

#[test]
fn t10_inventory_names_are_unique_and_its_stocks_are_distinct() {
    let mut names = std::collections::BTreeSet::new();
    for entry in ACCOUNT_INVENTORY {
        assert!(names.insert(entry.name), "duplicate inventory name: {}", entry.name);
        assert!(!entry.name.is_empty());
        assert!(!entry.transition_type.is_empty());
    }
    assert!(inventory_stocks_unique());
    // The three stocks the treasury instructions debit are all present.
    for stock in [RES_TREASURY_TRANCHE, RES_LENDING_BOOK, RES_EXIT_WINDOW] {
        assert!(ACCOUNT_INVENTORY.iter().any(|e| e.stock == Some(stock)));
    }
}

#[test]
fn t11_window_protocol_order_puts_payment_first_and_fees_last() {
    assert_eq!(protocol_position("open_window"), Some(0));
    let pay = protocol_position("pay_tranche").unwrap();
    let settle = protocol_position("settle_default").unwrap();
    let borrow = protocol_position("borrow").unwrap();
    let exit = protocol_position("exit").unwrap();
    let fees = protocol_position("collect_fees").unwrap();
    assert!(pay < settle && settle < borrow && borrow < exit && exit < fees);
    assert_eq!(fees, WINDOW_PROTOCOL_ORDER.len() - 1);
}
