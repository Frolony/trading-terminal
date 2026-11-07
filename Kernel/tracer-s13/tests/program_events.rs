//! W3.3 Phase 5 (S13 dogfood) — own-program event adapter tests.
//!
//! The extractor (`hidden_files/treasury-test-2026-11-06/extract-events.py`)
//! decodes the deployed program's 37 IDL events from devnet transaction
//! logs; this file pins the Rust side of the dogfood: each decoded event
//! maps to the value movement it records, or to nothing when it moves no
//! value. Field values below are taken from the live test's real
//! transactions where a figure is quoted (the 38-holder dividend payout,
//! the penalty probe, the swarm loans); the rest are the same shapes with
//! illustrative pubkeys.
//!
//! ## Ground truth quoted here
//!
//! Live test 2026-11-06 ~12:18 CST: 38/38 holders paid — 36 x 15,650,000,
//! Whale B 37,560,000, Whale A 187,800,000, totalling 788,760,000 lamports
//! in full across the 12-window rotation (run log
//! `hidden_files/treasury-live-test-run-2026-11-06.md`).
//!
//! Penalty probe, same run: 10,000,000 gross -> 8,700,000 out, 1,300,000
//! flat-13% retained in the vault.
//!
//! Swarm probe slice 1 (2026-11-06 17:06): first live loan outstanding
//! 1,000,000 lamports; Phase 2 (17:29): second loan 1,000,000 via
//! borrow_reinvest (book 2,000,000 / active 2).

use tracer_s13::ingest::{
    ingest, program_event_to_transfer, FieldValue, ProgramEvent,
    FEE_VAULT_LABEL, TREASURY_VAULT_LABEL,
};
use tracer_s13::reconstruct::{reconstruct, NetEffect};

const PROGRAM: &str = "CAQ4FvxtCy2d3852eyA1Hk4Qhwy9MpE14igTQkx5NSeJ";
const VAULT: &str = "FdGMFKZcnRbxX1Nu5F9PBZeGaD4TR9djsvPHGkw79TJn";
const OPERATOR: &str = "swarm-operator";
const HOLDER: &str = "swarm-holder-1";
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

fn ev<'a>(
    sig: &'a str,
    slot: u64,
    index: u32,
    name: &'a str,
    fields: &'a [(&'a str, FieldValue<'a>)],
) -> ProgramEvent<'a> {
    ProgramEvent {
        signature: sig,
        slot,
        index,
        timestamp_ms: 1_793_985_000_000 + slot,
        fee_lamports: 5000,
        name,
        fields,
    }
}

/// One value-moving event per row: (name, fields, from, to, mint, amount,
/// instruction). The `from`/`to` are the adapter's contract, including the
/// vault-label convention for payout events.
type ValueCase = (
    &'static str,
    Vec<(&'static str, FieldValue<'static>)>,
    &'static str,
    &'static str,
    &'static str,
    u64,
    &'static str,
);

