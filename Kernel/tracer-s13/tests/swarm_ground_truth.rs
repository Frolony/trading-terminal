//! W6.2 (second pass) — devnet-swarm ground truth.
//!
//! The first pass (`labeled_validation.rs`) validated the classifiers
//! against a synthetic corpus. This pass validates against traffic WE
//! generated: the W3.3 devnet swarm — 12 rotation-cycle windows × the full
//! Stage 1–3 holder lifecycle, 96/96 txs confirmed — plus the W3.3.2 whale
//! runs. The labels are known exactly because we ran the traffic.
//!
//! ## Ground-truth sources (all in the goal workspace)
//!
//! * `hidden_files/w33-scratch/rotation_cycle.py` — the swarm driver; the
//!   DISC map is the authoritative family list, the per-window sequence is
//!   `propose_commit → open_window → enroll → pay_tranche → borrow →
//!   settle_default → pay_tranche (forfeited) → exit`, and `fund()` is a
//!   20,000,000-lamport system transfer per fresh holder.
//! * `hidden_files/w33-scratch/cycle-fee-table.json` — per-instruction fees
//!   (5,000 lamports) and CU, all 96 entries.
//! * `hidden_files/w33-gas-report.md` — §1 measured baseline incl. the
//!   W3.3.2 whale instructions (`schedule_whale_exit`, `pay_whale_tranche`);
//!   "each holder PDA ~0.00114 SOL (holder-paid at enroll)".
//! * `hidden_files/w33-mock-treasury/programs/mock-treasury/src/lib.rs` —
//!   the mock's instructions perform NO value transfers (no system-program
//!   invokes in the instruction bodies); every treasury instruction is
//!   bookkeeping on the treasury/holder PDAs.
//!
//! ## Modeling (stated, not hidden)
//!
//! * Instruction invocations are modeled as events with `amount = 0` — the
//!   mock moved no value in its instructions (see the mock source). Each
//!   event carries what the indexer saw: the real program id, the real
//!   family name, the signer as `from`, the treasury PDA as `to`. The
//!   treasury story lives in the instruction families, and the test asserts
//!   the machine reads it there — via the pinned dictionary, the
//!   documented operator path ("pin a program id after observing a real
//!   deployment"), never from memory.
//! * Value movements are modeled exactly as observed: the 20M-lamport
//!   funding transfers and the ~1.14M-lamport PDA-rent payments, both on
//!   the system program. Ground truth: these are payments/plumbing — the
//!   machine must NOT label them treasury flows.
//! * Slots and signatures are illustrative; the fee table retained
//!   labels/fees/CU, not signatures or slots. The instruction ORDER and
//!   FAMILIES are the ground truth, and each case says which is which.
//!
//! ## What this pass found
//!
//! The dictionary's TreasuryProgram surface was missing two of the nine
//! families our own program executed on devnet — `propose_commit` and
//! `pay_whale_tranche` (red run: unrecognized; fixed in `dictionary.rs`
//! before this file went green). The pin corpus now asserts the pinned
//! surface equals the mock's 9-family surface in both directions, so a
//! future drift fails loudly.
//!
//! ## Gate status after this pass
//!
//! The devnet-swarm half of W6.2 is DONE. The W6.2 gate stays CLOSED:
//! remaining are real labeled mainnet routes (plus the position-ledger
//! consult validated on real routes — the ledger foundation landed in the
//! fifth pass, synthetic histories only). The MM-unwind overlap and the
//! PositionClose dead arm have since been resolved at the classifier.
//! No label is user-facing.

use tracer_s13::classify::{classify, Label, CONFIDENCE_THRESHOLD};
use tracer_s13::dictionary::{
    default_dictionary, Dictionary, HopKind, Venue, VenueEntry, VENUE_DICTIONARY,
};
use tracer_s13::ingest::{ingest, TransferEvent};
use tracer_s13::reconstruct::reconstruct;