fn value_cases() -> Vec<ValueCase> {
    vec![
        (
            "TreasurySolDeposit",
            vec![
                ("depositor", FieldValue::Pubkey("live-payer")),
                ("vault", FieldValue::Pubkey(VAULT)),
                ("amount_raw", FieldValue::U64(150_000_000)),
                ("total_raw", FieldValue::U64(12_600_000_000)),
                ("cash_raw", FieldValue::U64(12_600_000_000)),
            ],
            "live-payer",
            VAULT,
            "SOL",
            150_000_000,
            "deposit_sol",
        ),
        (
            "TreasuryDeposit",
            vec![
                ("depositor", FieldValue::Pubkey("live-payer")),
                ("mint", FieldValue::Pubkey(USDC_MINT)),
                ("vault", FieldValue::Pubkey("usdc-vault")),
                ("amount_raw", FieldValue::U64(1_000_000)),
                ("total_raw", FieldValue::U64(1_000_000)),
                ("cash_raw", FieldValue::U64(1_000_000)),
            ],
            "live-payer",
            "usdc-vault",
            USDC_MINT,
            1_000_000,
            "deposit",
        ),
        (
            // Live test: one of the 36 x 15,650,000 dividend tranches.
            "TranchePaid",
            vec![
                ("window_id", FieldValue::U64(3)),
                ("holder", FieldValue::Pubkey(HOLDER)),
                ("amount_raw", FieldValue::U64(15_650_000)),
                ("transition_id", FieldValue::U64(4)),
                ("commit_index", FieldValue::U64(3)),
                ("declared_hash", FieldValue::Hash("00")),
            ],
            TREASURY_VAULT_LABEL,
            HOLDER,
            "SOL",
            15_650_000,
            "pay_tranche",
        ),
        (
            // Penalty probe: 10,000,000 gross -> 8,700,000 out; the
            // 1,300,000 stays in the vault and must NOT appear as movement.
            "Exited",
            vec![
                ("window_id", FieldValue::U64(5)),
                ("holder", FieldValue::Pubkey(HOLDER)),
                ("amount_raw", FieldValue::U64(10_000_000)),
                ("penalty_raw", FieldValue::U64(1_300_000)),
                ("paid_raw", FieldValue::U64(8_700_000)),
                ("position_after_raw", FieldValue::U64(40_000_000)),
                ("transition_id", FieldValue::U64(9)),
                ("commit_index", FieldValue::U64(8)),
                ("declared_hash", FieldValue::Hash("00")),
            ],
            TREASURY_VAULT_LABEL,
            HOLDER,
            "SOL",
            8_700_000,
            "exit",
        ),
        (
            // Swarm slice 1: the first live loan, 1,000,000 lamports.
            "LoanIssued",
            vec![
                ("window_id", FieldValue::U64(13)),
                ("holder", FieldValue::Pubkey(HOLDER)),
                ("amount_raw", FieldValue::U64(1_000_000)),
                ("rate_ppm", FieldValue::U64(120_000)),
                ("maturity", FieldValue::I64(1_799_193_253)),
                ("transition_id", FieldValue::U64(18)),
                ("commit_index", FieldValue::U64(17)),
                ("declared_hash", FieldValue::Hash("00")),
            ],
            TREASURY_VAULT_LABEL,
            HOLDER,
            "SOL",
            1_000_000,
            "borrow",
        ),
        (
            // Phase 2: window-13 fees declared 109,500, collected in full.
            "FeesCollected",
            vec![
                ("window_id", FieldValue::U64(13)),
                ("operator", FieldValue::Pubkey(OPERATOR)),
                ("amount_raw", FieldValue::U64(109_500)),
                ("window_collected_raw", FieldValue::U64(109_500)),
                ("collected_raw", FieldValue::U64(109_500)),
                ("transition_id", FieldValue::U64(20)),
                ("commit_index", FieldValue::U64(19)),
                ("declared_hash", FieldValue::Hash("00")),
            ],
            FEE_VAULT_LABEL,
            OPERATOR,
            "SOL",
            109_500,
            "collect_fees",
        ),
        (
            // Live test: Whale A tranche 1 paid 420,000,000 (the D-10 flag
            // T/30 cap, not the brief's 600,000,000).
            "WhaleTrancheExecuted",
            vec![
                ("window_id", FieldValue::U64(12)),
                ("holder", FieldValue::Pubkey("whale-a")),
                ("tranche_index", FieldValue::U64(0)),
                ("amount_raw", FieldValue::U64(420_000_000)),
                ("tranches_executed", FieldValue::U64(1)),
                ("tranches_total", FieldValue::U64(8)),
                ("transition_id", FieldValue::U64(11)),
                ("commit_index", FieldValue::U64(10)),
                ("declared_hash", FieldValue::Hash("00")),
            ],
            TREASURY_VAULT_LABEL,
            "whale-a",
            "SOL",
            420_000_000,
            "execute_whale_tranche",
        ),
    ]
}