/// The W3.3 mock treasury on devnet — an observed deployment (W3.3 gas
/// report), pinned here through the dictionary's documented operator path.
const MOCK_PROGRAM: &str = "55jcLQiTRH9o46RZAFqsmY7C9w3bp35TiU1pjq62pzoU";
const TREASURY_PDA: &str = "Dm4eNUvV42onLdbmYKev5i8JRJ21jTNV4AyNUWGTttVe";
const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";
const PAYER: &str = "swarm-payer";
const HOLDER_W4: &str = "swarm-holder-w4";
const HOLDER_W4_PDA: &str = "swarm-holder-w4-pda";

/// The 9 instruction families the mock executed on devnet
/// (rotation_cycle.py DISC + W3.3.2 whale runs).
const MOCK_FAMILIES: [&str; 9] = [
    "propose_commit",
    "open_window",
    "enroll",
    "pay_tranche",
    "borrow",
    "settle_default",
    "exit",
    "schedule_whale_exit",
    "pay_whale_tranche",
];

/// The operator pin: our observed deployment over the dictionary's
/// TreasuryProgram surface.
fn pinned_dictionary() -> Dictionary {
    Dictionary::new(
        VENUE_DICTIONARY
            .iter()
            .filter(|e| e.venue == Venue::TreasuryProgram)
            .map(|e| VenueEntry { program_id: Some(MOCK_PROGRAM), ..*e })
            .collect(),
    )
}

#[allow(clippy::too_many_arguments)] // test-only event builder; arity is the point
fn ev(
    sig: &str,
    slot: u64,
    index: u32,
    from: &str,
    to: &str,
    mint: &str,
    amount: i128,
    program: &str,
    instruction: &str,
    fee: u64,
) -> TransferEvent {
    TransferEvent {
        signature: sig.to_string(),
        slot,
        index,
        timestamp_ms: 1_790_000_000_000 + slot * 400,
        from: from.to_string(),
        to: to.to_string(),
        mint: mint.to_string(),
        amount,
        program_id: program.to_string(),
        instruction: instruction.to_string(),
        fee_lamports: fee,
    }
}

/// Dictionary-level: the pin is the documented operator path, and the
/// pinned surface must match the mock's executed surface in both
/// directions — a drift on either side fails loudly.
#[test]
fn swarm_pin_recognition() {
    let dict = pinned_dictionary();
    let mut failures: Vec<String> = Vec::new();
    let mut passed = 0usize;

    println!("=== swarm pin recognition (dictionary level) ===");

    // 1. Surface equality: the dictionary's TreasuryProgram families must be
    // exactly the 9 families the mock executed on devnet.
    let mut pinned_families: Vec<&str> = VENUE_DICTIONARY
        .iter()
        .filter(|e| e.venue == Venue::TreasuryProgram)
        .map(|e| e.family)
        .collect();
    pinned_families.sort_unstable();
    let mut expected = MOCK_FAMILIES.to_vec();
    expected.sort_unstable();
    if pinned_families != expected {
        failures.push(format!(
            "dictionary TreasuryProgram surface drifted from the mock's executed families: dict={pinned_families:?} mock={expected:?}"
        ));
        println!("  [FAIL] surface drift: dict={pinned_families:?}");
    } else {
        passed += 1;
        println!("  [PASS] pinned TreasuryProgram surface == mock's 9 executed families");
    }

    // 2. Every mock family pins at 0.95 on the observed deployment.
    for f in MOCK_FAMILIES {
        match dict.recognize(MOCK_PROGRAM, f) {
            Some(hit)
                if hit.venue == Venue::TreasuryProgram
                    && hit.hop == HopKind::TreasuryFlow
                    && hit.pinned
                    && (hit.confidence - 0.95).abs() < 1e-9 =>
            {
                passed += 1;
                println!("  [PASS] pin: {f} -> TreasuryProgram @ 0.95");
            }
            other => {
                failures.push(format!(
                    "pin: {f} -> {other:?}, expected TreasuryProgram/TreasuryFlow @ 0.95 pinned"
                ));
                println!("  [FAIL] pin: {f} -> {other:?}");
            }
        }
    }

    // 3. Family-only (unpinned program) still scores exactly 0.70 — the
    // two-tier rule holds on our own families too: a name alone never
    // carries a classification.
    for f in ["enroll", "pay_tranche"] {
        match default_dictionary().recognize("some-unrelated-program", f) {
            Some(hit) if !hit.pinned && (hit.confidence - 0.70).abs() < 1e-9 => {
                passed += 1;
                println!("  [PASS] family-only: {f} -> 0.70 unpinned");
            }
            other => {
                failures.push(format!(
                    "family-only: {f} -> {other:?}, expected 0.70 unpinned"
                ));
                println!("  [FAIL] family-only: {f} -> {other:?}");
            }
        }
    }

    println!(
        "--- swarm pin recognition: {passed} passed, {} failed ---",
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "swarm pin recognition failures:\n  - {}",
        failures.join("\n  - ")
    );
}

/// One workflow-level case.
struct WfCase {
    name: &'static str,
    ground_truth: &'static str,
    events: Vec<TransferEvent>,
    expected_workflows: usize,
    /// Some: every workflow must carry this label at >= min_confidence.
    /// None: every workflow must stay a guess.
    expect_label: Option<Label>,
    min_confidence: f64,
}

/// Window 4 of the rotation cycle (cycle-fee-table.json labels w4:*):
/// propose_commit + open_window (operator-signed) then the 6 holder-signed
/// lifecycle instructions. The mock moved no value in any of them
/// (mock-treasury/src/lib.rs has no transfers); each event is the
/// instruction invocation as the indexer saw it: signer -> treasury PDA,
/// amount 0.
fn window_w4_lifecycle() -> Vec<TransferEvent> {
    let fams = [
        ("propose_commit", PAYER),
        ("open_window", PAYER),
        ("enroll", HOLDER_W4),
        ("pay_tranche", HOLDER_W4),
        ("borrow", HOLDER_W4),
        ("settle_default", HOLDER_W4),
        ("pay_tranche", HOLDER_W4), // the forfeited second tranche: same family, flag cleared on-chain
        ("exit", HOLDER_W4),
    ];
    fams.iter()
        .enumerate()
        .map(|(i, (fam, signer))| {
            ev(
                &format!("swarm-w4-{fam}-{i}"),
                3000 + i as u64,
                0,
                signer,
                TREASURY_PDA,
                "SOL",
                0,
                MOCK_PROGRAM,
                fam,
                5000,
            )
        })
        .collect()
}

fn workflow_corpus() -> Vec<WfCase> {
    vec![
        WfCase {
            name: "swarm_w4_lifecycle",
            ground_truth: "rotation-cycle window 4's 8 instruction invocations (the 96-tx cycle; all confirmed on devnet): operator propose_commit/open_window, then holder enroll/pay_tranche/borrow/settle_default/pay_tranche(forfeited)/exit. Eight 1-hop workflows — the mock chained nothing, it only touched state.",
            events: window_w4_lifecycle(),
            expected_workflows: 8,
            expect_label: Some(Label::TreasuryTransition),
            min_confidence: 0.90,
        },
        WfCase {
            name: "swarm_holder_funding",
            ground_truth: "rotation_cycle.py fund(): the operator's 20,000,000-lamport system transfer funding a fresh holder's tx fees. A payment, not a treasury flow — the machine must not label it.",
            events: vec![ev(
                "swarm-w4-fund",
                2999,
                0,
                PAYER,
                HOLDER_W4,
                "SOL",
                20_000_000,
                SYSTEM_PROGRAM,
                "transfer",
                5000,
            )],
            expected_workflows: 1,
            expect_label: None,
            min_confidence: 0.70, // calibration floor only; guesses carry no confidence
        },
        WfCase {
            name: "swarm_enroll_pda_rent",
            ground_truth: "enroll's holder-PDA creation: ~1,140,000 lamports rent, holder-paid (gas report §1). Rent plumbing on the system program, not a treasury value flow — the machine must not label it.",
            events: vec![ev(
                "swarm-w4-enroll-rent",
                3002,
                1,
                HOLDER_W4,
                HOLDER_W4_PDA,
                "SOL",
                1_140_000,
                SYSTEM_PROGRAM,
                "createAccount",
                5000,
            )],
            expected_workflows: 1,
            expect_label: None,
            min_confidence: 0.70, // calibration floor only; guesses carry no confidence
        },
    ]
}