#[test]
fn value_moving_events_map_to_their_movement() {
    let mut failures: Vec<String> = Vec::new();
    let mut passed = 0usize;
    for (i, (name, fields, from, to, mint, amount, instruction)) in
        value_cases().into_iter().enumerate()
    {
        let event = ev("sig-value", 519_000_000 + i as u64, 0, name, &fields);
        match program_event_to_transfer(PROGRAM, &event) {
            Some(t) => {
                let mut bad: Vec<String> = Vec::new();
                if t.from != from {
                    bad.push(format!("from {} != {from}", t.from));
                }
                if t.to != to {
                    bad.push(format!("to {} != {to}", t.to));
                }
                if t.mint != mint {
                    bad.push(format!("mint {} != {mint}", t.mint));
                }
                if t.amount != amount as i128 {
                    bad.push(format!("amount {} != {amount}", t.amount));
                }
                if t.instruction != instruction {
                    bad.push(format!("instruction {} != {instruction}", t.instruction));
                }
                if t.program_id != PROGRAM {
                    bad.push(format!("program {} != {PROGRAM}", t.program_id));
                }
                if t.signature != "sig-value" || t.slot != 519_000_000 + i as u64 {
                    bad.push("evidence (signature/slot) not carried through".to_string());
                }
                if bad.is_empty() {
                    passed += 1;
                    println!("  [PASS] {name} -> {from} -> {to} {amount} {mint} ({instruction})");
                } else {
                    failures.push(format!("{name}: {}", bad.join("; ")));
                    println!("  [FAIL] {name}: {}", bad.join("; "));
                }
            }
            None => {
                failures.push(format!("{name}: value-moving event mapped to None"));
                println!("  [FAIL] {name}: mapped to None");
            }
        }
    }
    println!("--- value-moving events: {passed}/7 mapped ---");
    assert!(failures.is_empty(), "adapter failures:\n  - {}", failures.join("\n  - "));
}

/// The 30 non-value events: governance/lifecycle plus the four
/// bookkeeping-only ones. None of them may produce a transfer — a guessed
/// transfer here would be a false value movement in the tracer.
#[test]
fn non_value_events_map_to_nothing() {
    let names = [
        // Governance / lifecycle.
        "CapacityStockSet", "Committed", "ConstraintReported",
        "EvidenceRecorded", "ExceptionRaised", "ExceptionResolved",
        "FeeScheduleDeclared", "HolderEnrolled", "MintRegistered",
        "MintUnregistered", "ObjectivesReported", "PlanSubmitted",
        "PolicyAmended", "PolicyInstalled", "Reconciled", "RoundAdvanced",
        "RuleVersionRegistered", "RuleVersionUnregistered", "Settled",
        "SignalSubmitted", "Stage2Launched", "TransitionExpired",
        "TransitionRejected", "TreasuryInitialized", "WhaleScheduleCreated",
        "WindowOpened",
        // Bookkeeping-only (no tokens move).
        "AllocationMade", "BorrowReinvested", "Reinvested", "DefaultSettled",
    ];
    assert_eq!(names.len(), 30, "the 37-event set minus the 7 value-moving ones");
    let mut failures: Vec<String> = Vec::new();
    for name in names {
        // Fields are irrelevant to the None verdict, but give the event a
        // plausible shape so the test pins the name dispatch, not a
        // missing-field refusal.
        let fields: Vec<(&str, FieldValue<'_>)> = vec![
            ("transition_id", FieldValue::U64(1)),
            ("holder", FieldValue::Pubkey(HOLDER)),
            ("amount_raw", FieldValue::U64(1_000_000)),
        ];
        let event = ev("sig-none", 519_000_001, 0, name, &fields);
        if let Some(t) = program_event_to_transfer(PROGRAM, &event) {
            failures.push(format!("{name} produced a transfer: {t:?}"));
            println!("  [FAIL] {name} -> {t:?}");
        }
    }
    println!(
        "--- non-value events: {}/30 correctly None ---",
        30 - failures.len()
    );
    assert!(failures.is_empty(), "adapter failures:\n  - {}", failures.join("\n  - "));
}

#[test]
fn missing_field_is_a_refusal_not_a_guess() {
    // TranchePaid without the holder: the adapter must not invent an
    // endpoint.
    let fields: Vec<(&str, FieldValue<'_>)> =
        vec![("amount_raw", FieldValue::U64(15_650_000))];
    let event = ev("sig-missing", 519_000_002, 0, "TranchePaid", &fields);
    assert!(
        program_event_to_transfer(PROGRAM, &event).is_none(),
        "TranchePaid with no holder must map to None"
    );
    // Wrong-typed field is the same refusal.
    let fields: Vec<(&str, FieldValue<'_>)> = vec![
        ("holder", FieldValue::Pubkey(HOLDER)),
        ("amount_raw", FieldValue::Pubkey("not-an-amount")),
    ];
    let event = ev("sig-mistyped", 519_000_003, 0, "TranchePaid", &fields);
    assert!(
        program_event_to_transfer(PROGRAM, &event).is_none(),
        "TranchePaid with a non-u64 amount must map to None"
    );
    // Unknown event names map to None.
    let event = ev("sig-unknown", 519_000_004, 0, "SomethingNew", &[]);
    assert!(
        program_event_to_transfer(PROGRAM, &event).is_none(),
        "an unknown event must map to None"
    );
}

/// End-to-end dogfood smoke: the live test's dividend payout as the
/// tracer sees it — three TranchePaid events (two 15,650,000 holders +
/// Whale B 37,560,000) ingest cleanly and reconstruct into three
/// one-hop workflows, totalling 68,860,000 lamports out of the vault.
#[test]
fn live_dividend_payouts_ingest_and_reconstruct() {
    let holders = [
        ("holder-a", 15_650_000u64),
        ("holder-b", 15_650_000u64),
        ("whale-b", 37_560_000u64),
    ];
    let mut transfers = Vec::new();
    for (i, (holder, amount)) in holders.iter().enumerate() {
        let fields: Vec<(&str, FieldValue<'_>)> = vec![
            ("window_id", FieldValue::U64(3)),
            ("holder", FieldValue::Pubkey(holder)),
            ("amount_raw", FieldValue::U64(*amount)),
            ("transition_id", FieldValue::U64(4)),
            ("commit_index", FieldValue::U64(3)),
            ("declared_hash", FieldValue::Hash("00")),
        ];
        let sig = format!("live-tranche-{i}");
        let event = ev(
            &sig,
            519_371_000 + i as u64,
            0,
            "TranchePaid",
            &fields,
        );
        transfers.push(
            program_event_to_transfer(PROGRAM, &event)
                .expect("TranchePaid must map"),
        );
    }
    let ingested = ingest(transfers).expect("mapped transfers must ingest");
    assert_eq!(ingested.len(), 3);
    let total: i128 = ingested.iter().map(|t| t.amount).sum();
    assert_eq!(total, 68_860_000, "2 x 15,650,000 + 37,560,000");
    let workflows = reconstruct(&ingested);
    assert_eq!(workflows.len(), 3, "one hop per holder, no false chaining");
    for (wf, (_, amount)) in workflows.iter().zip(holders.iter()) {
        assert_eq!(wf.hops.len(), 1);
        assert_eq!(wf.hops[0].amount, *amount as i128);
        assert_eq!(wf.origin(), Some(TREASURY_VAULT_LABEL));
        assert_eq!(
            NetEffect::of(wf).net_of(TREASURY_VAULT_LABEL, "SOL"),
            -(*amount as i128)
        );
    }
}