fn check_case(dict: &Dictionary, case: &WfCase) -> Result<String, String> {
    if case.min_confidence < CONFIDENCE_THRESHOLD {
        return Err(format!(
            "case {}: min_confidence below the seal threshold — re-derive the corpus floors",
            case.name
        ));
    }
    let ingested =
        ingest(case.events.clone()).map_err(|e| format!("ingest rejected the fixture: {e:?}"))?;
    let wfs = reconstruct(&ingested);
    if wfs.len() != case.expected_workflows {
        return Err(format!(
            "stitched into {} workflows, expected {} (ground truth: {})",
            wfs.len(),
            case.expected_workflows,
            case.ground_truth
        ));
    }
    match case.expect_label {
        Some(label) => {
            for wf in &wfs {
                let v = classify(dict, wf);
                if v.is_guess() {
                    return Err(format!(
                        "workflow {}: expected {label:?} (ground truth: {}), machine stayed a guess: {:?}",
                        wf.id, case.ground_truth, v.classification
                    ));
                }
                let got = v.label().expect("is_guess was false, so a label exists");
                if got != label {
                    return Err(format!(
                        "workflow {}: expected {label:?}, machine labeled {got:?} (ground truth: {})",
                        wf.id, case.ground_truth
                    ));
                }
                let conf = v.confidence();
                if conf < case.min_confidence {
                    return Err(format!(
                        "workflow {}: labeled {label:?} but confidence {conf:.2} < {:.2}",
                        wf.id, case.min_confidence
                    ));
                }
                if v.evidence.is_empty() {
                    return Err(format!("workflow {}: labeled with no evidence attached", wf.id));
                }
            }
            Ok(format!(
                "{} workflows, all {label:?} at >= {:.2} (ground truth: {})",
                wfs.len(),
                case.min_confidence,
                case.ground_truth
            ))
        }
        None => {
            for wf in &wfs {
                let v = classify(dict, wf);
                if !v.is_guess() || v.label().is_some() {
                    return Err(format!(
                        "workflow {}: must stay a guess (ground truth: {}), machine labeled {:?}",
                        wf.id,
                        case.ground_truth,
                        v.label()
                    ));
                }
            }
            Ok(format!(
                "{} workflow(s) stayed guesses (ground truth: {})",
                wfs.len(),
                case.ground_truth
            ))
        }
    }
}

/// Workflow-level: the swarm's real traffic through the full pipeline with
/// the pinned dictionary — our own flows read as TreasuryTransition, our
/// own plumbing stays a guess.
#[test]
fn swarm_ground_truth_workflows() {
    let dict = pinned_dictionary();
    let corpus = workflow_corpus();
    let mut failures: Vec<String> = Vec::new();
    let mut passed = 0usize;

    println!("=== W6.2 validation report (second pass: devnet-swarm ground truth) ===");
    for case in &corpus {
        match check_case(&dict, case) {
            Ok(note) => {
                passed += 1;
                println!("  [PASS] {}: {}", case.name, note);
            }
            Err(failure) => {
                failures.push(format!("{}: {}", case.name, failure));
                println!("  [FAIL] {}: {}", case.name, failure);
            }
        }
    }
    println!("--- summary: {passed}/{} cases passed ---", corpus.len());
    println!("--- gate: STILL CLOSED. Devnet-swarm half landed; remaining: real labeled mainnet routes (+ position-ledger consult validated on real routes). No label is user-facing. ---");
    println!("=== end of W6.2 validation report ===");

    assert!(
        failures.is_empty(),
        "swarm ground-truth failures:\n  - {}",
        failures.join("\n  - ")
    );
}
