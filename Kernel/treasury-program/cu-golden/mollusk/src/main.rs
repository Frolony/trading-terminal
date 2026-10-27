//! W4.3 CU golden-test harness (mollusk-svm path), slice 6.
//!
//! Extends slice 3's chain with the value-moving family, first member:
//! a full `treasury.init.v1` lifecycle (transition 2) through settle,
//! `initialize_treasury` on it, a settled `treasury.window.v1`
//! lifecycle (transition 3, SHA type) as the window-open source,
//! `open_window` on it (window 0), `enroll` of a holder into window 0,
//! a window-flow lifecycle (transition 4, SHA type) stopped at commit,
//! and the first GATED value-moving measurement: `pay_tranche` against
//! design §2.2 (~7,000 CU EST, 2x band 3,500-14,000 CU). Every
//! instruction must succeed; any failure aborts with a non-zero exit,
//! so a golden file is never written from a failed chain.
//!
//! Slice 2's `commit` stays gated against design §2.2 (25,000-40,000 CU
//! EST, 2x band 12,500-80,000 CU); slice 3's `settle` is gated against
//! design §2.2 (8,000-12,000 CU EST, 2x band 4,000-24,000 CU). A
//! measurement missing its band by more than 2x is recorded as a
//! DESIGN-FINDING, never tuned away. The chain here is minimal (1 plan,
//! 1 signal, 1 constraint, 1 stock, 1 attribution), so the gated
//! values are minimal-chain costs; larger transitions cost more and
//! are measured next. `record_evidence` has no §2.2 estimate, so it is
//! recorded as baseline, not gated.
//!
//! Slice-4 harness correction, stated plainly: slice 1 passed an
//! arbitrary pubkey ([2;32]) as `initialize_config`'s fee_vault, but
//! `initialize_treasury` requires the fee-vault account's key to equal
//! `config.fee_vault` AND to be the `[b"fee-vault"]` PDA (its `init`
//! seeds), so no `initialize_treasury` could ever succeed on slice 1's
//! config. Slice 4 derives the fee vault as the real PDA, so the
//! treasury actually initializes. Slice-1/2/3 CU must reproduce
//! identically under this change (a pubkey value cannot move CU); the
//! scratch run verifies that before landing.
//!
//! Slice-4 scope boundary, also plain: `launch_stage2` is NOT measured
//! here — it refuses `ThresholdUnset` by design (D-2: B is unset until
//! Carlos sets it, and an unset threshold must never read as a pass),
//! so no success chain exists for it yet. `borrow` is NOT measured
//! here either: its book cap is T/12 over the window's period-start
//! snapshot, and `initialize_treasury` funds the treasury at zero with
//! no deposit instruction yet on-chain — any positive borrow refuses
//! the cap. Both are next-slice work, not hidden gaps.
//!
//! Slice 5 (this file): the blocked-family refusal probes. Slice 4 left
//! `borrow` unmeasured because no funding path exists; this slice
//! proves the block is total and measured rather than assuming it:
//! on the slice-4 chain (window 0, zero snapshot, holder position
//! 10,000,000, accrued dividend 6,260,000, no loan), `borrow`,
//! `exit`, and `settle_default` are each executed and each MUST
//! refuse with its exact fail-closed error — `BookCapExceeded`
//! (6053), `ExitExceedsLiquidity` (6060), `NoActiveLoan` (6055) —
//! with the CU each refusal costs, and with state asserted unchanged
//! afterwards. These are refusal measurements, NOT success CU: no
//! success chain exists for these instructions until a funding path
//! puts a nonzero total into TreasuryState, and this slice does not
//! fabricate one (a harness-seeded funded snapshot would measure a
//! chain no real execution can reach, so it is refused here on
//! principle). The funding path itself is a design question for
//! Carlos — see the roadmap slice-5 entry — not builder
//! improvisation. `schedule_whale_exit` is NOT probed here: its
//! zero-FLAG refusal needs a Committed `treasury.exit.v1`
//! transition, which this chain does not build; its refusal is
//! queued behind the funding decision, not skipped silently.
//!
//! Slice 6 (this file): the queued refusal is measured WITHOUT the
//! funding decision, because it does not need a funded snapshot: a
//! Committed `treasury.exit.v1` lifecycle (transition 5, exit policy,
//! parent = chain head (4, [37;32])) is built on the slice-4/5 chain,
//! stopped at commit, and `schedule_whale_exit` is probed on it with a
//! 1,000,000-raw total. The gate order reaches the zero-FLAG check
//! (state Committed, exit type, window binding, positive total all
//! pass) and MUST refuse with `InvalidScheduleAmount` (6065) —
//! FLAG = T/30 over snapshot 0 is zero, so no schedule can be sized.
//! This is a refusal measurement, NOT a success CU: schedule success
//! still needs a funded snapshot, which is still Carlos's funding-path
//! decision. The funding blocker itself is unchanged by this slice.

//! Clock discipline: mollusk's clock starts at unix_timestamp 0 and
//! warp resets it, so this harness sets `mollusk.sysvars.clock`
//! directly. Phase A runs at NOW0 = 1,000,000 with a 3,600s planning
//! window (deadline 1,003,600, so plans/signals/reports land while
//! planning is open); phase B advances the clock to the deadline so
//! reconcile/commit judge planning closed. The 3,600s window is a
//! harness parameter, not a design value (design §9: planning-window
//! durations are genesis parameters).
//!
//! The keys below are throwaway local-test keys, never real secrets.
//! Measurement note: these CU are measured by mollusk-svm's in-process
//! SVM (Agave 4.x line) executing the program .so built by the adopted
//! Solana 2.2.20 toolchain. They are real executions of the real .so,
//! not host timings or estimates; a validator-measured value may differ
//! slightly (feature set, sysvar defaults) and the golden file records
//! the measurement environment so any such difference is visible.

use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::Mollusk;
use sha2::{Digest, Sha256};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::str::FromStr;

const PROGRAM_ID: &str = "FPGSeLBWEiFKdTuqDbJeKhjXeah5H4c3rqsW4Tzn3RAS";
const SYSTEM_ID: &str = "11111111111111111111111111111111";
const CLOCK_ID: &str = "SysvarC1ock11111111111111111111111111111111";
const MOLLUSK_SVM_VERSION: &str = "0.16.2";

const NOW0: i64 = 1_000_000;
const PLANNING_WINDOW: u64 = 3_600;
const NOW1: i64 = NOW0 + PLANNING_WINDOW as i64; // 1,003,600 = deadline
const NOW2: i64 = 2_000_000; // slice-4 phase C (init lifecycle planning)
const NOW3: i64 = 3_000_000; // slice-4 phase E (settled window lifecycle planning)
const NOW4: i64 = 4_000_000; // slice-4 phase G (flow lifecycle planning)
const NOW5: i64 = 5_000_000; // slice-6 phase H (exit lifecycle planning)
const NOW6: i64 = 6_000_000; // slice-7 phase I (funded window-open lifecycle planning)
const NOW7: i64 = 7_000_000; // slice-7 phase J (funded window-flow lifecycle planning)
const NOW8: i64 = 13_000_000; // slice-7 phase K (funded exit lifecycle planning; after the 60-day loan maturity at 12,187,600)
const TOKEN_PROGRAM_ID_STR: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const UPGRADEABLE_LOADER_ID_STR: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
const RENT_ID_STR: &str = "SysvarRent111111111111111111111111111111111";

fn disc(name: &str) -> Vec<u8> {
    Sha256::digest(format!("global:{name}").as_bytes())[..8].to_vec()
}

fn pda(seeds: &[&[u8]], program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(seeds, program_id)
}

fn vec32(ids: &[[u8; 32]]) -> Vec<u8> {
    let mut out = (ids.len() as u32).to_le_bytes().to_vec();
    for id in ids {
        out.extend_from_slice(id);
    }
    out
}

fn vec_u16(vals: &[u16]) -> Vec<u8> {
    let mut out = (vals.len() as u32).to_le_bytes().to_vec();
    for v in vals {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn vec_claims(claims: &[(u16, u64)]) -> Vec<u8> {
    let mut out = (claims.len() as u32).to_le_bytes().to_vec();
    for (id, amount) in claims {
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&amount.to_le_bytes());
    }
    out
}

fn type_window() -> [u8; 32] {
    let mut t = [0u8; 32];
    t[..18].copy_from_slice(b"treasury.window.v1");
    t
}

fn type_window_sha() -> [u8; 32] {
    // SHA-256("treasury.window.v1"), the program's canonical id.
    // Slice 2's harness type_window() is the literal string padded
    // with zeros, which is NOT this id: commit never checks the type
    // against the constant, so slice 2 passed, but open_window and
    // pay_tranche do — a literal-type transition fails them with
    // WindowMismatch. Slice 4 uses the SHA id for every transition
    // that must authorize a treasury instruction.
    [
        4, 183, 159, 106, 50, 193, 247, 212, 60, 77, 35, 78, 18, 62, 225, 63,
        78, 121, 164, 9, 39, 178, 22, 107, 67, 20, 222, 160, 106, 231, 127, 207,
    ]
}

fn type_init() -> [u8; 32] {
    // SHA-256("treasury.init.v1"), the canonical id in the program.
    [
        116, 167, 130, 118, 19, 78, 9, 62, 70, 91, 124, 207, 146, 13, 252, 203,
        41, 105, 101, 71, 5, 249, 233, 78, 114, 250, 85, 94, 229, 225, 62, 47,
    ]
}
/// SHA-256("treasury.exit.v1"), the program's canonical exit type
/// (TREASURY_EXIT_TYPE in lib.rs). Only a Committed transition of
/// this type authorizes `schedule_whale_exit`.
fn type_exit() -> [u8; 32] {
    [
        9, 240, 135, 210, 223, 48, 195, 245, 154, 6, 13, 81, 88, 230, 196, 158,
        209, 166, 62, 101, 10, 160, 167, 184, 155, 53, 66, 198, 210, 241, 254, 109,
    ]
}

const OPERATOR_SETTLE: [u8; 32] = [
    72, 124, 97, 111, 249, 107, 133, 84, 171, 144, 180, 21, 142, 58, 188, 30,
    25, 51, 152, 11, 111, 68, 99, 175, 120, 72, 162, 74, 234, 38, 52, 101,
];

/// Runs one instruction, asserts success, prints its measured CU, and
/// returns the threaded accounts plus the measured CU.
fn run(
    mollusk: &Mollusk,
    label: &str,
    ix: &Instruction,
    accounts: &[(Pubkey, Account)],
) -> (Vec<(Pubkey, Account)>, u64) {
    let result = mollusk.process_instruction(ix, accounts);
    println!(
        "{label}: program_result={:?} cu={}",
        result.program_result, result.compute_units_consumed
    );
    assert!(
        result.program_result.is_ok(),
        "{label} failed: {:?}",
        result.program_result
    );
    (result.resulting_accounts, result.compute_units_consumed)
}

/// Runs one instruction, asserts it REFUSES with the exact Anchor
/// custom error code, prints its measured CU, and returns the CU.
/// The threaded accounts are deliberately NOT replaced: a refused
/// instruction changes no state, so the caller keeps the pre-state
/// and asserts that separately. Slice 5 only.
fn run_expect_err(
    mollusk: &Mollusk,
    label: &str,
    ix: &Instruction,
    accounts: &[(Pubkey, Account)],
    expected_code: u32,
) -> u64 {
    let result = mollusk.process_instruction(ix, accounts);
    println!(
        "{label}: program_result={:?} cu={}",
        result.program_result, result.compute_units_consumed
    );
    assert!(
        result.program_result.is_err(),
        "{label} unexpectedly succeeded"
    );
    let rendered = format!("{:?}", result.program_result);
    assert!(
        rendered.contains(&format!("Custom({expected_code})")),
        "{label} refused with the wrong error: {rendered} (expected Custom({expected_code}))"
    );
    result.compute_units_consumed
}

/// Runs one instruction, asserts it REFUSES with any error, and
/// returns its measured CU plus the rendered error. For refusals
/// that happen at the Anchor account layer before the handler's
/// own gate runs (a double `init`, an account that does not exist
/// yet) there is no custom code to assert — the refusal itself,
/// with its rendered error recorded verbatim in the golden note,
/// is the measurement. The threaded accounts are deliberately NOT
/// replaced, as in `run_expect_err`.
fn run_expect_any_err(
    mollusk: &Mollusk,
    label: &str,
    ix: &Instruction,
    accounts: &[(Pubkey, Account)],
) -> (u64, String) {
    let result = mollusk.process_instruction(ix, accounts);
    println!(
        "{label}: program_result={:?} cu={}",
        result.program_result, result.compute_units_consumed
    );
    assert!(
        result.program_result.is_err(),
        "{label} unexpectedly succeeded"
    );
    let rendered = format!("{:?}", result.program_result);
    (result.compute_units_consumed, rendered)
}

fn upsert(accounts: &mut Vec<(Pubkey, Account)>, entry: (Pubkey, Account)) {
    if let Some(slot) = accounts.iter_mut().find(|(k, _)| *k == entry.0) {
        *slot = entry;
    } else {
        accounts.push(entry);
    }
}

/// Slice-7 helper: one full governance lifecycle — observation,
/// register, planning, plan, signal, constraint, reconcile, commit —
/// optionally followed by evidence + exact settle (the window-open
/// source transitions need settle; flow and exit transitions stop
/// at commit, because executions ride commit). All CU are pushed
/// to `measured` with the given label suffix (e.g. ":6"). The
/// caller derives every PDA; this helper only threads accounts.
#[allow(clippy::too_many_arguments)]
fn lifecycle_to_commit(
    mollusk: &mut Mollusk,
    mut accounts: Vec<(Pubkey, Account)>,
    measured: &mut Vec<serde_json::Value>,
    program_id: Pubkey,
    clock_id: Pubkey,
    system_id: Pubkey,
    oracle_record: Pubkey,
    oracle: Pubkey,
    proposer_record: Pubkey,
    proposer: Pubkey,
    evaluator_record: Pubkey,
    evaluator: Pubkey,
    chain: Pubkey,
    stock: Pubkey,
    policy: Pubkey,
    observation: Pubkey,
    transition: Pubkey,
    plan: Pubkey,
    signal: Pubkey,
    signal_bump: u8,
    tid: u64,
    obs_exp: i64,
    expires: i64,
    declared: [u8; 32],
    parent_index: u64,
    parent_hash: [u8; 32],
    constraint_c: [u8; 32],
    now: i64,
    suffix: &str,
    settle_evidence: Option<Pubkey>,
) -> Vec<(Pubkey, Account)> {
    mollusk.sysvars.clock.unix_timestamp = now;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let mut data = disc("post_observation");
    data.extend_from_slice(&tid.to_le_bytes());
    data.extend_from_slice(&now.to_le_bytes());
    data.extend_from_slice(&obs_exp.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&[8u8; 32]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(oracle_record, false),
            AccountMeta::new(oracle, true),
            AccountMeta::new(observation, false),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, &format!("post_observation{suffix}"), &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": format!("post_observation{suffix}"), "cu": cu, "result": "Success", "note": "slice-7 setup"}));
    let mut data = disc("register_transition");
    data.extend_from_slice(&tid.to_le_bytes());
    data.extend_from_slice(&tid.to_le_bytes());
    // The transition type is the policy's type: the caller passes
    // the policy account, and the type bytes are recovered from
    // the policy account's data (offset 8: 8 disc + 32 type).
    let policy_type: [u8; 32] = {
        let p = accounts.iter().find(|(k, _)| *k == policy).map(|(_, a)| a.clone()).unwrap();
        p.data[8..40].try_into().unwrap()
    };
    data.extend_from_slice(&policy_type);
    data.extend_from_slice(&expires.to_le_bytes());
    data.extend_from_slice(&declared);
    data.extend_from_slice(&parent_index.to_le_bytes());
    data.extend_from_slice(&parent_hash);
    data.extend_from_slice(&vec32(&[]));
    data.extend_from_slice(&vec_u16(&[1])); // slice-27: relevant_groups [1], overlapping the proposer's group
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(observation, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(transition, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, &format!("register_transition{suffix}"), &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": format!("register_transition{suffix}"), "cu": cu, "result": "Success", "note": "slice-7 setup"}));
    let mut data = disc("open_planning");
    data.extend_from_slice(&tid.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, &format!("open_planning{suffix}"), &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": format!("open_planning{suffix}"), "cu": cu, "result": "Success", "note": "slice-7 setup"}));
    let mut data = disc("submit_plan");
    data.extend_from_slice(&tid.to_le_bytes());
    data.extend_from_slice(&vec_claims(&[(7u16, 100u64)]));
    data.extend_from_slice(&expires.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(plan, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, &format!("submit_plan{suffix}"), &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": format!("submit_plan{suffix}"), "cu": cu, "result": "Success", "note": "slice-7 setup"}));
    let mut data = disc("submit_signal");
    data.extend_from_slice(&tid.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&[signal_bump]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(signal, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, &format!("submit_signal{suffix}"), &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": format!("submit_signal{suffix}"), "cu": cu, "result": "Success", "note": "slice-7 setup"}));
    let mut data = disc("submit_constraint_report");
    data.extend_from_slice(&tid.to_le_bytes());
    data.extend_from_slice(&constraint_c);
    data.push(0u8);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(evaluator_record, false),
            AccountMeta::new(transition, false),
            AccountMeta::new(evaluator, true),
        ],
    );
    let (next, cu) = run(&mollusk, &format!("submit_constraint_report{suffix}"), &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": format!("submit_constraint_report{suffix}"), "cu": cu, "result": "Success", "note": "slice-7 setup"}));
    mollusk.sysvars.clock.unix_timestamp = now + PLANNING_WINDOW as i64;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let mut data = disc("reconcile");
    data.extend_from_slice(&tid.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new(transition, false),
        AccountMeta::new(proposer, true),
        AccountMeta::new_readonly(clock_id, false),
    ];
    metas.push(AccountMeta::new(plan, false));
    metas.push(AccountMeta::new(stock, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, &format!("reconcile{suffix}"), &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": format!("reconcile{suffix}"), "cu": cu, "result": "Success", "note": "slice-7 setup"}));
    let mut data = disc("commit");
    data.extend_from_slice(&tid.to_le_bytes());
    data.extend_from_slice(&tid.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new(transition, false),
        AccountMeta::new_readonly(policy, false),
        AccountMeta::new_readonly(observation, false),
        AccountMeta::new(chain, false),
        AccountMeta::new(proposer, true),
        AccountMeta::new_readonly(clock_id, false),
    ];
    metas.push(AccountMeta::new(plan, false));
    metas.push(AccountMeta::new_readonly(proposer_record, false));
    metas.push(AccountMeta::new(stock, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, &format!("commit{suffix}"), &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": format!("commit{suffix}"), "cu": cu, "result": "Success", "note": "slice-7 setup (repeat of the gated instruction)"}));
    let tr = accounts.iter().find(|(k, _)| *k == transition).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(tr.data[48], 3u8, "transition {tid} is not COMMITTED");
    if let Some(evidence) = settle_evidence {
        let mut data = disc("record_evidence");
        data.extend_from_slice(&tid.to_le_bytes());
        data.extend_from_slice(&declared);
        let ix = Instruction::new_with_bytes(
            program_id,
            &data,
            vec![
                AccountMeta::new_readonly(oracle_record, false),
                AccountMeta::new_readonly(transition, false),
                AccountMeta::new(evidence, false),
                AccountMeta::new(oracle, true),
                AccountMeta::new_readonly(system_id, false),
            ],
        );
        let (next, cu) = run(&mollusk, &format!("record_evidence{suffix}"), &ix, &accounts);
        accounts = next;
        measured.push(serde_json::json!({"instruction": format!("record_evidence{suffix}"), "cu": cu, "result": "Success", "note": "slice-7 setup"}));
        let mut data = disc("settle");
        data.extend_from_slice(&tid.to_le_bytes());
        data.extend_from_slice(&declared);
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(proposer.as_ref());
        data.extend_from_slice(&5_000_000i64.to_le_bytes());
        let mut metas = vec![
            AccountMeta::new_readonly(oracle_record, false),
            AccountMeta::new(transition, false),
            AccountMeta::new_readonly(evidence, false),
            AccountMeta::new_readonly(oracle, true),
        ];
        metas.push(AccountMeta::new(proposer_record, false));
        metas.push(AccountMeta::new(plan, false));
        let ix = Instruction::new_with_bytes(program_id, &data, metas);
        let (next, cu) = run(&mollusk, &format!("settle{suffix}"), &ix, &accounts);
        accounts = next;
        measured.push(serde_json::json!({"instruction": format!("settle{suffix}"), "cu": cu, "result": "Success", "note": "slice-7 setup (repeat of the gated instruction)"}));
        let tr = accounts.iter().find(|(k, _)| *k == transition).map(|(_, a)| a.clone()).unwrap();
        assert_eq!(tr.data[48], 4u8, "transition {tid} is not SETTLED");
    }
    accounts
}

fn funded() -> Account {
    Account {
        lamports: 10_000_000_000,
        data: vec![],
        owner: Pubkey::from_str(SYSTEM_ID).unwrap(),
        executable: false,
        rent_epoch: 0,
    }
}

fn main() {
    let out_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "cu-golden-mollusk.json".to_string());
    let so_dir = std::env::var("SBF_OUT_DIR")
        .unwrap_or_else(|_| "target/deploy".to_string());
    let so_path = format!("{so_dir}/treasury_program.so");
    let so_bytes = std::fs::read(&so_path)
        .unwrap_or_else(|e| panic!("cannot read program .so at {so_path}: {e}"));
    let so_sha256 = hex_encode(&Sha256::digest(&so_bytes));

    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let system_id = Pubkey::from_str(SYSTEM_ID).unwrap();
    let clock_id = Pubkey::from_str(CLOCK_ID).unwrap();
    let mut mollusk = Mollusk::new(&program_id, "treasury_program");
    // Slice 7: load the REAL SPL Token program into the SVM so
    // `deposit` executes a genuine TransferChecked CPI, not a
    // stub. The ELF is the public deployed program bytes dumped
    // from devnet (`solana program dump`, a read — no deployment,
    // no wallet, no funds); its sha256 is recorded in the golden
    // file. Path via SPL_TOKEN_SO_PATH (set by run-mollusk.sh to
    // the cu-golden fixture); the loader key is the standard
    // upgradeable loader, matching mollusk's own default.
    let token_program_id = Pubkey::from_str(TOKEN_PROGRAM_ID_STR).unwrap();
    let token_elf_path = std::env::var("SPL_TOKEN_SO_PATH")
        .unwrap_or_else(|_| "spl-token.so".to_string());
    let token_elf = std::fs::read(&token_elf_path)
        .unwrap_or_else(|e| panic!("cannot read SPL Token ELF at {token_elf_path}: {e} (set SPL_TOKEN_SO_PATH)"));
    let token_elf_sha256 = hex_encode(&Sha256::digest(&token_elf));
    mollusk.add_program_with_loader_and_elf(
        &token_program_id,
        &Pubkey::from_str(UPGRADEABLE_LOADER_ID_STR).unwrap(),
        &token_elf,
    );
    mollusk.sysvars.clock.unix_timestamp = NOW0;

    let authority = Pubkey::new_from_array([1u8; 32]);
    let agent = Pubkey::new_from_array([3u8; 32]);
    let proposer = Pubkey::new_from_array([4u8; 32]);
    let evaluator = Pubkey::new_from_array([5u8; 32]);
    let oracle = Pubkey::new_from_array([6u8; 32]);
    let operator = Pubkey::new_from_array([7u8; 32]);
    let holder = Pubkey::new_from_array([8u8; 32]);
    let (config, _) = pda(&[b"config"], &program_id);
    let (chain, _) = pda(&[b"chain"], &program_id);
    // Slice-4 correction: the fee vault is its PDA, not a placeholder.
    let (fee_vault, _) = pda(&[b"fee-vault"], &program_id);
    let (treasury_state, _) = pda(&[b"treasury"], &program_id);
    let (lending_book, _) = pda(&[b"lending-book"], &program_id);
    let (window_state, _) = pda(&[b"window", &0u64.to_le_bytes()], &program_id);
    let (holder_record, _) = pda(&[b"holder", &0u64.to_le_bytes(), holder.as_ref()], &program_id);
    let (operator_record, _) = pda(&[b"agent", operator.as_ref()], &program_id);
    let (agent_record, _) = pda(&[b"agent", agent.as_ref()], &program_id);
    let (proposer_record, _) = pda(&[b"agent", proposer.as_ref()], &program_id);
    let (evaluator_record, _) = pda(&[b"agent", evaluator.as_ref()], &program_id);
    let (oracle_record, _) = pda(&[b"agent", oracle.as_ref()], &program_id);
    let (stock, _) = pda(&[b"stock", &7u16.to_le_bytes()], &program_id);
    let (policy, _) = pda(&[b"policy", type_window().as_ref()], &program_id);
    let (observation, _) = pda(&[b"observation", &1u64.to_le_bytes()], &program_id);
    let (transition, _) = pda(&[b"transition", &1u64.to_le_bytes()], &program_id);
    let (evidence, _) = pda(&[b"evidence", &1u64.to_le_bytes()], &program_id);
    let (plan, _) = pda(&[b"plan", &1u64.to_le_bytes(), proposer.as_ref()], &program_id);
    // Step-2 fork: the harness keeps the canonical bump (it derives
    // canonically off-chain, as a real caller would) and passes it as
    // the new trailing `signal_bump` argument.
    let (signal, signal_bump) = pda(
        &[b"signal", &1u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
        &program_id,
    );
    let (policy_init, _) = pda(&[b"policy", type_init().as_ref()], &program_id);
    let (observation2, _) = pda(&[b"observation", &2u64.to_le_bytes()], &program_id);
    let (transition2, _) = pda(&[b"transition", &2u64.to_le_bytes()], &program_id);
    let (evidence2, _) = pda(&[b"evidence", &2u64.to_le_bytes()], &program_id);
    let (plan2, _) = pda(&[b"plan", &2u64.to_le_bytes(), proposer.as_ref()], &program_id);
    let (signal2, signal2_bump) = pda(
        &[b"signal", &2u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
        &program_id,
    );
    let (observation3, _) = pda(&[b"observation", &3u64.to_le_bytes()], &program_id);
    let (transition3, _) = pda(&[b"transition", &3u64.to_le_bytes()], &program_id);
    let (plan3, _) = pda(&[b"plan", &3u64.to_le_bytes(), proposer.as_ref()], &program_id);
    let (signal3, signal3_bump) = pda(
        &[b"signal", &3u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
        &program_id,
    );
    let (policy_sha, _) = pda(&[b"policy", type_window_sha().as_ref()], &program_id);
    let (evidence3, _) = pda(&[b"evidence", &3u64.to_le_bytes()], &program_id);
    let (observation4, _) = pda(&[b"observation", &4u64.to_le_bytes()], &program_id);
    let (transition4, _) = pda(&[b"transition", &4u64.to_le_bytes()], &program_id);
    let (plan4, _) = pda(&[b"plan", &4u64.to_le_bytes(), proposer.as_ref()], &program_id);
    let (signal4, signal4_bump) = pda(
        &[b"signal", &4u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
        &program_id,
    );
    let (policy_exit, _) = pda(&[b"policy", type_exit().as_ref()], &program_id);
    let (observation5, _) = pda(&[b"observation", &5u64.to_le_bytes()], &program_id);
    let (transition5, _) = pda(&[b"transition", &5u64.to_le_bytes()], &program_id);
    let (plan5, _) = pda(&[b"plan", &5u64.to_le_bytes(), proposer.as_ref()], &program_id);
    let (signal5, signal5_bump) = pda(
        &[b"signal", &5u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
        &program_id,
    );
    let (exit_schedule, _) = pda(
        &[b"schedule", &0u64.to_le_bytes(), holder.as_ref()],
        &program_id,
    );

    let empty = Account {
        lamports: 0,
        data: vec![],
        owner: system_id,
        executable: false,
        rent_epoch: 0,
    };
    let mut accounts: Vec<(Pubkey, Account)> = vec![
        keyed_account_for_system_program(),
        mollusk.sysvars.keyed_account_for_clock_sysvar(),
        (config, empty.clone()),
        (chain, empty.clone()),
        (agent_record, empty.clone()),
        (proposer_record, empty.clone()),
        (evaluator_record, empty.clone()),
        (oracle_record, empty.clone()),
        (stock, empty.clone()),
        (policy, empty.clone()),
        (observation, empty.clone()),
        (transition, empty.clone()),
        (evidence, empty.clone()),
        (plan, empty.clone()),
        (signal, empty.clone()),
        (authority, funded()),
        (proposer, funded()),
        (evaluator, funded()),
        (oracle, funded()),
        (operator, funded()),
        (holder, funded()),
        (fee_vault, empty.clone()),
        (treasury_state, empty.clone()),
        (lending_book, empty.clone()),
        (window_state, empty.clone()),
        (holder_record, empty.clone()),
        (operator_record, empty.clone()),
        (policy_init, empty.clone()),
        (observation2, empty.clone()),
        (transition2, empty.clone()),
        (evidence2, empty.clone()),
        (plan2, empty.clone()),
        (signal2, empty.clone()),
        (observation3, empty.clone()),
        (transition3, empty.clone()),
        (plan3, empty.clone()),
        (signal3, empty.clone()),
        (policy_sha, empty.clone()),
        (evidence3, empty.clone()),
        (observation4, empty.clone()),
        (transition4, empty.clone()),
        (plan4, empty.clone()),
        (signal4, empty.clone()),
        (policy_exit, empty.clone()),
        (observation5, empty.clone()),
        (transition5, empty.clone()),
        (plan5, empty.clone()),
        (signal5, empty.clone()),
        (exit_schedule, empty.clone()),
    ];
    let mut measured: Vec<serde_json::Value> = Vec::new();
    let mut push = |label: &str, cu: u64, note: &str| {
        measured.push(serde_json::json!({"instruction": label, "cu": cu, "result": "Success", "note": note}));
    };

    // ---- Slice 1 (unchanged chain, baseline values) ----
    // 1. initialize_config(fee_vault)
    let mut data = disc("initialize_config");
    data.extend_from_slice(fee_vault.as_ref());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(config, false),
            AccountMeta::new(chain, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "initialize_config", &ix, &accounts);
    accounts = next;
    push("initialize_config", cu, "slice-1 baseline");
    let cfg = accounts.iter().find(|(k, _)| *k == config).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(cfg.data.len(), 76, "Config account size changed");
    assert_eq!(&cfg.data[8..40], &authority.to_bytes(), "Config authority mismatch");

    // 2. register_agent(agent, [], 0, 0)
    let mut data = disc("register_agent");
    data.extend_from_slice(agent.as_ref());
    data.extend_from_slice(&vec_u16(&[]));
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(config, false),
            AccountMeta::new(agent_record, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_agent", &ix, &accounts);
    accounts = next;
    push("register_agent", cu, "slice-1 baseline");

    // 3. grant_role(agent, 0 = Admin)
    let mut data = disc("grant_role");
    data.extend_from_slice(agent.as_ref());
    data.push(0u8);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(agent_record, false),
            AccountMeta::new(authority, true),
        ],
    );
    let (next, cu) = run(&mollusk, "grant_role", &ix, &accounts);
    accounts = next;
    push("grant_role", cu, "slice-1 baseline");

    // 4. set_capacity(resource_id=7, balance=1_000_000)
    let mut data = disc("set_capacity");
    data.extend_from_slice(&7u16.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(stock, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "set_capacity", &ix, &accounts);
    accounts = next;
    push("set_capacity", cu, "slice-1 baseline");

    // 5. grant_capability(agent, [0u8; 32])
    let mut data = disc("grant_capability");
    data.extend_from_slice(agent.as_ref());
    data.extend_from_slice(&[0u8; 32]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(agent_record, false),
            AccountMeta::new(authority, true),
        ],
    );
    let (next, cu) = run(&mollusk, "grant_capability", &ix, &accounts);
    accounts = next;
    push("grant_capability", cu, "slice-1 baseline");

    // ---- Slice 2: agents for the commit chain ----
    // 6. register_agent(proposer, [1], influence 1_000_000, 0) — slice-27: the proposer must overlap every transition's relevant_groups [1] to plan and signal
    let mut data = disc("register_agent");
    data.extend_from_slice(proposer.as_ref());
    data.extend_from_slice(&vec_u16(&[1]));
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(config, false),
            AccountMeta::new(proposer_record, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_agent:proposer", &ix, &accounts);
    accounts = next;
    push("register_agent:proposer", cu, "slice-2 setup");

    // 7. register_agent(evaluator, [], 0, 0) + 8. grant_role(evaluator, 2)
    let mut data = disc("register_agent");
    data.extend_from_slice(evaluator.as_ref());
    data.extend_from_slice(&vec_u16(&[]));
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(config, false),
            AccountMeta::new(evaluator_record, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_agent:evaluator", &ix, &accounts);
    accounts = next;
    push("register_agent:evaluator", cu, "slice-2 setup");
    let mut data = disc("grant_role");
    data.extend_from_slice(evaluator.as_ref());
    data.push(2u8);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(evaluator_record, false),
            AccountMeta::new(authority, true),
        ],
    );
    let (next, cu) = run(&mollusk, "grant_role:evaluator", &ix, &accounts);
    accounts = next;
    push("grant_role:evaluator", cu, "slice-2 setup");

    // 9. register_agent(oracle, [], 0, 0) + 10. grant_role(oracle, 1)
    let mut data = disc("register_agent");
    data.extend_from_slice(oracle.as_ref());
    data.extend_from_slice(&vec_u16(&[]));
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(config, false),
            AccountMeta::new(oracle_record, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_agent:oracle", &ix, &accounts);
    accounts = next;
    push("register_agent:oracle", cu, "slice-2 setup");
    let mut data = disc("grant_role");
    data.extend_from_slice(oracle.as_ref());
    data.push(1u8);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(oracle_record, false),
            AccountMeta::new(authority, true),
        ],
    );
    let (next, cu) = run(&mollusk, "grant_role:oracle", &ix, &accounts);
    accounts = next;
    push("grant_role:oracle", cu, "slice-2 setup");

    // 11. policy_transition(treasury.window.v1, design launch values;
    //     planning_window 3600 is a harness parameter, see header)
    let constraint_c1 = [9u8; 32];
    let mut data = disc("policy_transition");
    data.extend_from_slice(&type_window());
    data.extend_from_slice(&1_000_000u64.to_le_bytes()); // quorum 1.0
    data.extend_from_slice(&500_000u64.to_le_bytes()); // confidence 0.5
    data.extend_from_slice(&500_000u64.to_le_bytes()); // min_obs_conf 0.5
    data.push(0u8); // require_objectives false
    data.extend_from_slice(&PLANNING_WINDOW.to_le_bytes());
    data.extend_from_slice(&3u32.to_le_bytes()); // max_rounds
    data.extend_from_slice(&4u32.to_le_bytes()); // max_plans
    data.extend_from_slice(&vec32(&[constraint_c1])); // mandatory
    data.extend_from_slice(&vec32(&[])); // required capabilities
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(policy, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "policy_transition", &ix, &accounts);
    accounts = next;
    push("policy_transition", cu, "slice-2 setup");

    // 12. post_observation(1, ts 1_000_000, expiry 2_000_000, conf 1_000_000)
    let mut data = disc("post_observation");
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&NOW0.to_le_bytes());
    data.extend_from_slice(&2_000_000i64.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&[8u8; 32]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(oracle_record, false),
            AccountMeta::new(oracle, true),
            AccountMeta::new(observation, false),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "post_observation", &ix, &accounts);
    accounts = next;
    push("post_observation", cu, "slice-2 setup");

    // 13. register_transition(1, obs 1, window type, expires 3_000_000,
    //     declared [7;32], parent (0, zero), no proposer additions)
    let mut data = disc("register_transition");
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&type_window());
    data.extend_from_slice(&3_000_000i64.to_le_bytes());
    data.extend_from_slice(&[7u8; 32]);
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&vec32(&[]));
    data.extend_from_slice(&vec_u16(&[1])); // slice-27: relevant_groups [1], overlapping the proposer's group
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(observation, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(transition, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_transition", &ix, &accounts);
    accounts = next;
    push("register_transition", cu, "slice-2 setup");

    // 14. open_planning(1) by the proposer (deadline = NOW0 + 3600)
    let mut data = disc("open_planning");
    data.extend_from_slice(&1u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "open_planning", &ix, &accounts);
    accounts = next;
    push("open_planning", cu, "slice-2 setup");

    // 15. submit_plan(1, claims [(7, 100)], expires 3_000_000)
    let mut data = disc("submit_plan");
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&vec_claims(&[(7u16, 100u64)]));
    data.extend_from_slice(&3_000_000i64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(plan, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_plan", &ix, &accounts);
    accounts = next;
    push("submit_plan", cu, "slice-2 setup");

    // 16. submit_signal(1, round 0, raw 1_000_000, support 1_000_000)
    //     eff = 1e6*1e6/1e6 = 1_000_000 >= quorum; mean = 1_000_000 >= 0.5
    let mut data = disc("submit_signal");
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&[signal_bump]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(signal, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_signal", &ix, &accounts);
    accounts = next;
    push("submit_signal", cu, "slice-2 setup");

    // Step-2 fork, Attempt 2 — SHADOW-BUMP PROOF (the proof bar's
    // centerpiece). The proposer has just signalled transition 1
    // round 0 at the canonical bump. A second signal from the same
    // agent in the same round, carrying a DIFFERENT valid bump — the
    // next off-curve bump below the canonical one, hence a different
    // valid address — must refuse AlreadySignalled (6078) and change
    // nothing: the round aggregates stay bit-identical and no shadow
    // SignalRecord is created. Under canonical `init` this case could
    // not arise (one canonical address, collision refusal); under a
    // supplied bump, only the state gate stands between the second
    // signal and a double-counted aggregate.
    let tid_bytes = 1u64.to_le_bytes();
    let round_bytes = 0u32.to_le_bytes();
    let mut shadow_found: Option<(Pubkey, u8)> = None;
    for b in (0..signal_bump).rev() {
        let bump_seed = [b];
        if let Ok(addr) = Pubkey::create_program_address(
            &[
                b"signal",
                &tid_bytes,
                &round_bytes,
                proposer.as_ref(),
                &bump_seed,
            ],
            &program_id,
        ) {
            shadow_found = Some((addr, b));
            break;
        }
    }
    let (shadow_addr, shadow_bump) = shadow_found.expect("no second valid bump below the canonical bump");
    let mut shadow_data = disc("submit_signal");
    shadow_data.extend_from_slice(&1u64.to_le_bytes());
    shadow_data.extend_from_slice(&0u32.to_le_bytes());
    shadow_data.extend_from_slice(&1_000_000u64.to_le_bytes());
    shadow_data.extend_from_slice(&1_000_000u64.to_le_bytes());
    shadow_data.extend_from_slice(&[shadow_bump]);
    let shadow_ix = Instruction::new_with_bytes(
        program_id,
        &shadow_data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(shadow_addr, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let mut shadow_accounts = accounts.clone();
    upsert(&mut shadow_accounts, (shadow_addr, empty.clone()));
    let pre_tr = accounts
        .iter()
        .find(|(k, _)| *k == transition)
        .map(|(_, a)| a.clone())
        .unwrap();
    let shadow_result = mollusk.process_instruction(&shadow_ix, &shadow_accounts);
    println!(
        "submit_signal:shadow: program_result={:?} cu={}",
        shadow_result.program_result, shadow_result.compute_units_consumed
    );
    let shadow_rendered = format!("{:?}", shadow_result.program_result);
    assert!(
        shadow_result.program_result.is_err()
            && shadow_rendered.contains("Custom(6078)"),
        "shadow signal refused with the wrong outcome: {shadow_rendered} (expected AlreadySignalled Custom(6078))"
    );
    let post_tr = shadow_result
        .resulting_accounts
        .iter()
        .find(|(k, _)| *k == transition)
        .map(|(_, a)| a.clone())
        .unwrap();
    // TransitionRecord layout: total at data[113..121], weighted at
    // data[121..129] (8 disc + 8 id + 32 type + 1 state + 32 proposer
    // + 8 observation + 8 expiry + 8 deadline + 4 plans + 4 rounds).
    assert_eq!(
        &post_tr.data[113..121],
        &pre_tr.data[113..121],
        "shadow signal moved the round total"
    );
    assert_eq!(
        &post_tr.data[121..129],
        &pre_tr.data[121..129],
        "shadow signal moved the round weighted sum"
    );
    let post_shadow = shadow_result
        .resulting_accounts
        .iter()
        .find(|(k, _)| *k == shadow_addr)
        .map(|(_, a)| a.clone())
        .unwrap();
    assert_eq!(
        post_shadow.data.len(),
        0,
        "shadow signal created a shadow SignalRecord"
    );
    // Deferred push: the `push` closure still borrows `measured`
    // mutably at this point (it is used again below for the report
    // and reconcile entries), so a direct push here fails E0499.
    // The entry is inserted in order after the closure's last use.
    let shadow_entry = serde_json::json!({"instruction": "submit_signal:shadow", "cu": shadow_result.compute_units_consumed, "result": "Refused", "error": "AlreadySignalled", "error_code": 6078, "note": "step-2 fork Attempt 2 shadow-bump proof: second signal, same agent + round, different valid bump — gate refusal, aggregates bit-identical, no shadow record"});

    // Step-2 fork, Attempt 2 — REPLAY PROOF (the proof-bar item the
    // shadow probe alone does not cover: same agent, same round,
    // CANONICAL bump and address). The gate runs before creation,
    // so a replay must refuse AlreadySignalled (6078) with aggregates
    // bit-identical and the original SignalRecord byte-identical —
    // it must never reach the creation CPI a second time.
    let pre_signal = accounts
        .iter()
        .find(|(k, _)| *k == signal)
        .map(|(_, a)| a.clone())
        .unwrap();
    let mut replay_data = disc("submit_signal");
    replay_data.extend_from_slice(&1u64.to_le_bytes());
    replay_data.extend_from_slice(&0u32.to_le_bytes());
    replay_data.extend_from_slice(&1_000_000u64.to_le_bytes());
    replay_data.extend_from_slice(&1_000_000u64.to_le_bytes());
    replay_data.extend_from_slice(&[signal_bump]);
    let replay_ix = Instruction::new_with_bytes(
        program_id,
        &replay_data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(signal, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let replay_result = mollusk.process_instruction(&replay_ix, &accounts);
    println!(
        "submit_signal:replay: program_result={:?} cu={}",
        replay_result.program_result, replay_result.compute_units_consumed
    );
    let replay_rendered = format!("{:?}", replay_result.program_result);
    assert!(
        replay_result.program_result.is_err()
            && replay_rendered.contains("Custom(6078)"),
        "replay refused with the wrong outcome: {replay_rendered} (expected AlreadySignalled Custom(6078))"
    );
    let post_tr_replay = replay_result
        .resulting_accounts
        .iter()
        .find(|(k, _)| *k == transition)
        .map(|(_, a)| a.clone())
        .unwrap();
    assert_eq!(
        &post_tr_replay.data[113..121], &pre_tr.data[113..121],
        "replay moved the round total"
    );
    assert_eq!(
        &post_tr_replay.data[121..129], &pre_tr.data[121..129],
        "replay moved the round weighted sum"
    );
    let post_signal_replay = replay_result
        .resulting_accounts
        .iter()
        .find(|(k, _)| *k == signal)
        .map(|(_, a)| a.clone())
        .unwrap();
    assert_eq!(
        &post_signal_replay.data, &pre_signal.data,
        "replay changed the original SignalRecord"
    );
    let replay_entry = serde_json::json!({"instruction": "submit_signal:replay", "cu": replay_result.compute_units_consumed, "result": "Refused", "error": "AlreadySignalled", "error_code": 6078, "note": "step-2 fork Attempt 2 replay proof: second signal, same agent + round, canonical bump + address — gate refusal before creation, aggregates bit-identical, original SignalRecord byte-identical"});

    // Step-2 fork, Attempt 2 — MISMATCHED (address, bump) PROOF.
    // The shadow address paired with the CANONICAL bump: the seeds
    // constraint derives the canonical address, not the passed one,
    // so validation must fail BEFORE the handler (hence before the
    // gate) — a different error than AlreadySignalled, no state
    // change, and no record created at the shadow address.
    let mut mismatch_data = disc("submit_signal");
    mismatch_data.extend_from_slice(&1u64.to_le_bytes());
    mismatch_data.extend_from_slice(&0u32.to_le_bytes());
    mismatch_data.extend_from_slice(&1_000_000u64.to_le_bytes());
    mismatch_data.extend_from_slice(&1_000_000u64.to_le_bytes());
    mismatch_data.extend_from_slice(&[signal_bump]);
    let mismatch_ix = Instruction::new_with_bytes(
        program_id,
        &mismatch_data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(shadow_addr, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let mut mismatch_accounts = accounts.clone();
    upsert(&mut mismatch_accounts, (shadow_addr, empty.clone()));
    let mismatch_result = mollusk.process_instruction(&mismatch_ix, &mismatch_accounts);
    println!(
        "submit_signal:mismatch: program_result={:?} cu={}",
        mismatch_result.program_result, mismatch_result.compute_units_consumed
    );
    let mismatch_rendered = format!("{:?}", mismatch_result.program_result);
    assert!(
        mismatch_result.program_result.is_err(),
        "mismatched (address, bump) unexpectedly succeeded"
    );
    assert!(
        !mismatch_rendered.contains("Custom(6078)"),
        "mismatched (address, bump) reached the gate instead of failing seeds validation: {mismatch_rendered}"
    );
    let post_tr_mismatch = mismatch_result
        .resulting_accounts
        .iter()
        .find(|(k, _)| *k == transition)
        .map(|(_, a)| a.clone())
        .unwrap();
    assert_eq!(
        &post_tr_mismatch.data[113..121], &pre_tr.data[113..121],
        "mismatch moved the round total"
    );
    assert_eq!(
        &post_tr_mismatch.data[121..129], &pre_tr.data[121..129],
        "mismatch moved the round weighted sum"
    );
    let post_shadow_mismatch = mismatch_result
        .resulting_accounts
        .iter()
        .find(|(k, _)| *k == shadow_addr)
        .map(|(_, a)| a.clone())
        .unwrap();
    assert_eq!(
        post_shadow_mismatch.data.len(),
        0,
        "mismatched (address, bump) created a record at the shadow address"
    );
    let mismatch_entry = serde_json::json!({"instruction": "submit_signal:mismatch", "cu": mismatch_result.compute_units_consumed, "result": "Refused", "error": "SeedsConstraint", "error_code": 2006, "note": format!("step-2 fork Attempt 2 mismatch proof: shadow address + canonical bump — seeds validation fails before the handler (program_result={mismatch_rendered}), aggregates bit-identical, no record created")});

    // 17. submit_constraint_report(1, C1, PASS=0) by the evaluator
    let mut data = disc("submit_constraint_report");
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&constraint_c1);
    data.push(0u8);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(evaluator_record, false),
            AccountMeta::new(transition, false),
            AccountMeta::new(evaluator, true),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_constraint_report", &ix, &accounts);
    accounts = next;
    push("submit_constraint_report", cu, "slice-2 setup");

    // ---- Phase B: advance the clock to the planning deadline ----
    mollusk.sysvars.clock.unix_timestamp = NOW1;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());

    // 18. reconcile(1); remaining = [plan, stock]; fits: 100 <= 1_000_000
    let mut data = disc("reconcile");
    data.extend_from_slice(&1u64.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new(transition, false),
        AccountMeta::new(proposer, true),
        AccountMeta::new_readonly(clock_id, false),
    ];
    metas.push(AccountMeta::new(plan, false));
    metas.push(AccountMeta::new(stock, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, "reconcile", &ix, &accounts);
    accounts = next;
    push("reconcile", cu, "slice-2 setup");

    // Step-2 fork: the `push` closure's last use is the reconcile
    // entry above, so its borrow of `measured` is dead from here on
    // and the deferred shadow entry can be inserted in order, right
    // after the original submit_signal entry.
    let shadow_pos = measured
        .iter()
        .position(|v| v["instruction"] == "submit_signal")
        .expect("submit_signal entry missing");
    measured.insert(shadow_pos + 1, shadow_entry);
    measured.insert(shadow_pos + 2, replay_entry);
    measured.insert(shadow_pos + 3, mismatch_entry);

    // 19. commit(1, obs 1) — THE GATED MEASUREMENT.
    //     remaining = [plan, proposer_record, stock]; mask must be 0.
    let mut data = disc("commit");
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new(transition, false),
        AccountMeta::new_readonly(policy, false),
        AccountMeta::new_readonly(observation, false),
        AccountMeta::new(chain, false),
        AccountMeta::new(proposer, true),
        AccountMeta::new_readonly(clock_id, false),
    ];
    metas.push(AccountMeta::new(plan, false));
    metas.push(AccountMeta::new_readonly(proposer_record, false));
    metas.push(AccountMeta::new(stock, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, "commit", &ix, &accounts);
    accounts = next;
    let gate = if (12_500..=80_000).contains(&cu) {
        "pass"
    } else {
        "DESIGN-FINDING"
    };
    measured.push(serde_json::json!({
        "instruction": "commit",
        "cu": cu,
        "result": "Success",
        "note": "slice-2 GATED measurement: minimal chain (1 plan, 1 signal, 1 constraint, 1 stock)",
        "design_estimate_cu": "25000-40000",
        "gate_2x_band_cu": "12500-80000",
        "gate": gate,
    }));

    // Post-commit assertions: state COMMITTED, chain advanced, stock debited.
    let tr = accounts.iter().find(|(k, _)| *k == transition).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(tr.data[48], 3u8, "TransitionRecord state is not COMMITTED");
    let ch = accounts.iter().find(|(k, _)| *k == chain).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ch.data[8..16].try_into().unwrap()), 1u64, "chain index not 1");
    assert_eq!(&ch.data[16..48], &[7u8; 32], "chain hash is not the declared hash");
    let st = accounts.iter().find(|(k, _)| *k == stock).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(
        u64::from_le_bytes(st.data[10..18].try_into().unwrap()),
        999_900u64,
        "stock not debited 100 from 1_000_000"
    );

    // ---- Slice 3: evidence + settle (the second GATED measurement) ----
    // 20. record_evidence(1, realized = declared [7;32]) by the oracle.
    //     Context order: agent_record, transition_record (read),
    //     evidence_record (init, payer = oracle), oracle, system_program.
    let mut data = disc("record_evidence");
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&[7u8; 32]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(oracle_record, false),
            AccountMeta::new_readonly(transition, false),
            AccountMeta::new(evidence, false),
            AccountMeta::new(oracle, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "record_evidence", &ix, &accounts);
    accounts = next;
    // Direct push, not the `push` closure: the closure's borrow of
    // `measured` is still live at this point in the source order
    // (its last use precedes `commit`'s direct push), so reusing it
    // here would conflict with the direct pushes below.
    measured.push(serde_json::json!({
        "instruction": "record_evidence",
        "cu": cu,
        "result": "Success",
        "note": "slice-3 baseline (no §2.2 estimate)",
    }));

    // 21. settle(1, realized [7;32] == declared, exact) by the oracle,
    //     one attribution: proposer +5 units (5_000_000 raw, SCALE 1e6).
    //     Remaining accounts: proposer_record (writable, takes the
    //     reputation write) + plan (the membership proof set, complete:
    //     exactly plan_count = 1). AgentRecord reputation_raw sits at
    //     byte offset 52 (8 disc + 32 agent + 4 role flags + 8
    //     influence); proposer starts at 0, exact settle credits full.
    let mut data = disc("settle");
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&[7u8; 32]);
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(proposer.as_ref());
    data.extend_from_slice(&5_000_000i64.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new_readonly(oracle_record, false),
        AccountMeta::new(transition, false),
        AccountMeta::new_readonly(evidence, false),
        AccountMeta::new_readonly(oracle, true),
    ];
    metas.push(AccountMeta::new(proposer_record, false));
    metas.push(AccountMeta::new(plan, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, "settle", &ix, &accounts);
    accounts = next;
    let gate = if (4_000..=24_000).contains(&cu) {
        "pass"
    } else {
        "DESIGN-FINDING"
    };
    measured.push(serde_json::json!({
        "instruction": "settle",
        "cu": cu,
        "result": "Success",
        "note": "slice-3 GATED measurement: exact settle (realized == declared), 1 attribution (+5 units to the plan author)",
        "design_estimate_cu": "8000-12000",
        "gate_2x_band_cu": "4000-24000",
        "gate": gate,
    }));

    // Post-settle assertions: state SETTLED (4), realized hash
    // recorded, proposer reputation credited exactly +5,000,000 raw.
    // TransitionRecord realized_hash offset: 8 disc + 8 id + 32 type
    // + 1 state + 32 proposer + 8 obs + 8 expires + 8 deadline
    // + 4 plan_count + 4 round_count + 8 total + 8 weighted
    // + 8 prev_mean + (4 + 1 x 33) constraints + 1 objectives
    // + 1 reconciled + 32 declared + 8 parent_index + 32 parent_hash
    // + 8 commit_index = 256.
    let tr = accounts.iter().find(|(k, _)| *k == transition).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(tr.data[48], 4u8, "TransitionRecord state is not SETTLED");
    assert_eq!(&tr.data[256..288], &[7u8; 32], "realized hash not recorded on settle");
    let pr = accounts.iter().find(|(k, _)| *k == proposer_record).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(
        u64::from_le_bytes(pr.data[52..60].try_into().unwrap()),
        5_000_000u64,
        "proposer reputation not credited +5 units on exact settle"
    );

    // ---- Slice 4: treasury init + window open + enroll + first value-moving measurement ----
    // (a) Init lifecycle, transition 2 (treasury.init.v1). Parent is
    // the slice-2/3 chain head (index 1, hash [7;32]) — LINEAGE judged
    // against a real parent, not the genesis head.
    mollusk.sysvars.clock.unix_timestamp = NOW2;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let constraint_c_init = [11u8; 32];
    let mut data = disc("policy_transition");
    data.extend_from_slice(&type_init());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&500_000u64.to_le_bytes());
    data.extend_from_slice(&500_000u64.to_le_bytes());
    data.push(0u8);
    data.extend_from_slice(&PLANNING_WINDOW.to_le_bytes());
    data.extend_from_slice(&3u32.to_le_bytes());
    data.extend_from_slice(&4u32.to_le_bytes());
    data.extend_from_slice(&vec32(&[constraint_c_init]));
    data.extend_from_slice(&vec32(&[]));
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(policy_init, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "policy_transition:init", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "policy_transition:init", "cu": cu, "result": "Success", "note": "slice-4 setup"}));

    let mut data = disc("post_observation");
    data.extend_from_slice(&2u64.to_le_bytes());
    data.extend_from_slice(&NOW2.to_le_bytes());
    data.extend_from_slice(&3_000_000i64.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&[8u8; 32]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(oracle_record, false),
            AccountMeta::new(oracle, true),
            AccountMeta::new(observation2, false),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "post_observation:2", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "post_observation:2", "cu": cu, "result": "Success", "note": "slice-4 setup"}));

    let mut data = disc("register_transition");
    data.extend_from_slice(&2u64.to_le_bytes());
    data.extend_from_slice(&2u64.to_le_bytes());
    data.extend_from_slice(&type_init());
    data.extend_from_slice(&4_000_000i64.to_le_bytes());
    data.extend_from_slice(&[17u8; 32]);
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&[7u8; 32]);
    data.extend_from_slice(&vec32(&[]));
    data.extend_from_slice(&vec_u16(&[1])); // slice-27: relevant_groups [1], overlapping the proposer's group
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(observation2, false),
            AccountMeta::new_readonly(policy_init, false),
            AccountMeta::new(transition2, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_transition:2", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "register_transition:2", "cu": cu, "result": "Success", "note": "slice-4 setup"}));

    let mut data = disc("open_planning");
    data.extend_from_slice(&2u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition2, false),
            AccountMeta::new_readonly(policy_init, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "open_planning:2", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "open_planning:2", "cu": cu, "result": "Success", "note": "slice-4 setup"}));

    let mut data = disc("submit_plan");
    data.extend_from_slice(&2u64.to_le_bytes());
    data.extend_from_slice(&vec_claims(&[(7u16, 100u64)]));
    data.extend_from_slice(&4_000_000i64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition2, false),
            AccountMeta::new_readonly(policy_init, false),
            AccountMeta::new(plan2, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_plan:2", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_plan:2", "cu": cu, "result": "Success", "note": "slice-4 setup"}));

    let mut data = disc("submit_signal");
    data.extend_from_slice(&2u64.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&[signal2_bump]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition2, false),
            AccountMeta::new_readonly(policy_init, false),
            AccountMeta::new(signal2, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_signal:2", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_signal:2", "cu": cu, "result": "Success", "note": "slice-4 setup"}));

    let mut data = disc("submit_constraint_report");
    data.extend_from_slice(&2u64.to_le_bytes());
    data.extend_from_slice(&constraint_c_init);
    data.push(0u8);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(evaluator_record, false),
            AccountMeta::new(transition2, false),
            AccountMeta::new(evaluator, true),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_constraint_report:2", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_constraint_report:2", "cu": cu, "result": "Success", "note": "slice-4 setup"}));

    // Phase D: planning deadline for transition 2.
    mollusk.sysvars.clock.unix_timestamp = NOW2 + PLANNING_WINDOW as i64;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let mut data = disc("reconcile");
    data.extend_from_slice(&2u64.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new(transition2, false),
        AccountMeta::new(proposer, true),
        AccountMeta::new_readonly(clock_id, false),
    ];
    metas.push(AccountMeta::new(plan2, false));
    metas.push(AccountMeta::new(stock, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, "reconcile:2", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "reconcile:2", "cu": cu, "result": "Success", "note": "slice-4 setup"}));

    let mut data = disc("commit");
    data.extend_from_slice(&2u64.to_le_bytes());
    data.extend_from_slice(&2u64.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new(transition2, false),
        AccountMeta::new_readonly(policy_init, false),
        AccountMeta::new_readonly(observation2, false),
        AccountMeta::new(chain, false),
        AccountMeta::new(proposer, true),
        AccountMeta::new_readonly(clock_id, false),
    ];
    metas.push(AccountMeta::new(plan2, false));
    metas.push(AccountMeta::new_readonly(proposer_record, false));
    metas.push(AccountMeta::new(stock, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, "commit:2", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "commit:2", "cu": cu, "result": "Success", "note": "slice-4 setup (repeat of the gated instruction; the gate was paid at slice 2)"}));
    let ch = accounts.iter().find(|(k, _)| *k == chain).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ch.data[8..16].try_into().unwrap()), 2u64, "chain index not 2 after commit:2");
    assert_eq!(&ch.data[16..48], &[17u8; 32], "chain hash is not transition 2's declared hash");

    let mut data = disc("record_evidence");
    data.extend_from_slice(&2u64.to_le_bytes());
    data.extend_from_slice(&[17u8; 32]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(oracle_record, false),
            AccountMeta::new_readonly(transition2, false),
            AccountMeta::new(evidence2, false),
            AccountMeta::new(oracle, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "record_evidence:2", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "record_evidence:2", "cu": cu, "result": "Success", "note": "slice-4 setup"}));

    let mut data = disc("settle");
    data.extend_from_slice(&2u64.to_le_bytes());
    data.extend_from_slice(&[17u8; 32]);
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(proposer.as_ref());
    data.extend_from_slice(&5_000_000i64.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new_readonly(oracle_record, false),
        AccountMeta::new(transition2, false),
        AccountMeta::new_readonly(evidence2, false),
        AccountMeta::new_readonly(oracle, true),
    ];
    metas.push(AccountMeta::new(proposer_record, false));
    metas.push(AccountMeta::new(plan2, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, "settle:2", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "settle:2", "cu": cu, "result": "Success", "note": "slice-4 setup (repeat of the gated instruction)"}));
    let tr2 = accounts.iter().find(|(k, _)| *k == transition2).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(tr2.data[48], 4u8, "transition 2 is not SETTLED");
    assert_eq!(&tr2.data[256..288], &[17u8; 32], "realized hash not recorded on settle:2");
    let pr = accounts.iter().find(|(k, _)| *k == proposer_record).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(
        u64::from_le_bytes(pr.data[52..60].try_into().unwrap()),
        10_000_000u64,
        "proposer reputation not 10,000,000 raw after settle:2"
    );

    // (b) initialize_treasury on the settled init transition.
    // Slice 25, D-2 writer path: threshold B is a genesis
    // parameter — the chain pins the initial-testing value
    // 950_000 ppm (95%, relatively close to USDC-style full
    // backing), not a program constant and not a final ratio.
    let mut data = disc("initialize_treasury");
    data.extend_from_slice(&2u64.to_le_bytes());
    data.extend_from_slice(&950_000u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new_readonly(transition2, false),
            AccountMeta::new(treasury_state, false),
            AccountMeta::new(lending_book, false),
            AccountMeta::new(fee_vault, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "initialize_treasury", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "initialize_treasury", "cu": cu, "result": "Success", "note": "slice-25 genesis pin: backing_threshold_ppm 950_000 (D-2 initial-testing 95%)"}));
    let ts = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    // Slice 20: TreasuryState 65 -> 322 (registry count 1 + 8 mint slots x 32).
    assert_eq!(ts.data.len(), 322, "TreasuryState size changed");
    assert_eq!(u64::from_le_bytes(ts.data[48..56].try_into().unwrap()), 950_000u64, "backing_threshold_ppm not 950_000 at initialize_treasury");
    assert_eq!(u64::from_le_bytes(ts.data[56..64].try_into().unwrap()), 0u64, "window_count not 0 at initialize_treasury");
    let lb = accounts.iter().find(|(k, _)| *k == lending_book).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(lb.data[8..16].try_into().unwrap()), 0u64, "lending book outstanding not 0 at initialize_treasury");

    // (c2) Settled window lifecycle, transition 3 (SHA window type).
    // This is the transition open_window must ride: Settled AND of
    // the canonical SHA type. Slice 2's transition 1 is Settled but
    // of the literal-padded type, so it fails open_window's type
    // check (proven by the first scratch run: WindowMismatch).
    mollusk.sysvars.clock.unix_timestamp = NOW3;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let mut data = disc("policy_transition");
    data.extend_from_slice(&type_window_sha());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&500_000u64.to_le_bytes());
    data.extend_from_slice(&500_000u64.to_le_bytes());
    data.push(0u8);
    data.extend_from_slice(&PLANNING_WINDOW.to_le_bytes());
    data.extend_from_slice(&3u32.to_le_bytes());
    data.extend_from_slice(&4u32.to_le_bytes());
    data.extend_from_slice(&vec32(&[constraint_c1]));
    data.extend_from_slice(&vec32(&[]));
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(policy_sha, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "policy_transition:window-sha", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "policy_transition:window-sha", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("post_observation");
    data.extend_from_slice(&3u64.to_le_bytes());
    data.extend_from_slice(&NOW3.to_le_bytes());
    data.extend_from_slice(&4_000_000i64.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&[8u8; 32]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(oracle_record, false),
            AccountMeta::new(oracle, true),
            AccountMeta::new(observation3, false),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "post_observation:3", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "post_observation:3", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("register_transition");
    data.extend_from_slice(&3u64.to_le_bytes());
    data.extend_from_slice(&3u64.to_le_bytes());
    data.extend_from_slice(&type_window_sha());
    data.extend_from_slice(&5_000_000i64.to_le_bytes());
    data.extend_from_slice(&[27u8; 32]);
    data.extend_from_slice(&2u64.to_le_bytes());
    data.extend_from_slice(&[17u8; 32]);
    data.extend_from_slice(&vec32(&[]));
    data.extend_from_slice(&vec_u16(&[1])); // slice-27: relevant_groups [1], overlapping the proposer's group
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(observation3, false),
            AccountMeta::new_readonly(policy_sha, false),
            AccountMeta::new(transition3, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_transition:3", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "register_transition:3", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("open_planning");
    data.extend_from_slice(&3u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition3, false),
            AccountMeta::new_readonly(policy_sha, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "open_planning:3", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "open_planning:3", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("submit_plan");
    data.extend_from_slice(&3u64.to_le_bytes());
    data.extend_from_slice(&vec_claims(&[(7u16, 100u64)]));
    data.extend_from_slice(&5_000_000i64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition3, false),
            AccountMeta::new_readonly(policy_sha, false),
            AccountMeta::new(plan3, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_plan:3", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_plan:3", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("submit_signal");
    data.extend_from_slice(&3u64.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&[signal3_bump]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition3, false),
            AccountMeta::new_readonly(policy_sha, false),
            AccountMeta::new(signal3, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_signal:3", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_signal:3", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("submit_constraint_report");
    data.extend_from_slice(&3u64.to_le_bytes());
    data.extend_from_slice(&constraint_c1);
    data.push(0u8);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(evaluator_record, false),
            AccountMeta::new(transition3, false),
            AccountMeta::new(evaluator, true),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_constraint_report:3", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_constraint_report:3", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    mollusk.sysvars.clock.unix_timestamp = NOW3 + PLANNING_WINDOW as i64;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let mut data = disc("reconcile");
    data.extend_from_slice(&3u64.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new(transition3, false),
        AccountMeta::new(proposer, true),
        AccountMeta::new_readonly(clock_id, false),
    ];
    metas.push(AccountMeta::new(plan3, false));
    metas.push(AccountMeta::new(stock, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, "reconcile:3", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "reconcile:3", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("commit");
    data.extend_from_slice(&3u64.to_le_bytes());
    data.extend_from_slice(&3u64.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new(transition3, false),
        AccountMeta::new_readonly(policy_sha, false),
        AccountMeta::new_readonly(observation3, false),
        AccountMeta::new(chain, false),
        AccountMeta::new(proposer, true),
        AccountMeta::new_readonly(clock_id, false),
    ];
    metas.push(AccountMeta::new(plan3, false));
    metas.push(AccountMeta::new_readonly(proposer_record, false));
    metas.push(AccountMeta::new(stock, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, "commit:3", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "commit:3", "cu": cu, "result": "Success", "note": "slice-4 setup (repeat of the gated instruction)"}));
    let ch = accounts.iter().find(|(k, _)| *k == chain).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ch.data[8..16].try_into().unwrap()), 3u64, "chain index not 3 after commit:3");
    assert_eq!(&ch.data[16..48], &[27u8; 32], "chain hash is not transition 3's declared hash");
    let st = accounts.iter().find(|(k, _)| *k == stock).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(st.data[10..18].try_into().unwrap()), 999_700u64, "stock not 999,700 after commit:3");
    let mut data = disc("record_evidence");
    data.extend_from_slice(&3u64.to_le_bytes());
    data.extend_from_slice(&[27u8; 32]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(oracle_record, false),
            AccountMeta::new_readonly(transition3, false),
            AccountMeta::new(evidence3, false),
            AccountMeta::new(oracle, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "record_evidence:3", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "record_evidence:3", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("settle");
    data.extend_from_slice(&3u64.to_le_bytes());
    data.extend_from_slice(&[27u8; 32]);
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(proposer.as_ref());
    data.extend_from_slice(&5_000_000i64.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new_readonly(oracle_record, false),
        AccountMeta::new(transition3, false),
        AccountMeta::new_readonly(evidence3, false),
        AccountMeta::new_readonly(oracle, true),
    ];
    metas.push(AccountMeta::new(proposer_record, false));
    metas.push(AccountMeta::new(plan3, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, "settle:3", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "settle:3", "cu": cu, "result": "Success", "note": "slice-4 setup (repeat of the gated instruction)"}));
    let tr3 = accounts.iter().find(|(k, _)| *k == transition3).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(tr3.data[48], 4u8, "transition 3 is not SETTLED");
    assert_eq!(&tr3.data[256..288], &[27u8; 32], "realized hash not recorded on settle:3");
    let pr = accounts.iter().find(|(k, _)| *k == proposer_record).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(
        u64::from_le_bytes(pr.data[52..60].try_into().unwrap()),
        15_000_000u64,
        "proposer reputation not 15,000,000 raw after settle:3"
    );

    // (c) open_window on transition 3's settled SHA-type window transition, window 0.
    let mut data = disc("open_window");
    data.extend_from_slice(&3u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(treasury_state, false),
            AccountMeta::new_readonly(lending_book, false),
            AccountMeta::new(transition3, false),
            AccountMeta::new(window_state, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "open_window", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "open_window", "cu": cu, "result": "Success", "note": "slice-4 baseline (no §2.2 estimate)"}));
    let ts = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts.data[56..64].try_into().unwrap()), 1u64, "window_count not 1 after open_window");
    let ws = accounts.iter().find(|(k, _)| *k == window_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(ws.data.len(), 146, "WindowState size changed");
    assert_eq!(u64::from_le_bytes(ws.data[8..16].try_into().unwrap()), 0u64, "window_id not 0");
    assert_eq!(u64::from_le_bytes(ws.data[72..80].try_into().unwrap()), 3u64, "window source transition not 3");

    // (d) enroll the holder into window 0 (cohort 0 = 0 mod 12).
    let mut data = disc("enroll");
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&10_000_000u64.to_le_bytes());
    let (forfeiture_holder, _) = pda(&[b"forfeiture", holder.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder) { accounts.push((forfeiture_holder, empty.clone())); }
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state, false),
            AccountMeta::new(holder_record, false),
            AccountMeta::new(forfeiture_holder, false),
            AccountMeta::new(holder, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "enroll", "cu": cu, "result": "Success", "note": "slice-4 baseline (no §2.2 estimate)"}));
    let hr = accounts.iter().find(|(k, _)| *k == holder_record).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr.data[48..56].try_into().unwrap()), 0u64, "holder cohort not 0");
    assert_eq!(u64::from_le_bytes(hr.data[56..64].try_into().unwrap()), 10_000_000u64, "holder position not 10,000,000");
    assert_eq!(hr.data[89], 1u8, "standing authorization not true at enroll");

    // (e) Operator setup: register + OPERATOR_SETTLE capability.
    let mut data = disc("register_agent");
    data.extend_from_slice(operator.as_ref());
    data.extend_from_slice(&vec_u16(&[]));
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(config, false),
            AccountMeta::new(operator_record, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_agent:operator", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "register_agent:operator", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("grant_capability");
    data.extend_from_slice(operator.as_ref());
    data.extend_from_slice(&OPERATOR_SETTLE);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(operator_record, false),
            AccountMeta::new(authority, true),
        ],
    );
    let (next, cu) = run(&mollusk, "grant_capability:operator", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "grant_capability:operator", "cu": cu, "result": "Success", "note": "slice-4 setup"}));

    // (f) Window-flow lifecycle, transition 4 (window type), parent
    // (2, [17;32]); stopped at commit — executions ride commit, and a
    // Settled transition would refuse every payout.
    mollusk.sysvars.clock.unix_timestamp = NOW4;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let mut data = disc("post_observation");
    data.extend_from_slice(&4u64.to_le_bytes());
    data.extend_from_slice(&NOW4.to_le_bytes());
    data.extend_from_slice(&5_000_000i64.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&[8u8; 32]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(oracle_record, false),
            AccountMeta::new(oracle, true),
            AccountMeta::new(observation4, false),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "post_observation:4", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "post_observation:4", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("register_transition");
    data.extend_from_slice(&4u64.to_le_bytes());
    data.extend_from_slice(&4u64.to_le_bytes());
    data.extend_from_slice(&type_window_sha());
    data.extend_from_slice(&7_000_000i64.to_le_bytes());
    data.extend_from_slice(&[37u8; 32]);
    data.extend_from_slice(&3u64.to_le_bytes());
    data.extend_from_slice(&[27u8; 32]);
    data.extend_from_slice(&vec32(&[]));
    data.extend_from_slice(&vec_u16(&[1])); // slice-27: relevant_groups [1], overlapping the proposer's group
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(observation4, false),
            AccountMeta::new_readonly(policy_sha, false),
            AccountMeta::new(transition4, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_transition:4", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "register_transition:4", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("open_planning");
    data.extend_from_slice(&4u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition4, false),
            AccountMeta::new_readonly(policy_sha, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "open_planning:4", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "open_planning:4", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("submit_plan");
    data.extend_from_slice(&4u64.to_le_bytes());
    data.extend_from_slice(&vec_claims(&[(7u16, 100u64)]));
    data.extend_from_slice(&7_000_000i64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition4, false),
            AccountMeta::new_readonly(policy_sha, false),
            AccountMeta::new(plan4, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_plan:4", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_plan:4", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("submit_signal");
    data.extend_from_slice(&4u64.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&[signal4_bump]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition4, false),
            AccountMeta::new_readonly(policy_sha, false),
            AccountMeta::new(signal4, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_signal:4", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_signal:4", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("submit_constraint_report");
    data.extend_from_slice(&4u64.to_le_bytes());
    data.extend_from_slice(&constraint_c1);
    data.push(0u8);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(evaluator_record, false),
            AccountMeta::new(transition4, false),
            AccountMeta::new(evaluator, true),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_constraint_report:4", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_constraint_report:4", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    mollusk.sysvars.clock.unix_timestamp = NOW4 + PLANNING_WINDOW as i64;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let mut data = disc("reconcile");
    data.extend_from_slice(&4u64.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new(transition4, false),
        AccountMeta::new(proposer, true),
        AccountMeta::new_readonly(clock_id, false),
    ];
    metas.push(AccountMeta::new(plan4, false));
    metas.push(AccountMeta::new(stock, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, "reconcile:4", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "reconcile:4", "cu": cu, "result": "Success", "note": "slice-4 setup"}));
    let mut data = disc("commit");
    data.extend_from_slice(&4u64.to_le_bytes());
    data.extend_from_slice(&4u64.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new(transition4, false),
        AccountMeta::new_readonly(policy_sha, false),
        AccountMeta::new_readonly(observation4, false),
        AccountMeta::new(chain, false),
        AccountMeta::new(proposer, true),
        AccountMeta::new_readonly(clock_id, false),
    ];
    metas.push(AccountMeta::new(plan4, false));
    metas.push(AccountMeta::new_readonly(proposer_record, false));
    metas.push(AccountMeta::new(stock, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, "commit:4", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "commit:4", "cu": cu, "result": "Success", "note": "slice-4 setup (repeat of the gated instruction)"}));
    let tr3 = accounts.iter().find(|(k, _)| *k == transition4).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(tr3.data[48], 3u8, "transition 4 is not COMMITTED");
    let ch = accounts.iter().find(|(k, _)| *k == chain).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ch.data[8..16].try_into().unwrap()), 4u64, "chain index not 4 after commit:4");
    assert_eq!(&ch.data[16..48], &[37u8; 32], "chain hash is not transition 3's declared hash");
    let st = accounts.iter().find(|(k, _)| *k == stock).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(st.data[10..18].try_into().unwrap()), 999_600u64, "stock not 999,600 after four 100 debits");

    // (g) pay_tranche — THE GATED VALUE-MOVING MEASUREMENT.
    // Operator-settled under the standing authorization, $6.26 in raw
    // units (SCALE 1e6) — the operating dividend figure, used here as
    // a harness amount, not a promise.
    let mut data = disc("pay_tranche");
    data.extend_from_slice(&4u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(holder.as_ref());
    data.extend_from_slice(&6_260_000u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition4, false),
            AccountMeta::new_readonly(window_state, false),
            AccountMeta::new(holder_record, false),
        ],
    );
    let (next, cu) = run(&mollusk, "pay_tranche", &ix, &accounts);
    accounts = next;
    let gate = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "pay_tranche",
        "cu": cu,
        "result": "Success",
        "note": "slice-4 GATED measurement: operator-settled tranche payout on a Committed window flow transition (window 0, cohort 0, $6.26 raw)",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate,
    }));
    let hr = accounts.iter().find(|(k, _)| *k == holder_record).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr.data[64..72].try_into().unwrap()), 6_260_000u64, "accrued dividend not 6,260,000 after pay_tranche");
    assert_eq!(hr.data[98], 1u8, "tranche_paid flag not set after pay_tranche");
    let tr3 = accounts.iter().find(|(k, _)| *k == transition4).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(tr3.data[48], 3u8, "transition 4 moved from COMMITTED by pay_tranche");

    // ---- Slice 5: blocked-family refusal probes (no state moves) ----
    // (h) borrow — holder-signed, 1,000,000 raw: within the per-holder
    // cap (50% of the 10,000,000 position) but the book cap is
    // max_outstanding(snapshot 0) = 0, so this MUST refuse.
    let mut data = disc("borrow");
    data.extend_from_slice(&4u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(holder, true),
            AccountMeta::new_readonly(transition4, false),
            AccountMeta::new_readonly(window_state, false),
            AccountMeta::new(holder_record, false),
            AccountMeta::new(lending_book, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "borrow_refused", &ix, &accounts, 6053);
    measured.push(serde_json::json!({
        "instruction": "borrow",
        "cu": cu,
        "result": "Refused",
        "error": "BookCapExceeded",
        "error_code": 6053,
        "note": "slice-5 refusal probe: holder-signed 1,000,000 raw on Committed window flow transition 4 (window 0, zero snapshot) — book cap max_outstanding(0) = 0; NOT a success CU, no funding path exists on-chain",
    }));
    // (i) exit — holder-signed, 1,000,000 raw: within accrued dividend
    // and position, but the D-10 per-order bound is movable_liquidity
    // (snapshot 0) = 0, so this MUST refuse before the stock check.
    let mut data = disc("exit");
    data.extend_from_slice(&4u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(holder, true),
            AccountMeta::new_readonly(transition4, false),
            AccountMeta::new(window_state, false),
            AccountMeta::new(holder_record, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "exit_refused", &ix, &accounts, 6060);
    measured.push(serde_json::json!({
        "instruction": "exit",
        "cu": cu,
        "result": "Refused",
        "error": "ExitExceedsLiquidity",
        "error_code": 6060,
        "note": "slice-5 refusal probe: holder-signed 1,000,000 raw on Committed window flow transition 4 (window 0, zero snapshot) — per-order bound movable_liquidity(0) = 0; NOT a success CU",
    }));
    // (j) settle_default — operator-settled, holder with no loan:
    // the no-active-loan gate MUST refuse before any maturity check.
    let mut data = disc("settle_default");
    data.extend_from_slice(&4u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(holder.as_ref());
    let (forfeiture_holder, _) = pda(&[b"forfeiture", holder.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder) { accounts.push((forfeiture_holder, empty.clone())); }
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition4, false),
            AccountMeta::new_readonly(window_state, false),
            AccountMeta::new(holder_record, false),
            AccountMeta::new(lending_book, false),
            AccountMeta::new(forfeiture_holder, false),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "settle_default_refused", &ix, &accounts, 6055);
    measured.push(serde_json::json!({
        "instruction": "settle_default",
        "cu": cu,
        "result": "Refused",
        "error": "NoActiveLoan",
        "error_code": 6055,
        "note": "slice-5 refusal probe: operator-settled default on a holder with no loan (transition 4, window 0) — no loan exists because borrow cannot succeed on a zero snapshot; NOT a success CU",
    }));
    // Post-refusal assertions: NOTHING moved. Holder loan still 0,
    // position and accrued dividend unchanged, book outstanding
    // still 0, transition 4 still COMMITTED.
    let hr = accounts.iter().find(|(k, _)| *k == holder_record).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr.data[56..64].try_into().unwrap()), 10_000_000u64, "holder position moved during refusal probes");
    assert_eq!(u64::from_le_bytes(hr.data[64..72].try_into().unwrap()), 6_260_000u64, "accrued dividend moved during refusal probes");
    assert_eq!(u64::from_le_bytes(hr.data[72..80].try_into().unwrap()), 0u64, "holder loan appeared during refusal probes");
    let lb = accounts.iter().find(|(k, _)| *k == lending_book).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(lb.data[8..16].try_into().unwrap()), 0u64, "lending book outstanding moved during refusal probes");
    let tr4 = accounts.iter().find(|(k, _)| *k == transition4).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(tr4.data[48], 3u8, "transition 4 moved from COMMITTED during refusal probes");


    // ---- Slice 6: exit lifecycle + schedule_whale_exit zero-FLAG refusal ----
    // (k) Exit lifecycle, transition 5 (treasury.exit.v1). Parent is the
    // slice-4/5 chain head (index 4, hash [37;32]) — LINEAGE judged
    // against a real parent. Stopped at commit: schedules ride commit,
    // exactly as payouts ride the flow transition's commit.
    mollusk.sysvars.clock.unix_timestamp = NOW5;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let constraint_c_exit = [13u8; 32];
    let mut data = disc("policy_transition");
    data.extend_from_slice(&type_exit());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&500_000u64.to_le_bytes());
    data.extend_from_slice(&500_000u64.to_le_bytes());
    data.push(0u8);
    data.extend_from_slice(&PLANNING_WINDOW.to_le_bytes());
    data.extend_from_slice(&3u32.to_le_bytes());
    data.extend_from_slice(&4u32.to_le_bytes());
    data.extend_from_slice(&vec32(&[constraint_c_exit]));
    data.extend_from_slice(&vec32(&[]));
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(policy_exit, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "policy_transition:exit", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "policy_transition:exit", "cu": cu, "result": "Success", "note": "slice-6 setup"}));
    let mut data = disc("post_observation");
    data.extend_from_slice(&5u64.to_le_bytes());
    data.extend_from_slice(&NOW5.to_le_bytes());
    data.extend_from_slice(&6_000_000i64.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&[8u8; 32]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(oracle_record, false),
            AccountMeta::new(oracle, true),
            AccountMeta::new(observation5, false),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "post_observation:5", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "post_observation:5", "cu": cu, "result": "Success", "note": "slice-6 setup"}));
    let mut data = disc("register_transition");
    data.extend_from_slice(&5u64.to_le_bytes());
    data.extend_from_slice(&5u64.to_le_bytes());
    data.extend_from_slice(&type_exit());
    data.extend_from_slice(&8_000_000i64.to_le_bytes());
    data.extend_from_slice(&[47u8; 32]);
    data.extend_from_slice(&4u64.to_le_bytes());
    data.extend_from_slice(&[37u8; 32]);
    data.extend_from_slice(&vec32(&[]));
    data.extend_from_slice(&vec_u16(&[1])); // slice-27: relevant_groups [1], overlapping the proposer's group
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(observation5, false),
            AccountMeta::new_readonly(policy_exit, false),
            AccountMeta::new(transition5, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_transition:5", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "register_transition:5", "cu": cu, "result": "Success", "note": "slice-6 setup"}));
    let mut data = disc("open_planning");
    data.extend_from_slice(&5u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition5, false),
            AccountMeta::new_readonly(policy_exit, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "open_planning:5", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "open_planning:5", "cu": cu, "result": "Success", "note": "slice-6 setup"}));
    let mut data = disc("submit_plan");
    data.extend_from_slice(&5u64.to_le_bytes());
    data.extend_from_slice(&vec_claims(&[(7u16, 100u64)]));
    data.extend_from_slice(&8_000_000i64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition5, false),
            AccountMeta::new_readonly(policy_exit, false),
            AccountMeta::new(plan5, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_plan:5", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_plan:5", "cu": cu, "result": "Success", "note": "slice-6 setup"}));
    let mut data = disc("submit_signal");
    data.extend_from_slice(&5u64.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&[signal5_bump]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition5, false),
            AccountMeta::new_readonly(policy_exit, false),
            AccountMeta::new(signal5, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_signal:5", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_signal:5", "cu": cu, "result": "Success", "note": "slice-6 setup"}));
    let mut data = disc("submit_constraint_report");
    data.extend_from_slice(&5u64.to_le_bytes());
    data.extend_from_slice(&constraint_c_exit);
    data.push(0u8);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(evaluator_record, false),
            AccountMeta::new(transition5, false),
            AccountMeta::new(evaluator, true),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_constraint_report:5", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_constraint_report:5", "cu": cu, "result": "Success", "note": "slice-6 setup"}));
    mollusk.sysvars.clock.unix_timestamp = NOW5 + PLANNING_WINDOW as i64;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let mut data = disc("reconcile");
    data.extend_from_slice(&5u64.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new(transition5, false),
        AccountMeta::new(proposer, true),
        AccountMeta::new_readonly(clock_id, false),
    ];
    metas.push(AccountMeta::new(plan5, false));
    metas.push(AccountMeta::new(stock, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, "reconcile:5", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "reconcile:5", "cu": cu, "result": "Success", "note": "slice-6 setup"}));
    let mut data = disc("commit");
    data.extend_from_slice(&5u64.to_le_bytes());
    data.extend_from_slice(&5u64.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new(transition5, false),
        AccountMeta::new_readonly(policy_exit, false),
        AccountMeta::new_readonly(observation5, false),
        AccountMeta::new(chain, false),
        AccountMeta::new(proposer, true),
        AccountMeta::new_readonly(clock_id, false),
    ];
    metas.push(AccountMeta::new(plan5, false));
    metas.push(AccountMeta::new_readonly(proposer_record, false));
    metas.push(AccountMeta::new(stock, false));
    let ix = Instruction::new_with_bytes(program_id, &data, metas);
    let (next, cu) = run(&mollusk, "commit:5", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "commit:5", "cu": cu, "result": "Success", "note": "slice-6 setup (repeat of the gated instruction)"}));
    let tr5 = accounts.iter().find(|(k, _)| *k == transition5).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(tr5.data[48], 3u8, "transition 5 is not COMMITTED");
    let ch = accounts.iter().find(|(k, _)| *k == chain).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ch.data[8..16].try_into().unwrap()), 5u64, "chain index not 5 after commit:5");
    assert_eq!(&ch.data[16..48], &[47u8; 32], "chain hash is not transition 5's declared hash");
    let st = accounts.iter().find(|(k, _)| *k == stock).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(st.data[10..18].try_into().unwrap()), 999_500u64, "stock not 999,500 after five 100 debits");

    // (l) schedule_whale_exit — holder-signed, 1,000,000 raw, on the
    // Committed exit transition 5, window 0, holder position
    // 10,000,000. The gate order passes state, exit type, window
    // binding, and positive total, then reaches the FLAG check:
    // FLAG = T/30 over snapshot 0 is zero, so this MUST refuse
    // with InvalidScheduleAmount — a zero-FLAG window sizes no
    // schedule at all. NOT a success CU: schedule success still
    // needs a funded snapshot (Carlos's funding-path decision).
    let mut data = disc("schedule_whale_exit");
    data.extend_from_slice(&5u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(holder, true),
            AccountMeta::new_readonly(transition5, false),
            AccountMeta::new_readonly(window_state, false),
            AccountMeta::new_readonly(holder_record, false),
            AccountMeta::new(exit_schedule, false),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "schedule_whale_exit_refused", &ix, &accounts, 6065);
    measured.push(serde_json::json!({
        "instruction": "schedule_whale_exit",
        "cu": cu,
        "result": "Refused",
        "error": "InvalidScheduleAmount",
        "error_code": 6065,
        "note": "slice-6 refusal probe: holder-signed 1,000,000 raw on Committed treasury.exit.v1 transition 5 (window 0, zero snapshot) — FLAG = T/30 over snapshot 0 is zero, so no schedule can be sized; NOT a success CU, schedule success still blocked on a funding path",
    }));
    // Post-refusal assertions: NOTHING moved. Holder state and the
    // exit transition are exactly as the slice-5 probes left them,
    // and no schedule account was created (its pre-seeded empty
    // slot is still empty).
    let hr = accounts.iter().find(|(k, _)| *k == holder_record).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr.data[56..64].try_into().unwrap()), 10_000_000u64, "holder position moved during schedule_whale_exit probe");
    assert_eq!(u64::from_le_bytes(hr.data[64..72].try_into().unwrap()), 6_260_000u64, "accrued dividend moved during schedule_whale_exit probe");
    assert_eq!(u64::from_le_bytes(hr.data[72..80].try_into().unwrap()), 0u64, "holder loan appeared during schedule_whale_exit probe");
    let tr5 = accounts.iter().find(|(k, _)| *k == transition5).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(tr5.data[48], 3u8, "transition 5 moved from COMMITTED during schedule_whale_exit probe");
    let es = accounts.iter().find(|(k, _)| *k == exit_schedule).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(es.data.len(), 0, "exit schedule account was created by a refused schedule_whale_exit");
    let ch = accounts.iter().find(|(k, _)| *k == chain).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ch.data[8..16].try_into().unwrap()), 5u64, "chain index moved during schedule_whale_exit probe");

    // ---- Slice 7: FUNDED chain — deposit + borrow / exit / settle_default / schedule_whale_exit SUCCESS CU ----
    // The funding gap that blocked every success measurement since
    // slice 5 is closed here by a REAL deposit: the SPL Token
    // program loaded above executes the vault creation
    // (system create + InitializeAccount) and the TransferChecked
    // CPI, and only then does the handler credit TreasuryState.
    // Nothing is seeded: the funded snapshot below is produced
    // by this deposit, and the slice-5/6 refusal values stand
    // untouched as the unfunded-chain record.
    //
    // Funded-chain numbers (all raw, all ratio-exact):
    //   deposit 60,000,000 -> T = 60,000,000, cash = 60,000,000
    //   window 1 snapshot: total 60,000,000, exit stock L = T/6
    //     = 10,000,000, book cap max_outstanding = T/12 = 5,000,000,
    //     whale FLAG = T/30 = 2,000,000
    //   holder window 1: position 30,000,000
    //   pay_tranche 1,000,000 -> accrued 1,000,000
    //   borrow 2,000,000 (<= 50% position 15,000,000; <= book cap
    //     5,000,000) -> maturity = borrow clock + 60d
    //   exit 1,500,000 = 1,000,000 accrued + 500,000 excess ->
    //     penalty 13% x 500,000 = 65,000, paid 1,435,000,
    //     position 29,500,000, exit stock 8,500,000
    //   settle_default at maturity -> position 27,500,000,
    //     loan 0, skip flag set, outstanding 0
    //   schedule 10,000,000 (>= FLAG 2,000,000, <= position) ->
    //     tranche min(20% x 10,000,000, FLAG) = 2,000,000, count 5,
    //     entry window 1, lock_until 13
    // execute_whale_tranche success is NOT measured here: its
    // gate needs an execution window >= lock_until (13), i.e. a
    // 12-window walk from window 1 — that walk is the next slice,
    // not hidden inside this one.
    let rent_id = Pubkey::from_str(RENT_ID_STR).unwrap();
    let depositor = Pubkey::new_from_array([9u8; 32]);
    let mint = Pubkey::new_from_array([10u8; 32]);
    let (vault, _) = pda(&[b"vault", mint.as_ref()], &program_id);
    let (window_state_1, _) = pda(&[b"window", &1u64.to_le_bytes()], &program_id);
    let (holder_record_1, _) = pda(&[b"holder", &1u64.to_le_bytes(), holder.as_ref()], &program_id);
    let (exit_schedule_1, _) = pda(&[b"schedule", &1u64.to_le_bytes(), holder.as_ref()], &program_id);
    let (observation6, _) = pda(&[b"observation", &6u64.to_le_bytes()], &program_id);
    let (transition6, _) = pda(&[b"transition", &6u64.to_le_bytes()], &program_id);
    let (plan6, _) = pda(&[b"plan", &6u64.to_le_bytes(), proposer.as_ref()], &program_id);
    let (signal6, signal6_bump) = pda(
        &[b"signal", &6u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
        &program_id,
    );
    let (evidence6, _) = pda(&[b"evidence", &6u64.to_le_bytes()], &program_id);
    let (observation7, _) = pda(&[b"observation", &7u64.to_le_bytes()], &program_id);
    let (transition7, _) = pda(&[b"transition", &7u64.to_le_bytes()], &program_id);
    let (plan7, _) = pda(&[b"plan", &7u64.to_le_bytes(), proposer.as_ref()], &program_id);
    let (signal7, signal7_bump) = pda(
        &[b"signal", &7u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
        &program_id,
    );
    let (observation8, _) = pda(&[b"observation", &8u64.to_le_bytes()], &program_id);
    let (transition8, _) = pda(&[b"transition", &8u64.to_le_bytes()], &program_id);
    let (plan8, _) = pda(&[b"plan", &8u64.to_le_bytes(), proposer.as_ref()], &program_id);
    let (signal8, signal8_bump) = pda(
        &[b"signal", &8u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
        &program_id,
    );
    // Token account layouts (SPL Token packed form):
    //   mint (82B): COption mint_authority [0..36] = None,
    //     supply [36..44], decimals [44] = 6, initialized [45] = 1,
    //     COption freeze_authority [46..82] = None.
    //   token account (165B): mint [0..32], owner [32..64],
    //     amount [64..72], COption delegate [72..108] = None,
    //     state [108] = 1 (Initialized), COption is_native
    //     [109..121] = None, delegated [121..129] = 0,
    //     COption close_authority [129..165] = None.
    let mut mint_data = vec![0u8; 82];
    mint_data[36..44].copy_from_slice(&1_000_000_000u64.to_le_bytes());
    mint_data[44] = 6;
    mint_data[45] = 1;
    let mint_account = Account {
        lamports: 1_000_000,
        data: mint_data,
        owner: token_program_id,
        executable: false,
        rent_epoch: 0,
    };
    let mut source_data = vec![0u8; 165];
    source_data[0..32].copy_from_slice(mint.as_ref());
    source_data[32..64].copy_from_slice(depositor.as_ref());
    source_data[64..72].copy_from_slice(&100_000_000u64.to_le_bytes());
    source_data[108] = 1;
    let source_account = Account {
        lamports: 1_000_000,
        data: source_data,
        owner: token_program_id,
        executable: false,
        rent_epoch: 0,
    };
    let token_program_account = Account {
        lamports: 1,
        data: vec![],
        owner: Pubkey::from_str(UPGRADEABLE_LOADER_ID_STR).unwrap(),
        executable: true,
        rent_epoch: 0,
    };
    upsert(&mut accounts, (depositor, funded()));
    upsert(&mut accounts, (mint, mint_account));
    upsert(&mut accounts, (token_program_id, token_program_account));
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_rent_sysvar());
    // The source account must be threaded by key: the deposit
    // instruction names it by its own key, not by a PDA.
    let source_key = Pubkey::new_from_array([11u8; 32]);
    upsert(&mut accounts, (source_key, source_account));
    upsert(&mut accounts, (vault, empty.clone()));
    upsert(&mut accounts, (window_state_1, empty.clone()));
    upsert(&mut accounts, (holder_record_1, empty.clone()));
    upsert(&mut accounts, (exit_schedule_1, empty.clone()));
    upsert(&mut accounts, (observation6, empty.clone()));
    upsert(&mut accounts, (transition6, empty.clone()));
    upsert(&mut accounts, (plan6, empty.clone()));
    upsert(&mut accounts, (signal6, empty.clone()));
    upsert(&mut accounts, (evidence6, empty.clone()));
    upsert(&mut accounts, (observation7, empty.clone()));
    upsert(&mut accounts, (transition7, empty.clone()));
    upsert(&mut accounts, (plan7, empty.clone()));
    upsert(&mut accounts, (signal7, empty.clone()));
    upsert(&mut accounts, (observation8, empty.clone()));
    upsert(&mut accounts, (transition8, empty.clone()));
    upsert(&mut accounts, (plan8, empty.clone()));
    upsert(&mut accounts, (signal8, empty.clone()));

    // (m0) slice-20 registry prelude: the deposit mint registers
    // BEFORE the first deposit, because slice 20 makes the
    // registry fail-closed — an empty registry accepts nothing,
    // so the slice-7 deposit below would refuse 6100 without
    // this step. Authority-signed under the §4.1 Admin pattern
    // (config.authority == [1;32], the genesis authority).
    let mut data20pre = disc("register_mint");
    data20pre.extend_from_slice(mint.as_ref());
    let ix20pre = Instruction::new_with_bytes(
        program_id,
        &data20pre,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(treasury_state, false),
            AccountMeta::new(authority, true),
        ],
    );
    let (next, cu) = run(&mollusk, "register_mint:20prelude", &ix20pre, &accounts);
    accounts = next;
    measured.push(serde_json::json!({
        "instruction": "register_mint:20prelude",
        "cu": cu,
        "result": "Success",
        "note": "slice-20 prelude: genesis authority registers the deposit mint ([10;32]) into the accepted-mints registry — the fail-closed gate the slice-7 deposit then passes; without this step that deposit would refuse MintNotAccepted (6100)",
    }));
    let ts20pre = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(ts20pre.data[65], 1u8, "registry count not 1 after register_mint:20prelude");
    assert_eq!(&ts20pre.data[66..98], mint.as_ref(), "registry slot 0 is not the deposit mint after register_mint:20prelude");

    // (m) deposit — THE GATED-CLASS FIRST: the first real funding
    // credit on-chain. No §2.2 EST exists for deposit, so this is
    // recorded as baseline, not gated — but it is a real SPL CPI
    // chain (vault create + InitializeAccount + TransferChecked),
    // and its CU is the deposit cost against the real token
    // program, not a host estimate.
    let mut data = disc("deposit");
    data.extend_from_slice(&60_000_000u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(treasury_state, false),
            AccountMeta::new(depositor, true),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(source_key, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(token_program_id, false),
            AccountMeta::new_readonly(rent_id, false),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "deposit", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({
        "instruction": "deposit",
        "cu": cu,
        "result": "Success",
        "note": "slice-7 FIRST success measurement: real SPL Token CPI chain (vault create + InitializeAccount + TransferChecked, 60,000,000 raw, 6 decimals) credits TreasuryState; baseline (no §2.2 EST exists for deposit)",
    }));
    let ts = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts.data[8..16].try_into().unwrap()), 60_000_000u64, "TreasuryState total not 60,000,000 after deposit");
    assert_eq!(u64::from_le_bytes(ts.data[16..24].try_into().unwrap()), 60_000_000u64, "TreasuryState cash not 60,000,000 after deposit");
    assert_eq!(u64::from_le_bytes(ts.data[56..64].try_into().unwrap()), 1u64, "window_count moved during deposit");
    let va = accounts.iter().find(|(k, _)| *k == vault).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(va.owner, token_program_id, "vault owner is not the token program after deposit");
    assert_eq!(u64::from_le_bytes(va.data[64..72].try_into().unwrap()), 60_000_000u64, "vault token amount not 60,000,000 after deposit");
    assert_eq!(&va.data[32..64], treasury_state.as_ref(), "vault owner is not the TreasuryState PDA after deposit");
    let sa = accounts.iter().find(|(k, _)| *k == source_key).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(sa.data[64..72].try_into().unwrap()), 40_000_000u64, "source token amount not 40,000,000 after deposit (conservation: 100,000,000 - 60,000,000)");
    println!("deposit vault lamports={} (rent-exact funding for 165 bytes)", va.lamports);

    // (n) Funded window-open lifecycle, transition 6 (window SHA
    // policy), parent = chain head (5, [47;32]), through settle.
    accounts = lifecycle_to_commit(
        &mut mollusk, accounts, &mut measured,
        program_id, clock_id, system_id,
        oracle_record, oracle, proposer_record, proposer, evaluator_record, evaluator,
        chain, stock, policy_sha,
        observation6, transition6, plan6, signal6, signal6_bump,
        6, NOW6 + 1_000_000, NOW6 + 2_000_000, [57u8; 32], 5, [47u8; 32],
        [9u8; 32], NOW6, ":6", Some(evidence6),
    );
    let ch = accounts.iter().find(|(k, _)| *k == chain).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ch.data[8..16].try_into().unwrap()), 6u64, "chain index not 6 after commit:6");
    assert_eq!(&ch.data[16..48], &[57u8; 32], "chain hash is not transition 6's declared hash");
    let st = accounts.iter().find(|(k, _)| *k == stock).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(st.data[10..18].try_into().unwrap()), 999_400u64, "stock not 999,400 after six 100 debits");

    // (o) open_window 1 on settled transition 6: the snapshot is
    // the FUNDED total, not zero — this is the measurement the
    // funding gap blocked.
    let mut data = disc("open_window");
    data.extend_from_slice(&6u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(treasury_state, false),
            AccountMeta::new_readonly(lending_book, false),
            AccountMeta::new(transition6, false),
            AccountMeta::new(window_state_1, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "open_window:1", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "open_window:1", "cu": cu, "result": "Success", "note": "slice-7 setup: window 1 opens on the funded snapshot (total 60,000,000)"}));
    let ts = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts.data[56..64].try_into().unwrap()), 2u64, "window_count not 2 after open_window:1");
    let ws1 = accounts.iter().find(|(k, _)| *k == window_state_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws1.data[16..24].try_into().unwrap()), 60_000_000u64, "window 1 snapshot total not 60,000,000");
    assert_eq!(u64::from_le_bytes(ws1.data[64..72].try_into().unwrap()), 10_000_000u64, "window 1 exit stock not L = 10,000,000");
    assert_eq!(u64::from_le_bytes(ws1.data[72..80].try_into().unwrap()), 6u64, "window 1 source transition not 6");

    // (p) enroll the holder into window 1 (cohort 1, position
    // 30,000,000). Next-window-only entry: window 1 is the
    // currently open window (counter 2, current = 1).
    let mut data = disc("enroll");
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&30_000_000u64.to_le_bytes());
    let (forfeiture_holder, _) = pda(&[b"forfeiture", holder.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder) { accounts.push((forfeiture_holder, empty.clone())); }
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_1, false),
            AccountMeta::new(holder_record_1, false),
            AccountMeta::new(forfeiture_holder, false),
            AccountMeta::new(holder, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll:1", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "enroll:1", "cu": cu, "result": "Success", "note": "slice-7 setup: holder enrolls in funded window 1 (cohort 1, position 30,000,000)"}));
    let hr1 = accounts.iter().find(|(k, _)| *k == holder_record_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr1.data[48..56].try_into().unwrap()), 1u64, "holder window-1 cohort not 1");
    assert_eq!(u64::from_le_bytes(hr1.data[56..64].try_into().unwrap()), 30_000_000u64, "holder window-1 position not 30,000,000");
    assert_eq!(hr1.data[89], 1u8, "standing authorization not true at enroll:1");

    // (q) Funded window-flow lifecycle, transition 7 (window SHA
    // policy), parent = chain head (6, [57;32]), stopped at
    // commit — executions ride commit.
    accounts = lifecycle_to_commit(
        &mut mollusk, accounts, &mut measured,
        program_id, clock_id, system_id,
        oracle_record, oracle, proposer_record, proposer, evaluator_record, evaluator,
        chain, stock, policy_sha,
        observation7, transition7, plan7, signal7, signal7_bump,
        7, NOW7 + 1_000_000, NOW7 + 2_000_000, [67u8; 32], 6, [57u8; 32],
        [9u8; 32], NOW7, ":7", None,
    );
    let ch = accounts.iter().find(|(k, _)| *k == chain).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ch.data[8..16].try_into().unwrap()), 7u64, "chain index not 7 after commit:7");
    assert_eq!(&ch.data[16..48], &[67u8; 32], "chain hash is not transition 7's declared hash");

    // (r) pay_tranche on transition 7, window 1, 1,000,000 —
    // repeat of the slice-4 gated instruction on the funded
    // window (cohort 1 holder, operator-settled).
    let mut data = disc("pay_tranche");
    data.extend_from_slice(&7u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(holder.as_ref());
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_1, false),
            AccountMeta::new(holder_record_1, false),
        ],
    );
    let (next, cu) = run(&mollusk, "pay_tranche:1", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "pay_tranche:1", "cu": cu, "result": "Success", "note": "slice-7 setup: operator-settled tranche on funded window 1 (repeat of the gated instruction)"}));
    let hr1 = accounts.iter().find(|(k, _)| *k == holder_record_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr1.data[64..72].try_into().unwrap()), 1_000_000u64, "accrued dividend not 1,000,000 after pay_tranche:1");
    assert_eq!(hr1.data[98], 1u8, "tranche_paid flag not set after pay_tranche:1");

    // (s) borrow — FIRST SUCCESS MEASUREMENT (gated §2.2 ~7K,
    // 2x band 3,500–14,000). Clock is NOW7+3600 = 7,003,600, so
    // maturity = 7,003,600 + 60d = 12,187,600.
    let mut data = disc("borrow");
    data.extend_from_slice(&7u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&2_000_000u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(holder, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_1, false),
            AccountMeta::new(holder_record_1, false),
            AccountMeta::new(lending_book, false),
        ],
    );
    let (next, cu) = run(&mollusk, "borrow", &ix, &accounts);
    accounts = next;
    let gate = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "borrow",
        "cu": cu,
        "result": "Success",
        "note": "slice-7 GATED measurement: holder-signed 2,000,000 raw on Committed window flow transition 7 (funded window 1: snapshot 60,000,000, book cap 5,000,000, per-holder cap 15,000,000)",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate,
    }));
    let hr1 = accounts.iter().find(|(k, _)| *k == holder_record_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr1.data[72..80].try_into().unwrap()), 2_000_000u64, "holder loan not 2,000,000 after borrow");
    assert_eq!(i64::from_le_bytes(hr1.data[80..88].try_into().unwrap()), 12_187_600i64, "loan maturity not 12,187,600 after borrow");
    let lb = accounts.iter().find(|(k, _)| *k == lending_book).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(lb.data[8..16].try_into().unwrap()), 2_000_000u64, "lending book outstanding not 2,000,000 after borrow");
    assert_eq!(u32::from_le_bytes(lb.data[16..20].try_into().unwrap()), 1u32, "lending book active loans not 1 after borrow");

    // (t) exit — FIRST SUCCESS MEASUREMENT (gated §2.2 ~7K,
    // 2x band 3,500–14,000). 1,500,000 = 1,000,000 accrued at
    // par + 500,000 excess at the flat 13% door: penalty
    // 65,000, paid 1,435,000; position 29,500,000 stays above
    // the 2,000,000 loan; exit stock 10,000,000 -> 8,500,000.
    let mut data = disc("exit");
    data.extend_from_slice(&7u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&1_500_000u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(holder, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new(window_state_1, false),
            AccountMeta::new(holder_record_1, false),
        ],
    );
    let (next, cu) = run(&mollusk, "exit", &ix, &accounts);
    accounts = next;
    let gate = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "exit",
        "cu": cu,
        "result": "Success",
        "note": "slice-7 GATED measurement: holder-signed 1,500,000 raw on Committed window flow transition 7 (funded window 1: 1,000,000 accrued at par + 500,000 excess at flat 13% = 65,000 penalty, 1,435,000 paid; D-10 per-order bound L = 10,000,000)",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate,
    }));
    let hr1 = accounts.iter().find(|(k, _)| *k == holder_record_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr1.data[64..72].try_into().unwrap()), 0u64, "accrued dividend not consumed by exit");
    assert_eq!(u64::from_le_bytes(hr1.data[56..64].try_into().unwrap()), 29_500_000u64, "position not 29,500,000 after exit");
    let ws1 = accounts.iter().find(|(k, _)| *k == window_state_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws1.data[64..72].try_into().unwrap()), 8_500_000u64, "window 1 exit stock not 8,500,000 after exit");

    // (u) settle_default — FIRST SUCCESS MEASUREMENT (gated
    // §2.2 ~7K, 2x band 3,500–14,000). Clock advances to the
    // loan maturity (12,187,600); transition 7 is still
    // Committed, so it still authorizes the settlement, and
    // the window binding is the loan's window (1), not a
    // later window's transition. Position 29,500,000 -
    // 2,000,000 = 27,500,000; skip-next-dividend set (D-3,
    // pre-default base 29,500,000 carried on the event).
    mollusk.sysvars.clock.unix_timestamp = 12_187_600;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let mut data = disc("settle_default");
    data.extend_from_slice(&7u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(holder.as_ref());
    let (forfeiture_holder, _) = pda(&[b"forfeiture", holder.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder) { accounts.push((forfeiture_holder, empty.clone())); }
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_1, false),
            AccountMeta::new(holder_record_1, false),
            AccountMeta::new(lending_book, false),
            AccountMeta::new(forfeiture_holder, false),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "settle_default", &ix, &accounts);
    accounts = next;
    let gate = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "settle_default",
        "cu": cu,
        "result": "Success",
        "note": "slice-7 GATED measurement: operator-settled default at maturity 12,187,600 on Committed window flow transition 7 (funded window 1: principal 2,000,000 settles against position, interest forgiven, skip-next-dividend set on the pre-default base)",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate,
    }));
    let hr1 = accounts.iter().find(|(k, _)| *k == holder_record_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr1.data[56..64].try_into().unwrap()), 27_500_000u64, "position not 27,500,000 after settle_default");
    assert_eq!(u64::from_le_bytes(hr1.data[72..80].try_into().unwrap()), 0u64, "loan not cleared by settle_default");
    assert_eq!(i64::from_le_bytes(hr1.data[80..88].try_into().unwrap()), 0i64, "maturity not cleared by settle_default");
    assert_eq!(hr1.data[88], 1u8, "skip-next-dividend flag not set by settle_default");
    let lb = accounts.iter().find(|(k, _)| *k == lending_book).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(lb.data[8..16].try_into().unwrap()), 0u64, "lending book outstanding not 0 after settle_default");
    assert_eq!(u32::from_le_bytes(lb.data[16..20].try_into().unwrap()), 0u32, "lending book active loans not 0 after settle_default");

    // (v) Funded exit lifecycle, transition 8 (exit policy),
    // parent = chain head (7, [67;32]), stopped at commit.
    accounts = lifecycle_to_commit(
        &mut mollusk, accounts, &mut measured,
        program_id, clock_id, system_id,
        oracle_record, oracle, proposer_record, proposer, evaluator_record, evaluator,
        chain, stock, policy_exit,
        observation8, transition8, plan8, signal8, signal8_bump,
        8, NOW8 + 1_000_000, NOW8 + 2_000_000, [77u8; 32], 7, [67u8; 32],
        [13u8; 32], NOW8, ":8", None,
    );
    let ch = accounts.iter().find(|(k, _)| *k == chain).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ch.data[8..16].try_into().unwrap()), 8u64, "chain index not 8 after commit:8");
    assert_eq!(&ch.data[16..48], &[77u8; 32], "chain hash is not transition 8's declared hash");

    // (w) schedule_whale_exit — FIRST SUCCESS MEASUREMENT
    // (gated §2.2 ~7K, 2x band 3,500–14,000). Total
    // 10,000,000 >= FLAG 2,000,000 and <= position
    // 27,500,000: tranche min(20% x 10,000,000, FLAG) =
    // 2,000,000, count 5, entry window 1, lock_until 13.
    // Execution itself waits for the next slice's window walk.
    let mut data = disc("schedule_whale_exit");
    data.extend_from_slice(&8u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&10_000_000u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(holder, true),
            AccountMeta::new_readonly(transition8, false),
            AccountMeta::new_readonly(window_state_1, false),
            AccountMeta::new_readonly(holder_record_1, false),
            AccountMeta::new(exit_schedule_1, false),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "schedule_whale_exit", &ix, &accounts);
    accounts = next;
    let gate = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "schedule_whale_exit",
        "cu": cu,
        "result": "Success",
        "note": "slice-7 GATED measurement: holder-signed 10,000,000 raw schedule on Committed treasury.exit.v1 transition 8 (funded window 1: FLAG 2,000,000, tranche 2,000,000, count 5, entry window 1, lock_until 13; execution rides the next slice's window walk)",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate,
    }));
    let es1 = accounts.iter().find(|(k, _)| *k == exit_schedule_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(es1.data[48..56].try_into().unwrap()), 10_000_000u64, "schedule total not 10,000,000");
    assert_eq!(u64::from_le_bytes(es1.data[56..64].try_into().unwrap()), 2_000_000u64, "schedule tranche not 2,000,000");
    assert_eq!(u64::from_le_bytes(es1.data[64..72].try_into().unwrap()), 5u64, "schedule tranche count not 5");
    assert_eq!(u64::from_le_bytes(es1.data[72..80].try_into().unwrap()), 0u64, "schedule executed count not 0 at creation");
    assert_eq!(u64::from_le_bytes(es1.data[80..88].try_into().unwrap()), 1u64, "schedule entry window not 1");
    assert_eq!(u64::from_le_bytes(es1.data[88..96].try_into().unwrap()), 13u64, "schedule lock_until window not 13");
    assert_eq!(u64::from_le_bytes(es1.data[96..104].try_into().unwrap()), 0u64, "schedule last_block_time not 0 at creation");
    // Slice-7 final state: the funded chain is intact — the
    // slice-4/5/6 unfunded chain is untouched (window 0
    // snapshot still 0, holder window-0 record unchanged),
    // and the funded window-1 chain carries its own state.
    let hr0 = accounts.iter().find(|(k, _)| *k == holder_record).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr0.data[56..64].try_into().unwrap()), 10_000_000u64, "window-0 holder position moved during slice 7");
    let ws0 = accounts.iter().find(|(k, _)| *k == window_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws0.data[16..24].try_into().unwrap()), 0u64, "window-0 snapshot moved during slice 7");

    // ---- Slice 8: 12-window walk -> execute_whale_tranche SUCCESS CU ----
    // The slice-7 schedule (entry window 1, lock_until 13, tranche
    // 2,000,000, count 5) cannot execute until an execution window
    // >= 13 exists. Each new window needs its OWN Settled
    // treasury.window.v1 transition as its open source
    // (open_window requires window_id == window_count, so windows
    // open strictly in sequence; a single transition authorizes
    // exactly one window open because each open consumes the
    // window_count slot, and no transition is reused here).
    // Windows 2..13 ride transitions 9..20, each a full lifecycle
    // through settle on the window-SHA policy, parented on the
    // previous transition's declared hash (transition 9 parents on
    // head 8, [77;32]). Planning times step 1,000,000s from
    // 20,000,000 — harness parameters, not design values. Every
    // window opens on the same funded snapshot (total 60,000,000,
    // exit stock L = 10,000,000, FLAG = 2,000,000), because no
    // further deposit lands in this chain.
    // The tranche gate then reads: Committed exit transition 8,
    // creation window 1 (schedule + holder record), execution
    // window 13 (>= lock_until 13), execution window 13's own
    // exit stock (10,000,000) and FLAG (2,000,000) as the bound's
    // basis, position 27,500,000 after slice-7 settle_default,
    // loan 0, first tranche (no weekly-cadence check yet).
    let mut parent_index: u64 = 8;
    let mut parent_hash: [u8; 32] = [77u8; 32];
    for w in 2..=13u64 {
        let tid = w + 7;
        let (observation_w, _) = pda(&[b"observation", &tid.to_le_bytes()], &program_id);
        let (transition_w, _) = pda(&[b"transition", &tid.to_le_bytes()], &program_id);
        let (plan_w, _) = pda(&[b"plan", &tid.to_le_bytes(), proposer.as_ref()], &program_id);
        let (signal_w, signal_w_bump) = pda(
            &[b"signal", &tid.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
            &program_id,
        );
        let (evidence_w, _) = pda(&[b"evidence", &tid.to_le_bytes()], &program_id);
        let (window_state_w, _) = pda(&[b"window", &w.to_le_bytes()], &program_id);
        upsert(&mut accounts, (observation_w, empty.clone()));
        upsert(&mut accounts, (transition_w, empty.clone()));
        upsert(&mut accounts, (plan_w, empty.clone()));
        upsert(&mut accounts, (signal_w, empty.clone()));
        upsert(&mut accounts, (evidence_w, empty.clone()));
        upsert(&mut accounts, (window_state_w, empty.clone()));
        let now_w: i64 = 20_000_000 + ((w - 2) as i64) * 1_000_000;
        let declared_w: [u8; 32] = [((87 + (w - 2) * 10) as u8); 32];
        accounts = lifecycle_to_commit(
            &mut mollusk, accounts, &mut measured,
            program_id, clock_id, system_id,
            oracle_record, oracle, proposer_record, proposer, evaluator_record, evaluator,
            chain, stock, policy_sha,
            observation_w, transition_w, plan_w, signal_w, signal_w_bump,
            tid, now_w + 1_000_000, now_w + 2_000_000, declared_w, parent_index, parent_hash,
            [9u8; 32], now_w, &format!(":{tid}"), Some(evidence_w),
        );
        let ch = accounts.iter().find(|(k, _)| *k == chain).map(|(_, a)| a.clone()).unwrap();
        assert_eq!(u64::from_le_bytes(ch.data[8..16].try_into().unwrap()), tid, "chain index not {tid} after commit:{tid}");
        assert_eq!(&ch.data[16..48], &declared_w, "chain hash is not transition {tid}'s declared hash");
        let mut data = disc("open_window");
        data.extend_from_slice(&tid.to_le_bytes());
        data.extend_from_slice(&w.to_le_bytes());
        let ix = Instruction::new_with_bytes(
            program_id,
            &data,
            vec![
                AccountMeta::new(treasury_state, false),
                AccountMeta::new_readonly(lending_book, false),
                AccountMeta::new(transition_w, false),
                AccountMeta::new(window_state_w, false),
                AccountMeta::new(authority, true),
                AccountMeta::new_readonly(system_id, false),
            ],
        );
        let (next, cu) = run(&mollusk, &format!("open_window:{w}"), &ix, &accounts);
        accounts = next;
        measured.push(serde_json::json!({"instruction": format!("open_window:{w}"), "cu": cu, "result": "Success", "note": format!("slice-8 setup: window {w} opens on Settled transition {tid} (funded snapshot total 60,000,000, exit stock 10,000,000)")}));
        let ts = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
        assert_eq!(u64::from_le_bytes(ts.data[56..64].try_into().unwrap()), w + 1, "window_count not {w} + 1 after open_window:{w}");
        let wsw = accounts.iter().find(|(k, _)| *k == window_state_w).map(|(_, a)| a.clone()).unwrap();
        assert_eq!(u64::from_le_bytes(wsw.data[16..24].try_into().unwrap()), 60_000_000u64, "window {w} snapshot total not 60,000,000");
        assert_eq!(u64::from_le_bytes(wsw.data[64..72].try_into().unwrap()), 10_000_000u64, "window {w} exit stock not 10,000,000");
        assert_eq!(u64::from_le_bytes(wsw.data[72..80].try_into().unwrap()), tid, "window {w} source transition not {tid}");
        parent_index = tid;
        parent_hash = declared_w;
    }

    // (x) execute_whale_tranche — FIRST SUCCESS MEASUREMENT (gated
    // §2.2 ~7K, 2x band 3,500–14,000). Operator-settled, on
    // Committed exit transition 8, creation window 1, execution
    // window 13: tranche 2,000,000, exactly the FLAG over window
    // 13's snapshot. Clock at execution is the last walk
    // lifecycle's close (31,000,000 + 3,600 = 31,003,600), which
    // becomes the schedule's last_block_time for the weekly
    // cadence on tranche 2 (not measured here).
    let (window_state_13, _) = pda(&[b"window", &13u64.to_le_bytes()], &program_id);
    let mut data = disc("execute_whale_tranche");
    data.extend_from_slice(&8u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&13u64.to_le_bytes());
    data.extend_from_slice(holder.as_ref());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition8, false),
            AccountMeta::new(window_state_13, false),
            AccountMeta::new(holder_record_1, false),
            AccountMeta::new(exit_schedule_1, false),
        ],
    );
    let (next, cu) = run(&mollusk, "execute_whale_tranche", &ix, &accounts);
    accounts = next;
    let gate = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "execute_whale_tranche",
        "cu": cu,
        "result": "Success",
        "note": "slice-8 GATED measurement: operator-settled tranche 1 of 5 (2,000,000 raw) on Committed treasury.exit.v1 transition 8 (creation window 1, execution window 13 >= lock_until 13, execution window 13 snapshot 60,000,000 / FLAG 2,000,000, position 27,500,000, loan 0)",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate,
    }));
    let es1 = accounts.iter().find(|(k, _)| *k == exit_schedule_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(es1.data[72..80].try_into().unwrap()), 1u64, "schedule tranches_executed not 1 after execute_whale_tranche");
    assert_eq!(i64::from_le_bytes(es1.data[96..104].try_into().unwrap()), 31_003_600i64, "schedule last_block_time not 31,003,600 after execute_whale_tranche");
    assert_eq!(u64::from_le_bytes(es1.data[48..56].try_into().unwrap()), 10_000_000u64, "schedule total moved during execute_whale_tranche");
    assert_eq!(u64::from_le_bytes(es1.data[64..72].try_into().unwrap()), 5u64, "schedule tranche count moved during execute_whale_tranche");
    let hr1 = accounts.iter().find(|(k, _)| *k == holder_record_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr1.data[56..64].try_into().unwrap()), 25_500_000u64, "position not 25,500,000 after execute_whale_tranche");
    let ws13 = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws13.data[64..72].try_into().unwrap()), 8_000_000u64, "window 13 exit stock not 8,000,000 after execute_whale_tranche");
    let ws1 = accounts.iter().find(|(k, _)| *k == window_state_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws1.data[64..72].try_into().unwrap()), 8_500_000u64, "window 1 exit stock moved during execute_whale_tranche (execution debits the execution window only)");

    // ---- Slice 9: W3.2 re-expression, part 1 — tranche-2 weekly cadence ----
    // The D-10 revised contract pays a whale schedule in weekly blocks:
    // tranche 2 may execute only at now >= last_block_time + WEEK_SECS
    // (604,800s), the gate inclusive (`now >= earliest`). Three
    // measurements on the slice-8 chain, all on execution window 13
    // (13 >= lock_until 13; window-13 exit stock 8,000,000 after
    // tranche 1 still covers a 2,000,000 block) under the same
    // Committed exit transition 8:
    // (a) too-soon probe at the tranche-1 clock (31,003,600 ==
    // last_block_time, a full week short of the earliest lawful
    // block) MUST refuse TrancheTooSoon (6069);
    // (b) boundary-minus-one probe at 31,608,399 (one second short)
    // MUST refuse TrancheTooSoon — the boundary is exact, not
    // approximate;
    // (c) success at exactly 31,608,400, gated §2.2 ~7K (2x band
    // 3,500–14,000) like tranche 1. Refusals change no state
    // (run_expect_err keeps the pre-state); the success asserts
    // tranches_executed 2, last_block_time 31,608,400, position
    // 23,500,000, window-13 exit stock 6,000,000, window-1 stock
    // untouched at 8,500,000.
    let mut data9 = disc("execute_whale_tranche");
    data9.extend_from_slice(&8u64.to_le_bytes());
    data9.extend_from_slice(&1u64.to_le_bytes());
    data9.extend_from_slice(&13u64.to_le_bytes());
    data9.extend_from_slice(holder.as_ref());
    let ix9 = Instruction::new_with_bytes(
        program_id,
        &data9,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition8, false),
            AccountMeta::new(window_state_13, false),
            AccountMeta::new(holder_record_1, false),
            AccountMeta::new(exit_schedule_1, false),
        ],
    );
    // (a) too-soon at the tranche-1 clock.
    mollusk.sysvars.clock.unix_timestamp = 31_003_600;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let cu = run_expect_err(&mollusk, "execute_whale_tranche:too_soon", &ix9, &accounts, 6069);
    measured.push(serde_json::json!({
        "instruction": "execute_whale_tranche:too_soon",
        "cu": cu,
        "result": "Refused",
        "error": "TrancheTooSoon",
        "error_code": 6069,
        "note": "slice-9 W3.2 re-expression: tranche 2 attempted at the tranche-1 clock (31,003,600 == last_block_time, 604,800s short of the earliest lawful block) — weekly cadence refuses, NOT a success CU",
    }));
    // (b) boundary minus one second.
    mollusk.sysvars.clock.unix_timestamp = 31_608_399;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let cu = run_expect_err(&mollusk, "execute_whale_tranche:too_soon_boundary_minus_1", &ix9, &accounts, 6069);
    measured.push(serde_json::json!({
        "instruction": "execute_whale_tranche:too_soon_boundary_minus_1",
        "cu": cu,
        "result": "Refused",
        "error": "TrancheTooSoon",
        "error_code": 6069,
        "note": "slice-9 W3.2 re-expression: tranche 2 attempted at last_block_time + WEEK_SECS - 1 (31,608,399) — the cadence boundary is exact, one second short still refuses, NOT a success CU",
    }));
    // Post-refusal assertions: NOTHING moved. Schedule still 1
    // executed block, last_block_time still the tranche-1 clock,
    // position and both window stocks unchanged.
    let es9r = accounts.iter().find(|(k, _)| *k == exit_schedule_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(es9r.data[72..80].try_into().unwrap()), 1u64, "schedule tranches_executed moved during tranche-2 refusal probes");
    assert_eq!(i64::from_le_bytes(es9r.data[96..104].try_into().unwrap()), 31_003_600i64, "schedule last_block_time moved during tranche-2 refusal probes");
    let hr9r = accounts.iter().find(|(k, _)| *k == holder_record_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr9r.data[56..64].try_into().unwrap()), 25_500_000u64, "position moved during tranche-2 refusal probes");
    let ws13r = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws13r.data[64..72].try_into().unwrap()), 8_000_000u64, "window 13 exit stock moved during tranche-2 refusal probes");
    // (c) success at exactly the boundary: last_block_time + WEEK_SECS.
    mollusk.sysvars.clock.unix_timestamp = 31_608_400;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let (next, cu) = run(&mollusk, "execute_whale_tranche:tranche2", &ix9, &accounts);
    accounts = next;
    let gate9 = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "execute_whale_tranche:tranche2",
        "cu": cu,
        "result": "Success",
        "note": "slice-9 GATED measurement (W3.2 re-expression): tranche 2 of 5 (2,000,000 raw) at exactly last_block_time + WEEK_SECS (31,608,400), on Committed treasury.exit.v1 transition 8, creation window 1, execution window 13 >= lock_until 13, execution window 13 snapshot 60,000,000 / FLAG 2,000,000, position 25,500,000, loan 0",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate9,
    }));
    let es9 = accounts.iter().find(|(k, _)| *k == exit_schedule_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(es9.data[72..80].try_into().unwrap()), 2u64, "schedule tranches_executed not 2 after execute_whale_tranche:tranche2");
    assert_eq!(i64::from_le_bytes(es9.data[96..104].try_into().unwrap()), 31_608_400i64, "schedule last_block_time not 31,608,400 after execute_whale_tranche:tranche2");
    assert_eq!(u64::from_le_bytes(es9.data[48..56].try_into().unwrap()), 10_000_000u64, "schedule total moved during execute_whale_tranche:tranche2");
    assert_eq!(u64::from_le_bytes(es9.data[64..72].try_into().unwrap()), 5u64, "schedule tranche count moved during execute_whale_tranche:tranche2");
    let hr9 = accounts.iter().find(|(k, _)| *k == holder_record_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr9.data[56..64].try_into().unwrap()), 23_500_000u64, "position not 23,500,000 after execute_whale_tranche:tranche2");
    let ws13_9 = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws13_9.data[64..72].try_into().unwrap()), 6_000_000u64, "window 13 exit stock not 6,000,000 after execute_whale_tranche:tranche2");
    let ws1_9 = accounts.iter().find(|(k, _)| *k == window_state_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws1_9.data[64..72].try_into().unwrap()), 8_500_000u64, "window 1 exit stock moved during execute_whale_tranche:tranche2 (execution debits the execution window only)");

    // ---- Slice 10: W3.2 re-expression, part 2 — blind-ape bulk exit vs the flat 13% door ----
    // Engine-sim T11 extracts the whole position in one go above the
    // per-period allowance and prices the flat 13% penalty on the
    // excess. On the program that whole-position order cannot even
    // be placed: the D-10 hard bound (no single order above one
    // period's movable liquidity L = T/6, judged on the window's
    // period-start snapshot) refuses it before any position or
    // stock check. This slice measures, on the slice-9 chain
    // (funded window 1: snapshot 60,000,000, L = 10,000,000;
    // holder window 1: position 23,500,000 after tranche 2,
    // accrued dividend 0, loan 0; window-1 exit stock 8,500,000
    // after the slice-7 exit of 1,500,000; Committed window-flow
    // transition 7 still authorizes exits):
    // (a) the ape's whole-position order (23,500,000) MUST refuse
    // ExitExceedsLiquidity (6060) — 23,500,000 > L = 10,000,000;
    // (b) the largest executable bulk exit — the entire remaining
    // window-1 stock, 8,500,000, below L — SUCCEEDS, gated §2.2
    // ~7K (2x band 3,500–14,000) like the slice-7 exit: accrued 0,
    // so the whole amount is excess at the flat 13% door, penalty
    // 1,105,000, paid 7,395,000, position 23,500,000 -> 15,000,000,
    // window-1 stock 8,500,000 -> 0;
    // (c) any further window-1 exit MUST refuse ExitStockExhausted
    // (6063) — the window's aggregate exits now total exactly L
    // (slice-7 1,500,000 + this 8,500,000 = 10,000,000), so
    // repeated sub-L exits cannot drain more than one period's
    // capacity in total. The ape's ledger on this record:
    // position 15,000,000 + paid 7,395,000 = 22,395,000 against
    // 23,500,000 before the bulk exit — 1,105,000 worse at once,
    // before any future penalty-free allowances abandoned by
    // rushing. As in every slice, `exit` here is bookkeeping
    // (accrued consumed, position reduced, stock debited); it
    // does NOT move tokens — the disbursement step is separate.
    // (a) whole-position order: refused by the hard bound.
    let mut data10 = disc("exit");
    data10.extend_from_slice(&7u64.to_le_bytes());
    data10.extend_from_slice(&1u64.to_le_bytes());
    data10.extend_from_slice(&23_500_000u64.to_le_bytes());
    let ix10a = Instruction::new_with_bytes(
        program_id,
        &data10,
        vec![
            AccountMeta::new(holder, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new(window_state_1, false),
            AccountMeta::new(holder_record_1, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "exit:bulk_over_L", &ix10a, &accounts, 6060);
    measured.push(serde_json::json!({
        "instruction": "exit:bulk_over_L",
        "cu": cu,
        "result": "Refused",
        "error": "ExitExceedsLiquidity",
        "error_code": 6060,
        "note": "slice-10 W3.2 re-expression (T11): the blind ape's whole-position order (23,500,000 raw) on Committed window flow transition 7, funded window 1 (snapshot 60,000,000, L = 10,000,000) — the D-10 hard bound refuses the order itself, before any position or stock check, NOT a success CU",
    }));
    let hr10r = accounts.iter().find(|(k, _)| *k == holder_record_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr10r.data[56..64].try_into().unwrap()), 23_500_000u64, "position moved during exit:bulk_over_L refusal");
    assert_eq!(u64::from_le_bytes(hr10r.data[64..72].try_into().unwrap()), 0u64, "accrued dividend moved during exit:bulk_over_L refusal");
    let ws1_10r = accounts.iter().find(|(k, _)| *k == window_state_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws1_10r.data[64..72].try_into().unwrap()), 8_500_000u64, "window 1 exit stock moved during exit:bulk_over_L refusal");
    // (b) largest executable bulk exit: the remaining stock, all excess.
    let mut data10b = disc("exit");
    data10b.extend_from_slice(&7u64.to_le_bytes());
    data10b.extend_from_slice(&1u64.to_le_bytes());
    data10b.extend_from_slice(&8_500_000u64.to_le_bytes());
    let ix10b = Instruction::new_with_bytes(
        program_id,
        &data10b,
        vec![
            AccountMeta::new(holder, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new(window_state_1, false),
            AccountMeta::new(holder_record_1, false),
        ],
    );
    let (next, cu) = run(&mollusk, "exit:bulk_max", &ix10b, &accounts);
    accounts = next;
    let gate10 = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "exit:bulk_max",
        "cu": cu,
        "result": "Success",
        "note": "slice-10 GATED measurement (W3.2 re-expression, T11): largest executable bulk exit (8,500,000 raw = entire remaining window-1 stock, below L = 10,000,000) on Committed window flow transition 7, funded window 1 — accrued 0, so the full amount is excess at the flat 13% door: penalty 1,105,000, paid 7,395,000, position 23,500,000 -> 15,000,000",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate10,
    }));
    let hr10 = accounts.iter().find(|(k, _)| *k == holder_record_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr10.data[64..72].try_into().unwrap()), 0u64, "accrued dividend not 0 after exit:bulk_max");
    assert_eq!(u64::from_le_bytes(hr10.data[56..64].try_into().unwrap()), 15_000_000u64, "position not 15,000,000 after exit:bulk_max");
    assert_eq!(u64::from_le_bytes(hr10.data[72..80].try_into().unwrap()), 0u64, "loan moved during exit:bulk_max");
    let ws1_10 = accounts.iter().find(|(k, _)| *k == window_state_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws1_10.data[64..72].try_into().unwrap()), 0u64, "window 1 exit stock not 0 after exit:bulk_max");
    assert_eq!(u64::from_le_bytes(ws1_10.data[16..24].try_into().unwrap()), 60_000_000u64, "window 1 snapshot moved during exit:bulk_max");
    let ws13_10 = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws13_10.data[64..72].try_into().unwrap()), 6_000_000u64, "window 13 exit stock moved during exit:bulk_max (the bulk exit debits its own window only)");
    let es10 = accounts.iter().find(|(k, _)| *k == exit_schedule_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(es10.data[72..80].try_into().unwrap()), 2u64, "schedule tranches_executed moved during exit:bulk_max");
    assert_eq!(i64::from_le_bytes(es10.data[96..104].try_into().unwrap()), 31_608_400i64, "schedule last_block_time moved during exit:bulk_max");
    // (c) window-1 stock exhausted: no further exit in this window.
    let mut data10c = disc("exit");
    data10c.extend_from_slice(&7u64.to_le_bytes());
    data10c.extend_from_slice(&1u64.to_le_bytes());
    data10c.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix10c = Instruction::new_with_bytes(
        program_id,
        &data10c,
        vec![
            AccountMeta::new(holder, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new(window_state_1, false),
            AccountMeta::new(holder_record_1, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "exit:stock_exhausted", &ix10c, &accounts, 6063);
    measured.push(serde_json::json!({
        "instruction": "exit:stock_exhausted",
        "cu": cu,
        "result": "Refused",
        "error": "ExitStockExhausted",
        "error_code": 6063,
        "note": "slice-10 W3.2 re-expression: a further 1,000,000 raw exit on window 1 after the bulk exit — window-1 aggregate exits now total exactly L (slice-7 1,500,000 + slice-10 8,500,000 = 10,000,000), so the stock bound refuses, NOT a success CU",
    }));
    let hr10c = accounts.iter().find(|(k, _)| *k == holder_record_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr10c.data[56..64].try_into().unwrap()), 15_000_000u64, "position moved during exit:stock_exhausted refusal");
    let ws1_10c = accounts.iter().find(|(k, _)| *k == window_state_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws1_10c.data[64..72].try_into().unwrap()), 0u64, "window 1 exit stock moved during exit:stock_exhausted refusal");

    // ---- Slice 11: W3.2 re-expression, part 3 — default-as-exit (T10) on the funded chain ----
    // Engine-sim T10: the attacker borrows the max against their
    // position at every window and defaults (interest forgiven,
    // principal settles against the position, next dividend
    // forfeited), and ends strictly worse than the scheduled path
    // because the forfeited dividends are the gap. On the program
    // that claim has two halves, and this slice separates them
    // honestly instead of asserting the engine's conclusion onto
    // the program. The mechanics half is measured here, on twin
    // holders in the currently open window 13 (window_count 14,
    // so 13 is the current window under can_enroll; cohort
    // 13 % 12 = 1; snapshot 60,000,000, exit stock 6,000,000
    // after tranche 2, Committed window-flow transition 7 still
    // authorizing executions; clock 31,608,400 at slice start):
    // twin D (the defaulter) and twin S (the scheduled path),
    // each enrolled at position 10,000,000. Twin D: a borrow
    // above the per-holder cap MUST refuse LoanExceedsPosition
    // (5,000,001 > 50% x 10,000,000), then the max loan
    // 5,000,000 SUCCEEDS at the inclusive boundary of BOTH caps
    // (50% of position AND book cap max_outstanding(60,000,000)
    // = 5,000,000, outstanding 0 + 5,000,000); a settle_default
    // before maturity MUST refuse LoanNotMatured; at maturity
    // (31,608,400 + 60d = 36,792,400) settle_default SUCCEEDS,
    // position 10,000,000 -> 5,000,000, loan cleared, skip flag
    // set, book back to 0. Twin S: pay_tranche 1,000,000, then
    // exit 1,000,000 at par (penalty 0, position untouched).
    // The forfeiture half is the PROBE, and it is measured, not
    // assumed: after twin D's default sets skip_next_dividend,
    // a pay_tranche on that same record is attempted. Slice 16
    // landed the skip gate (can_pay_tranche takes the flag and
    // refuses DividendSkipped 6091), so the probe tranche
    // MUST refuse and the forfeiture bites on this record:
    // twin D then exits 1,000,000 from position instead, at
    // the flat 13% penalty (paid 870,000, position 5,000,000
    // -> 4,000,000), and the one-cycle ledgers come out
    // STRICTLY WORSE for the defaulter (loan cash 5,000,000 +
    // position 4,000,000 + paid 870,000 = 9,870,000; scheduled:
    // paid 1,000,000 + position 10,000,000 = 11,000,000) —
    // the 1,130,000 gap is the forfeited 1,000,000 tranche
    // plus the 130,000 penalty on the replacement exit. T10
    // is claimed re-expressed on the program on this record's
    // terms; the carried remainder, stated plainly, is the
    // cross-record carry: the flag is set on the loan
    // window's record while a holder's next eligible slot
    // lives on a fresh record in a later window, so honoring
    // the skip at enroll time in that window is still owed
    // before the D-3 forfeiture is re-expressed end to end.
    // As in every slice, borrow / exit / settle_default here
    // are bookkeeping, not token movement; the disbursement
    // step is separate.
    let holder_d = Pubkey::new_from_array([21u8; 32]);
    let holder_s = Pubkey::new_from_array([22u8; 32]);
    let (holder_record_d, _) = pda(&[b"holder", &13u64.to_le_bytes(), holder_d.as_ref()], &program_id);
    let (holder_record_s, _) = pda(&[b"holder", &13u64.to_le_bytes(), holder_s.as_ref()], &program_id);
    upsert(&mut accounts, (holder_d, funded()));
    upsert(&mut accounts, (holder_s, funded()));
    upsert(&mut accounts, (holder_record_d, empty.clone()));
    upsert(&mut accounts, (holder_record_s, empty.clone()));
    // (a) enroll twin D into window 13 (cohort 1, position 10,000,000).
    let mut data11a = disc("enroll");
    data11a.extend_from_slice(&13u64.to_le_bytes());
    data11a.extend_from_slice(&10_000_000u64.to_le_bytes());
    let (forfeiture_holder_d, _) = pda(&[b"forfeiture", holder_d.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_d) { accounts.push((forfeiture_holder_d, empty.clone())); }
    let ix11a = Instruction::new_with_bytes(
        program_id,
        &data11a,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_13, false),
            AccountMeta::new(holder_record_d, false),
            AccountMeta::new(forfeiture_holder_d, false),
            AccountMeta::new(holder_d, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll:13D", &ix11a, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "enroll:13D", "cu": cu, "result": "Success", "note": "slice-11 setup: defaulter twin enrolls in window 13 (cohort 1, position 10,000,000)"}));
    let hr11d = accounts.iter().find(|(k, _)| *k == holder_record_d).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr11d.data[48..56].try_into().unwrap()), 1u64, "twin D cohort not 1 (13 % 12)");
    assert_eq!(u64::from_le_bytes(hr11d.data[56..64].try_into().unwrap()), 10_000_000u64, "twin D position not 10,000,000 after enroll:13D");
    assert_eq!(hr11d.data[89], 1u8, "twin D standing authorization not true at enroll:13D");
    // (b) enroll twin S into window 13 (cohort 1, position 10,000,000).
    let mut data11b = disc("enroll");
    data11b.extend_from_slice(&13u64.to_le_bytes());
    data11b.extend_from_slice(&10_000_000u64.to_le_bytes());
    let (forfeiture_holder_s, _) = pda(&[b"forfeiture", holder_s.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_s) { accounts.push((forfeiture_holder_s, empty.clone())); }
    let ix11b = Instruction::new_with_bytes(
        program_id,
        &data11b,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_13, false),
            AccountMeta::new(holder_record_s, false),
            AccountMeta::new(forfeiture_holder_s, false),
            AccountMeta::new(holder_s, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll:13S", &ix11b, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "enroll:13S", "cu": cu, "result": "Success", "note": "slice-11 setup: scheduled-path twin enrolls in window 13 (cohort 1, position 10,000,000)"}));
    let hr11s = accounts.iter().find(|(k, _)| *k == holder_record_s).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr11s.data[48..56].try_into().unwrap()), 1u64, "twin S cohort not 1 (13 % 12)");
    assert_eq!(u64::from_le_bytes(hr11s.data[56..64].try_into().unwrap()), 10_000_000u64, "twin S position not 10,000,000 after enroll:13S");
    let ws13_11 = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws13_11.data[16..24].try_into().unwrap()), 60_000_000u64, "window 13 snapshot moved during slice-11 enrolls");
    assert_eq!(u64::from_le_bytes(ws13_11.data[64..72].try_into().unwrap()), 6_000_000u64, "window 13 exit stock moved during slice-11 enrolls");
    let ts11 = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts11.data[56..64].try_into().unwrap()), 14u64, "window_count moved during slice-11 enrolls");
    // (c) borrow above the per-holder cap MUST refuse LoanExceedsPosition (6052).
    let mut data11c = disc("borrow");
    data11c.extend_from_slice(&7u64.to_le_bytes());
    data11c.extend_from_slice(&13u64.to_le_bytes());
    data11c.extend_from_slice(&5_000_001u64.to_le_bytes());
    let ix11c = Instruction::new_with_bytes(
        program_id,
        &data11c,
        vec![
            AccountMeta::new(holder_d, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_13, false),
            AccountMeta::new(holder_record_d, false),
            AccountMeta::new(lending_book, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "borrow:over_cap", &ix11c, &accounts, 6052);
    measured.push(serde_json::json!({
        "instruction": "borrow:over_cap",
        "cu": cu,
        "result": "Refused",
        "error": "LoanExceedsPosition",
        "error_code": 6052,
        "note": "slice-11 W3.2 re-expression (T10): twin D borrow 5,000,001 raw on Committed window flow transition 7, window 13 — above the 50%-of-position cap (5,000,000), NOT a success CU",
    }));
    let hr11cr = accounts.iter().find(|(k, _)| *k == holder_record_d).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr11cr.data[72..80].try_into().unwrap()), 0u64, "loan moved during borrow:over_cap refusal");
    // (d) borrow at the max — the inclusive boundary of BOTH caps — SUCCEEDS, gated §2.2 ~7K.
    let mut data11d = disc("borrow");
    data11d.extend_from_slice(&7u64.to_le_bytes());
    data11d.extend_from_slice(&13u64.to_le_bytes());
    data11d.extend_from_slice(&5_000_000u64.to_le_bytes());
    let ix11d = Instruction::new_with_bytes(
        program_id,
        &data11d,
        vec![
            AccountMeta::new(holder_d, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_13, false),
            AccountMeta::new(holder_record_d, false),
            AccountMeta::new(lending_book, false),
        ],
    );
    let (next, cu) = run(&mollusk, "borrow:11max", &ix11d, &accounts);
    accounts = next;
    let gate11 = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "borrow:11max",
        "cu": cu,
        "result": "Success",
        "note": "slice-11 GATED measurement (W3.2 re-expression, T10): twin D borrows the max 5,000,000 raw on Committed window flow transition 7, window 13 — at the inclusive boundary of the 50%-of-position cap (5,000,000) and the book cap (max_outstanding(60,000,000) = 5,000,000, outstanding 0 + 5,000,000)",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate11,
    }));
    let hr11d = accounts.iter().find(|(k, _)| *k == holder_record_d).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr11d.data[72..80].try_into().unwrap()), 5_000_000u64, "twin D loan not 5,000,000 after borrow:11max");
    assert_eq!(i64::from_le_bytes(hr11d.data[80..88].try_into().unwrap()), 36_792_400i64, "twin D maturity not 36,792,400 (31,608,400 + 60d) after borrow:11max");
    let lb11 = accounts.iter().find(|(k, _)| *k == lending_book).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(lb11.data[8..16].try_into().unwrap()), 5_000_000u64, "lending book outstanding not 5,000,000 after borrow:11max");
    assert_eq!(u32::from_le_bytes(lb11.data[16..20].try_into().unwrap()), 1u32, "lending book active loans not 1 after borrow:11max");
    // (e) settle_default before maturity MUST refuse LoanNotMatured (6056).
    let mut data11e = disc("settle_default");
    data11e.extend_from_slice(&7u64.to_le_bytes());
    data11e.extend_from_slice(&13u64.to_le_bytes());
    data11e.extend_from_slice(holder_d.as_ref());
    let (forfeiture_holder_d, _) = pda(&[b"forfeiture", holder_d.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_d) { accounts.push((forfeiture_holder_d, empty.clone())); }
    let ix11e = Instruction::new_with_bytes(
        program_id,
        &data11e,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_13, false),
            AccountMeta::new(holder_record_d, false),
            AccountMeta::new(lending_book, false),
            AccountMeta::new(forfeiture_holder_d, false),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "settle_default:pre_maturity", &ix11e, &accounts, 6056);
    measured.push(serde_json::json!({
        "instruction": "settle_default:pre_maturity",
        "cu": cu,
        "result": "Refused",
        "error": "LoanNotMatured",
        "error_code": 6056,
        "note": "slice-11 W3.2 re-expression (T10): operator settle_default on twin D at clock 31,608,400, before maturity 36,792,400 — no operator seizes a position before it is due, NOT a success CU",
    }));
    let hr11er = accounts.iter().find(|(k, _)| *k == holder_record_d).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr11er.data[56..64].try_into().unwrap()), 10_000_000u64, "position moved during settle_default:pre_maturity refusal");
    assert_eq!(u64::from_le_bytes(hr11er.data[72..80].try_into().unwrap()), 5_000_000u64, "loan moved during settle_default:pre_maturity refusal");
    // (f) settle_default at maturity SUCCEEDS, gated §2.2 ~7K: position 10,000,000 -> 5,000,000,
    // loan cleared, skip flag set, book back to 0. D-3: the event carries position_before 10,000,000.
    mollusk.sysvars.clock.unix_timestamp = 36_792_400;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let mut data11f = disc("settle_default");
    data11f.extend_from_slice(&7u64.to_le_bytes());
    data11f.extend_from_slice(&13u64.to_le_bytes());
    data11f.extend_from_slice(holder_d.as_ref());
    let (forfeiture_holder_d, _) = pda(&[b"forfeiture", holder_d.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_d) { accounts.push((forfeiture_holder_d, empty.clone())); }
    let ix11f = Instruction::new_with_bytes(
        program_id,
        &data11f,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_13, false),
            AccountMeta::new(holder_record_d, false),
            AccountMeta::new(lending_book, false),
            AccountMeta::new(forfeiture_holder_d, false),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "settle_default:11", &ix11f, &accounts);
    accounts = next;
    let gate11 = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "settle_default:11",
        "cu": cu,
        "result": "Success",
        "note": "slice-11 GATED measurement (W3.2 re-expression, T10): operator-settled default at maturity 36,792,400 on Committed window flow transition 7, window 13 — principal 5,000,000 settles against position, interest forgiven, skip-next-dividend set on the pre-default base 10,000,000 (D-3)",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate11,
    }));
    let hr11d = accounts.iter().find(|(k, _)| *k == holder_record_d).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr11d.data[56..64].try_into().unwrap()), 5_000_000u64, "twin D position not 5,000,000 after settle_default:11");
    assert_eq!(u64::from_le_bytes(hr11d.data[72..80].try_into().unwrap()), 0u64, "twin D loan not cleared by settle_default:11");
    assert_eq!(i64::from_le_bytes(hr11d.data[80..88].try_into().unwrap()), 0i64, "twin D maturity not cleared by settle_default:11");
    assert_eq!(hr11d.data[88], 1u8, "twin D skip-next-dividend flag not set by settle_default:11");
    let lb11 = accounts.iter().find(|(k, _)| *k == lending_book).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(lb11.data[8..16].try_into().unwrap()), 0u64, "lending book outstanding not 0 after settle_default:11");
    assert_eq!(u32::from_le_bytes(lb11.data[16..20].try_into().unwrap()), 0u32, "lending book active loans not 0 after settle_default:11");
    // (g) THE PROBE, slice 16: pay_tranche on twin D AFTER the default set skip_next_dividend
    // MUST refuse DividendSkipped (6091) — the skip gate landed, so the forfeited slot pays
    // nothing, the flag stays set, and the slot stays unpaid, so a retry cannot pay it either.
    // A refused instruction leaves the accounts unchanged, so no account threading here.
    let mut data11g = disc("pay_tranche");
    data11g.extend_from_slice(&7u64.to_le_bytes());
    data11g.extend_from_slice(&13u64.to_le_bytes());
    data11g.extend_from_slice(holder_d.as_ref());
    data11g.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix11g = Instruction::new_with_bytes(
        program_id,
        &data11g,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_13, false),
            AccountMeta::new(holder_record_d, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "pay_tranche:11skip_probe", &ix11g, &accounts, 6091);
    measured.push(serde_json::json!({
        "instruction": "pay_tranche:11skip_probe",
        "cu": cu,
        "result": "Refused",
        "error": "DividendSkipped",
        "error_code": 6091,
        "note": "slice-16 PROBE (W3.2 re-expression, T10): operator pay_tranche 1,000,000 on twin D AFTER settle_default set skip_next_dividend — REFUSES DividendSkipped (6091): the slice-16 skip gate closes the slice-11 probe on the record that carries the flag (accrued stays 0, flag stays set, slot stays unpaid); the cross-record carry to a later window's fresh record stays a carried item, NOT a success CU",
    }));
    let hr11d = accounts.iter().find(|(k, _)| *k == holder_record_d).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr11d.data[64..72].try_into().unwrap()), 0u64, "twin D accrued moved during pay_tranche:11skip_probe refusal");
    assert_eq!(hr11d.data[88], 1u8, "twin D skip flag cleared during pay_tranche:11skip_probe refusal");
    assert_eq!(hr11d.data[98], 0u8, "twin D tranche_paid flag set during pay_tranche:11skip_probe refusal");
    assert_eq!(u64::from_le_bytes(hr11d.data[56..64].try_into().unwrap()), 5_000_000u64, "twin D position moved during pay_tranche:11skip_probe refusal");
    // (h) twin D, tranche forfeited, exits 1,000,000 from position instead: accrued 0, so the
    // whole amount is excess at the flat 13% penalty (paid 870,000, penalty 130,000), position
    // 5,000,000 -> 4,000,000, stock 6,000,000 -> 5,000,000.
    let mut data11h = disc("exit");
    data11h.extend_from_slice(&7u64.to_le_bytes());
    data11h.extend_from_slice(&13u64.to_le_bytes());
    data11h.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix11h = Instruction::new_with_bytes(
        program_id,
        &data11h,
        vec![
            AccountMeta::new(holder_d, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new(window_state_13, false),
            AccountMeta::new(holder_record_d, false),
        ],
    );
    let (next, cu) = run(&mollusk, "exit:11D", &ix11h, &accounts);
    accounts = next;
    let gate11 = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "exit:11D",
        "cu": cu,
        "result": "Success",
        "note": "slice-16 GATED measurement (W3.2 re-expression, T10): twin D, tranche forfeited by the slice-16 skip gate, exits 1,000,000 from position on Committed window flow transition 7, window 13 — accrued 0, so the whole amount is excess at the flat 13% penalty (paid 870,000, penalty 130,000), position 5,000,000 -> 4,000,000",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate11,
    }));
    let hr11d = accounts.iter().find(|(k, _)| *k == holder_record_d).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr11d.data[64..72].try_into().unwrap()), 0u64, "twin D accrued moved during exit:11D");
    assert_eq!(u64::from_le_bytes(hr11d.data[56..64].try_into().unwrap()), 4_000_000u64, "twin D position not 4,000,000 after exit:11D (5,000,000 - 1,000,000 excess exit)");
    // (i) twin S, the scheduled path: pay_tranche 1,000,000, then exit 1,000,000 at par.
    let mut data11i = disc("pay_tranche");
    data11i.extend_from_slice(&7u64.to_le_bytes());
    data11i.extend_from_slice(&13u64.to_le_bytes());
    data11i.extend_from_slice(holder_s.as_ref());
    data11i.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix11i = Instruction::new_with_bytes(
        program_id,
        &data11i,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_13, false),
            AccountMeta::new(holder_record_s, false),
        ],
    );
    let (next, cu) = run(&mollusk, "pay_tranche:11S", &ix11i, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "pay_tranche:11S", "cu": cu, "result": "Success", "note": "slice-11 setup: scheduled-path twin S pay_tranche 1,000,000 on Committed window flow transition 7, window 13"}));
    let mut data11j = disc("exit");
    data11j.extend_from_slice(&7u64.to_le_bytes());
    data11j.extend_from_slice(&13u64.to_le_bytes());
    data11j.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix11j = Instruction::new_with_bytes(
        program_id,
        &data11j,
        vec![
            AccountMeta::new(holder_s, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new(window_state_13, false),
            AccountMeta::new(holder_record_s, false),
        ],
    );
    let (next, cu) = run(&mollusk, "exit:11S_par", &ix11j, &accounts);
    accounts = next;
    let gate11 = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "exit:11S_par",
        "cu": cu,
        "result": "Success",
        "note": "slice-11 GATED measurement (W3.2 re-expression, T10): twin S exits the scheduled tranche 1,000,000 at par on Committed window flow transition 7, window 13 — accrued consumed, penalty 0, position stays 10,000,000",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate11,
    }));
    let hr11s = accounts.iter().find(|(k, _)| *k == holder_record_s).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr11s.data[64..72].try_into().unwrap()), 0u64, "twin S accrued not consumed by exit:11S_par");
    assert_eq!(u64::from_le_bytes(hr11s.data[56..64].try_into().unwrap()), 10_000_000u64, "twin S position moved during exit:11S_par");
    assert_eq!(hr11s.data[88], 0u8, "twin S skip flag set without a default");
    // Final slice-11 state, slice 16: the one-cycle ledgers are STRICTLY WORSE for the
    // defaulter on the program now — defaulter 5,000,000 loan cash + 4,000,000 position
    // + 870,000 net paid = 9,870,000; scheduled 1,000,000 paid + 10,000,000 position =
    // 11,000,000. The 1,130,000 gap is the forfeited 1,000,000 tranche plus the 130,000
    // penalty on the replacement position exit. T10 is claimed re-expressed on this
    // record's terms; the cross-record carry stays a carried item (see the slice header).
    let ws13_11f = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws13_11f.data[16..24].try_into().unwrap()), 60_000_000u64, "window 13 snapshot moved during slice 11");
    assert_eq!(u64::from_le_bytes(ws13_11f.data[64..72].try_into().unwrap()), 4_000_000u64, "window 13 exit stock not 4,000,000 after slice 11 (6,000,000 - twin D 1,000,000 - twin S 1,000,000)");
    let ws1_11f = accounts.iter().find(|(k, _)| *k == window_state_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws1_11f.data[64..72].try_into().unwrap()), 0u64, "window 1 exit stock moved during slice 11");
    let es11f = accounts.iter().find(|(k, _)| *k == exit_schedule_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(es11f.data[72..80].try_into().unwrap()), 2u64, "schedule tranches_executed moved during slice 11");
    assert_eq!(i64::from_le_bytes(es11f.data[96..104].try_into().unwrap()), 31_608_400i64, "schedule last_block_time moved during slice 11");

    // ---- Slice 12: W3.2 re-expression, part 4 — cohort-timing at window boundaries (D-11) ----
    // Engine-sim cohort-timing games ask whether a holder can slip
    // into a window that has closed, reserve a window that has not
    // opened, enter twice, or enter with nothing locked. The
    // program answers all four at the enroll gate (can_enroll:
    // window_id must equal window_count - 1, position positive)
    // plus the PDA init (one record per holder per window). This
    // slice measures the gate on the slice-11 chain (window_count
    // 14, current window 13, clock 36,792,400, window 13 snapshot
    // 60,000,000 / stock 4,000,000, Committed window-flow
    // transition 7 still authorizing executions), then CROSSES
    // the boundary for real — a new Settled window transition
    // opens window 14 — and measures both sides of the crossing:
    // the just-closed window 13 now refuses, the new current
    // window 14 accepts with the next rotation slot (14 % 12 = 2),
    // and the new cohort's payout rides that slot. A future
    // window's enroll cannot even reach can_enroll: its window
    // account does not exist yet, so Anchor's account layer
    // refuses first — measured with the any-error helper and the
    // rendered error recorded verbatim, not forced into a custom
    // code it never reaches. Same for the double enroll: the
    // second init refuses at the account layer, which IS the
    // counted-once mechanism, not a gate bypass. As in every
    // slice, enroll / pay_tranche here are bookkeeping, not token
    // movement; the disbursement step is separate.
    let holder_23 = Pubkey::new_from_array([23u8; 32]);
    let holder_24 = Pubkey::new_from_array([24u8; 32]);
    let holder_25 = Pubkey::new_from_array([25u8; 32]);
    let holder_26 = Pubkey::new_from_array([26u8; 32]);
    let holder_27 = Pubkey::new_from_array([27u8; 32]);
    let holder_28 = Pubkey::new_from_array([28u8; 32]);
    let (window_state_12, _) = pda(&[b"window", &12u64.to_le_bytes()], &program_id);
    let (window_state_14, _) = pda(&[b"window", &14u64.to_le_bytes()], &program_id);
    let (window_state_15, _) = pda(&[b"window", &15u64.to_le_bytes()], &program_id);
    let ws12_pre = accounts.iter().find(|(k, _)| *k == window_state_12).map(|(_, a)| a.clone()).unwrap();
    let ws12_roster_pre: Vec<u8> = ws12_pre.data[88..120].to_vec();
    // (a) enroll into PAST window 12 MUST refuse NotCurrentWindow (6043).
    let (holder_record_23_12, _) = pda(&[b"holder", &12u64.to_le_bytes(), holder_23.as_ref()], &program_id);
    upsert(&mut accounts, (holder_23, funded()));
    upsert(&mut accounts, (holder_record_23_12, empty.clone()));
    let mut data12a = disc("enroll");
    data12a.extend_from_slice(&12u64.to_le_bytes());
    data12a.extend_from_slice(&5_000_000u64.to_le_bytes());
    let (forfeiture_holder_23, _) = pda(&[b"forfeiture", holder_23.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_23) { accounts.push((forfeiture_holder_23, empty.clone())); }
    let ix12a = Instruction::new_with_bytes(
        program_id,
        &data12a,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_12, false),
            AccountMeta::new(holder_record_23_12, false),
            AccountMeta::new(forfeiture_holder_23, false),
            AccountMeta::new(holder_23, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "enroll:12past", &ix12a, &accounts, 6043);
    measured.push(serde_json::json!({
        "instruction": "enroll:12past",
        "cu": cu,
        "result": "Refused",
        "error": "NotCurrentWindow",
        "error_code": 6043,
        "note": "slice-12 W3.2 re-expression (D-11): enroll into past window 12 while window_count is 14 (current window 13) — a closed pool admits no newcomer, NOT a success CU",
    }));
    let ws12r = accounts.iter().find(|(k, _)| *k == window_state_12).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(ws12r.data[88..120].to_vec(), ws12_roster_pre, "window 12 roster hash moved during enroll:12past refusal");
    let ts12r = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts12r.data[56..64].try_into().unwrap()), 14u64, "window_count moved during enroll:12past refusal");
    // (b) enroll into the CURRENT window 13 with a ZERO position MUST refuse InvalidPosition (6044).
    let (holder_record_24_13, _) = pda(&[b"holder", &13u64.to_le_bytes(), holder_24.as_ref()], &program_id);
    upsert(&mut accounts, (holder_24, funded()));
    upsert(&mut accounts, (holder_record_24_13, empty.clone()));
    let mut data12b = disc("enroll");
    data12b.extend_from_slice(&13u64.to_le_bytes());
    data12b.extend_from_slice(&0u64.to_le_bytes());
    let (forfeiture_holder_24, _) = pda(&[b"forfeiture", holder_24.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_24) { accounts.push((forfeiture_holder_24, empty.clone())); }
    let ix12b = Instruction::new_with_bytes(
        program_id,
        &data12b,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_13, false),
            AccountMeta::new(holder_record_24_13, false),
            AccountMeta::new(forfeiture_holder_24, false),
            AccountMeta::new(holder_24, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "enroll:13zero", &ix12b, &accounts, 6044);
    measured.push(serde_json::json!({
        "instruction": "enroll:13zero",
        "cu": cu,
        "result": "Refused",
        "error": "InvalidPosition",
        "error_code": 6044,
        "note": "slice-12 W3.2 re-expression (D-11): enroll into current window 13 with position 0 — a zero lock is not an entry, NOT a success CU",
    }));
    let ws13_12b = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws13_12b.data[64..72].try_into().unwrap()), 4_000_000u64, "window 13 exit stock moved during enroll:13zero refusal");
    // (c) DOUBLE enroll: slice-11 twin D enrolls again in window 13 MUST refuse at the account layer
    // (the record already exists; the one-record-per-holder-per-window init is the counted-once mechanism).
    let mut data12c = disc("enroll");
    data12c.extend_from_slice(&13u64.to_le_bytes());
    data12c.extend_from_slice(&1_000_000u64.to_le_bytes());
    let (forfeiture_holder_d, _) = pda(&[b"forfeiture", holder_d.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_d) { accounts.push((forfeiture_holder_d, empty.clone())); }
    let ix12c = Instruction::new_with_bytes(
        program_id,
        &data12c,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_13, false),
            AccountMeta::new(holder_record_d, false),
            AccountMeta::new(forfeiture_holder_d, false),
            AccountMeta::new(holder_d, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (cu, err12c) = run_expect_any_err(&mollusk, "enroll:13double", &ix12c, &accounts);
    measured.push(serde_json::json!({
        "instruction": "enroll:13double",
        "cu": cu,
        "result": "Refused",
        "error": err12c,
        "note": "slice-12 W3.2 re-expression (D-11): slice-11 twin D enrolls a second time in window 13 — refused at the account layer before the handler: the Anchor init's system-program create_account CPI fails with custom 0x0 on the already-existing holder record, so one holder's funds are never counted twice in one pool; the rendered error is recorded verbatim, NOT a success CU",
    }));
    let hr12c = accounts.iter().find(|(k, _)| *k == holder_record_d).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr12c.data[56..64].try_into().unwrap()), 4_000_000u64, "twin D position moved during enroll:13double refusal");
    assert_eq!(u64::from_le_bytes(hr12c.data[64..72].try_into().unwrap()), 0u64, "twin D accrued moved during enroll:13double refusal");
    assert_eq!(hr12c.data[88], 1u8, "twin D skip flag moved during enroll:13double refusal");
    // (d) enroll into the CURRENT window 13 SUCCEEDS (setup, as in slice 11): cohort 1, standing authorization true.
    let (holder_record_25_13, _) = pda(&[b"holder", &13u64.to_le_bytes(), holder_25.as_ref()], &program_id);
    upsert(&mut accounts, (holder_25, funded()));
    upsert(&mut accounts, (holder_record_25_13, empty.clone()));
    let mut data12d = disc("enroll");
    data12d.extend_from_slice(&13u64.to_le_bytes());
    data12d.extend_from_slice(&5_000_000u64.to_le_bytes());
    let (forfeiture_holder_25, _) = pda(&[b"forfeiture", holder_25.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_25) { accounts.push((forfeiture_holder_25, empty.clone())); }
    let ix12d = Instruction::new_with_bytes(
        program_id,
        &data12d,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_13, false),
            AccountMeta::new(holder_record_25_13, false),
            AccountMeta::new(forfeiture_holder_25, false),
            AccountMeta::new(holder_25, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll:13current", &ix12d, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "enroll:13current", "cu": cu, "result": "Success", "note": "slice-12 setup: holder 25 enrolls in the still-current window 13 (cohort 1, position 5,000,000) — the live side of the boundary before it is crossed"}));
    let hr12d = accounts.iter().find(|(k, _)| *k == holder_record_25_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr12d.data[48..56].try_into().unwrap()), 1u64, "holder 25 cohort not 1 (13 % 12)");
    assert_eq!(u64::from_le_bytes(hr12d.data[56..64].try_into().unwrap()), 5_000_000u64, "holder 25 position not 5,000,000 after enroll:13current");
    assert_eq!(hr12d.data[89], 1u8, "holder 25 standing authorization not true at enroll:13current");
    let ws13_12d = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws13_12d.data[64..72].try_into().unwrap()), 4_000_000u64, "window 13 exit stock moved during enroll:13current (enroll debits no stock)");
    let ws13_roster_after_d: Vec<u8> = ws13_12d.data[88..120].to_vec();
    // (e) CROSS THE BOUNDARY: Settled treasury.window.v1 transition 21 (parent 20, [197;32],
    // declared [207;32], now 37,000,000 — after the slice-11 clock 36,792,400) opens window 14.
    // One lifecycle, in the slice-8 shape: its 10 setup entries carry the helper's notes.
    let (observation_21, _) = pda(&[b"observation", &21u64.to_le_bytes()], &program_id);
    let (transition_21, _) = pda(&[b"transition", &21u64.to_le_bytes()], &program_id);
    let (plan_21, _) = pda(&[b"plan", &21u64.to_le_bytes(), proposer.as_ref()], &program_id);
    let (signal_21, signal_21_bump) = pda(
        &[b"signal", &21u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
        &program_id,
    );
    let (evidence_21, _) = pda(&[b"evidence", &21u64.to_le_bytes()], &program_id);
    upsert(&mut accounts, (observation_21, empty.clone()));
    upsert(&mut accounts, (transition_21, empty.clone()));
    upsert(&mut accounts, (plan_21, empty.clone()));
    upsert(&mut accounts, (signal_21, empty.clone()));
    upsert(&mut accounts, (evidence_21, empty.clone()));
    upsert(&mut accounts, (window_state_14, empty.clone()));
    accounts = lifecycle_to_commit(
        &mut mollusk, accounts, &mut measured,
        program_id, clock_id, system_id,
        oracle_record, oracle, proposer_record, proposer, evaluator_record, evaluator,
        chain, stock, policy_sha,
        observation_21, transition_21, plan_21, signal_21, signal_21_bump,
        21, 38_000_000, 39_000_000, [207u8; 32], 20, [197u8; 32],
        [9u8; 32], 37_000_000, ":21", Some(evidence_21),
    );
    let mut data12e = disc("open_window");
    data12e.extend_from_slice(&21u64.to_le_bytes());
    data12e.extend_from_slice(&14u64.to_le_bytes());
    let ix12e = Instruction::new_with_bytes(
        program_id,
        &data12e,
        vec![
            AccountMeta::new(treasury_state, false),
            AccountMeta::new_readonly(lending_book, false),
            AccountMeta::new(transition_21, false),
            AccountMeta::new(window_state_14, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "open_window:14", &ix12e, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "open_window:14", "cu": cu, "result": "Success", "note": "slice-12 setup: window 14 opens on Settled transition 21 (funded snapshot total 60,000,000, exit stock 10,000,000) — the boundary crossing; window_count 14 -> 15, current window 13 -> 14"}));
    let ts12e = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts12e.data[56..64].try_into().unwrap()), 15u64, "window_count not 15 after open_window:14");
    let ws14_12e = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws14_12e.data[16..24].try_into().unwrap()), 60_000_000u64, "window 14 snapshot total not 60,000,000");
    assert_eq!(u64::from_le_bytes(ws14_12e.data[64..72].try_into().unwrap()), 10_000_000u64, "window 14 exit stock not 10,000,000");
    assert_eq!(u64::from_le_bytes(ws14_12e.data[72..80].try_into().unwrap()), 21u64, "window 14 source transition not 21");
    // (f) enroll into the JUST-CLOSED window 13 MUST refuse NotCurrentWindow (6043):
    // the holder who waited one window too long has lost the old pool.
    let (holder_record_26_13, _) = pda(&[b"holder", &13u64.to_le_bytes(), holder_26.as_ref()], &program_id);
    upsert(&mut accounts, (holder_26, funded()));
    upsert(&mut accounts, (holder_record_26_13, empty.clone()));
    let mut data12f = disc("enroll");
    data12f.extend_from_slice(&13u64.to_le_bytes());
    data12f.extend_from_slice(&5_000_000u64.to_le_bytes());
    let (forfeiture_holder_26, _) = pda(&[b"forfeiture", holder_26.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_26) { accounts.push((forfeiture_holder_26, empty.clone())); }
    let ix12f = Instruction::new_with_bytes(
        program_id,
        &data12f,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_13, false),
            AccountMeta::new(holder_record_26_13, false),
            AccountMeta::new(forfeiture_holder_26, false),
            AccountMeta::new(holder_26, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "enroll:13closed", &ix12f, &accounts, 6043);
    measured.push(serde_json::json!({
        "instruction": "enroll:13closed",
        "cu": cu,
        "result": "Refused",
        "error": "NotCurrentWindow",
        "error_code": 6043,
        "note": "slice-12 W3.2 re-expression (D-11): enroll into window 13 AFTER window 14 opened (window_count 15) — the same window that accepted enroll:13current minutes earlier now refuses; the boundary is the counter, not the clock, NOT a success CU",
    }));
    let ws13_12f = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(ws13_12f.data[88..120].to_vec(), ws13_roster_after_d, "window 13 roster hash moved during enroll:13closed refusal");
    // (g) enroll into the NEW current window 14 SUCCEEDS (setup): cohort 2 (14 % 12), standing authorization true.
    let (holder_record_27_14, _) = pda(&[b"holder", &14u64.to_le_bytes(), holder_27.as_ref()], &program_id);
    upsert(&mut accounts, (holder_27, funded()));
    upsert(&mut accounts, (holder_record_27_14, empty.clone()));
    let mut data12g = disc("enroll");
    data12g.extend_from_slice(&14u64.to_le_bytes());
    data12g.extend_from_slice(&5_000_000u64.to_le_bytes());
    let (forfeiture_holder_27, _) = pda(&[b"forfeiture", holder_27.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_27) { accounts.push((forfeiture_holder_27, empty.clone())); }
    let ix12g = Instruction::new_with_bytes(
        program_id,
        &data12g,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_14, false),
            AccountMeta::new(holder_record_27_14, false),
            AccountMeta::new(forfeiture_holder_27, false),
            AccountMeta::new(holder_27, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll:14current", &ix12g, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "enroll:14current", "cu": cu, "result": "Success", "note": "slice-12 setup: holder 27 enrolls in the new current window 14 (cohort 2, position 5,000,000) — next-window-only entry in its positive form: the newcomer joins the new pool, never the closed one"}));
    let hr12g = accounts.iter().find(|(k, _)| *k == holder_record_27_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr12g.data[48..56].try_into().unwrap()), 2u64, "holder 27 cohort not 2 (14 % 12)");
    assert_eq!(u64::from_le_bytes(hr12g.data[56..64].try_into().unwrap()), 5_000_000u64, "holder 27 position not 5,000,000 after enroll:14current");
    assert_eq!(hr12g.data[89], 1u8, "holder 27 standing authorization not true at enroll:14current");
    let ws14_12g = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws14_12g.data[16..24].try_into().unwrap()), 60_000_000u64, "window 14 snapshot moved during enroll:14current");
    assert_eq!(u64::from_le_bytes(ws14_12g.data[64..72].try_into().unwrap()), 10_000_000u64, "window 14 exit stock moved during enroll:14current (enroll debits no stock)");
    // (h) enroll into FUTURE window 15 MUST refuse at the account layer: no window 15 account
    // exists yet, so the gate is never reached — the reservation of a not-yet-opened pool is
    // impossible by construction, not merely by policy. Rendered error recorded verbatim.
    let (holder_record_28_15, _) = pda(&[b"holder", &15u64.to_le_bytes(), holder_28.as_ref()], &program_id);
    upsert(&mut accounts, (holder_28, funded()));
    upsert(&mut accounts, (window_state_15, empty.clone()));
    upsert(&mut accounts, (holder_record_28_15, empty.clone()));
    let mut data12h = disc("enroll");
    data12h.extend_from_slice(&15u64.to_le_bytes());
    data12h.extend_from_slice(&5_000_000u64.to_le_bytes());
    let (forfeiture_holder_28, _) = pda(&[b"forfeiture", holder_28.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_28) { accounts.push((forfeiture_holder_28, empty.clone())); }
    let ix12h = Instruction::new_with_bytes(
        program_id,
        &data12h,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_15, false),
            AccountMeta::new(holder_record_28_15, false),
            AccountMeta::new(forfeiture_holder_28, false),
            AccountMeta::new(holder_28, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (cu, err12h) = run_expect_any_err(&mollusk, "enroll:15future", &ix12h, &accounts);
    measured.push(serde_json::json!({
        "instruction": "enroll:15future",
        "cu": cu,
        "result": "Refused",
        "error": err12h,
        "note": "slice-12 W3.2 re-expression (D-11): enroll into future window 15 while window_count is 15 (current window 14) — refused at the Anchor account layer because no window 15 account exists yet, before can_enroll runs; no custom error code, the rendered error is recorded verbatim, NOT a success CU",
    }));
    let ts12h = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts12h.data[56..64].try_into().unwrap()), 15u64, "window_count moved during enroll:15future refusal");
    // (i) the new cohort's payout rides its own slot: pay_tranche 1,000,000 on holder 27 in
    // window 14 (slot 14 % 12 = 2, holder cohort 2) on Committed window-flow transition 7,
    // gated §2.2 ~7K. A window-13 cohort could not collect this slot: its record is
    // PDA-bound to window 13, so no cross-window payout address exists to attempt.
    let mut data12i = disc("pay_tranche");
    data12i.extend_from_slice(&7u64.to_le_bytes());
    data12i.extend_from_slice(&14u64.to_le_bytes());
    data12i.extend_from_slice(holder_27.as_ref());
    data12i.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix12i = Instruction::new_with_bytes(
        program_id,
        &data12i,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_14, false),
            AccountMeta::new(holder_record_27_14, false),
        ],
    );
    let (next, cu) = run(&mollusk, "pay_tranche:14cohort", &ix12i, &accounts);
    accounts = next;
    let gate12 = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "pay_tranche:14cohort",
        "cu": cu,
        "result": "Success",
        "note": "slice-12 GATED measurement (W3.2 re-expression, D-11): operator pay_tranche 1,000,000 on holder 27 in window 14 on Committed window flow transition 7 — holder cohort 2 equals window 14's rotation slot (14 % 12), so the new cohort's slot pays in its own window",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate12,
    }));
    let hr12i = accounts.iter().find(|(k, _)| *k == holder_record_27_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr12i.data[64..72].try_into().unwrap()), 1_000_000u64, "holder 27 accrued not 1,000,000 after pay_tranche:14cohort");
    assert_eq!(hr12i.data[98], 1u8, "holder 27 tranche_paid flag not set by pay_tranche:14cohort");
    assert_eq!(u64::from_le_bytes(hr12i.data[56..64].try_into().unwrap()), 5_000_000u64, "holder 27 position moved during pay_tranche:14cohort");
    // Final slice-12 state: the boundary moved, nothing else did.
    let ws13_12f2 = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws13_12f2.data[16..24].try_into().unwrap()), 60_000_000u64, "window 13 snapshot moved during slice 12");
    assert_eq!(u64::from_le_bytes(ws13_12f2.data[64..72].try_into().unwrap()), 4_000_000u64, "window 13 exit stock not 4,000,000 after slice 12");
    assert_eq!(ws13_12f2.data[88..120].to_vec(), ws13_roster_after_d, "window 13 roster hash moved after enroll:13current");
    let ws14_12f = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws14_12f.data[16..24].try_into().unwrap()), 60_000_000u64, "window 14 snapshot moved during slice 12");
    assert_eq!(u64::from_le_bytes(ws14_12f.data[64..72].try_into().unwrap()), 10_000_000u64, "window 14 exit stock not 10,000,000 after slice 12");
    let ws1_12f = accounts.iter().find(|(k, _)| *k == window_state_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws1_12f.data[64..72].try_into().unwrap()), 0u64, "window 1 exit stock moved during slice 12");
    let es12f = accounts.iter().find(|(k, _)| *k == exit_schedule_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(es12f.data[72..80].try_into().unwrap()), 2u64, "schedule tranches_executed moved during slice 12");
    assert_eq!(i64::from_le_bytes(es12f.data[96..104].try_into().unwrap()), 31_608_400i64, "schedule last_block_time moved during slice 12");
    assert_eq!(mollusk.sysvars.clock.unix_timestamp, 37_003_600i64, "clock not 37,003,600 after slice 12 (transition 21 lifecycle close)");

    // ---- Slice 13: W3.2 re-expression, part 5 — fee-flow edge cases under the single 73 bps stream (D-5) ----
    // The design (§4.2) says the 73 bps fee rides the venue flows in the compose layer and
    // `collect_fees` reconciles the window's collected total against the declared total; it does
    // NOT say any current instruction skims a fee. The program today matches that negative half
    // exactly: FEE_PPM = 7_300 is the only fee constant, no value-moving handler adds a fee term
    // to any amount, and the only FeeVault write anywhere is initialize_treasury zeroing
    // collected_raw. This slice proves the negative half on the chain instead of asserting it:
    // (a) a second real deposit credits EXACTLY the transferred amount — had a 73 bps fee been
    // skimmed at funding, a 10,000,000 deposit would credit 9,927,000, not 10,000,000 — and a
    // mid-window deposit does not inflate the current window's snapshot or stock (the caps read
    // the period-start snapshot, never the live total); (b) an all-accrued exit takes no penalty
    // and no fee: accrued consumed, position untouched, stock debited by exactly the gross
    // amount; (c) an all-excess exit's penalty is exactly the flat 13% (130,000 on 1,000,000),
    // because can_exit computes (excess * PENALTY_PPM / PPM, amount - penalty) with no FEE_PPM
    // term anywhere in it — the state proves the gross/position accounting, the formula proves
    // the paid figure, and neither admits a second skim; (d) FeeVault collected_raw is still 0
    // after every value move. That last read is stated, not passed off: the POSITIVE half of the
    // fee flow — collection and reconciliation — is unbuilt until the remaining §4.2
    // `collect_fees` row lands, so the 73 bps income identity has no on-chain realization yet
    // and the fee-flow class is only half re-expressed after this slice. As in every slice,
    // deposit moves real tokens (SPL CPI) while exit here is bookkeeping, not token movement.
    // (a) second deposit, 10,000,000 raw, on the slice-12 chain (source holds 40,000,000 after
    // slice 7; the vault already exists, so no vault-create CPI runs this time).
    let mut data13a = disc("deposit");
    data13a.extend_from_slice(&10_000_000u64.to_le_bytes());
    let ix13a = Instruction::new_with_bytes(
        program_id,
        &data13a,
        vec![
            AccountMeta::new(treasury_state, false),
            AccountMeta::new(depositor, true),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(source_key, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(token_program_id, false),
            AccountMeta::new_readonly(rent_id, false),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "deposit:13fee_exact", &ix13a, &accounts);
    accounts = next;
    measured.push(serde_json::json!({
        "instruction": "deposit:13fee_exact",
        "cu": cu,
        "result": "Success",
        "note": "slice-13 fee-flow probe (D-5): second real SPL deposit, 10,000,000 raw — credits TreasuryState by exactly 10,000,000 (a 73 bps skim at funding would credit 9,927,000), vault and source conserve exactly, FeeVault collected_raw stays 0; baseline (no §2.2 EST exists for deposit)",
    }));
    let ts13a = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts13a.data[8..16].try_into().unwrap()), 70_000_000u64, "TreasuryState total not 70,000,000 after deposit:13fee_exact (60,000,000 + exactly 10,000,000, no fee skim)");
    assert_eq!(u64::from_le_bytes(ts13a.data[16..24].try_into().unwrap()), 70_000_000u64, "TreasuryState cash not 70,000,000 after deposit:13fee_exact");
    assert_eq!(u64::from_le_bytes(ts13a.data[56..64].try_into().unwrap()), 15u64, "window_count moved during deposit:13fee_exact");
    let va13a = accounts.iter().find(|(k, _)| *k == vault).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(va13a.data[64..72].try_into().unwrap()), 70_000_000u64, "vault token amount not 70,000,000 after deposit:13fee_exact");
    let sa13a = accounts.iter().find(|(k, _)| *k == source_key).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(sa13a.data[64..72].try_into().unwrap()), 30_000_000u64, "source token amount not 30,000,000 after deposit:13fee_exact (conservation: 40,000,000 - 10,000,000)");
    let fv13a = accounts.iter().find(|(k, _)| *k == fee_vault).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fv13a.data[8..16].try_into().unwrap()), 0u64, "FeeVault collected_raw moved during deposit:13fee_exact (no instruction collects fees yet)");
    let ws14_13a = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws14_13a.data[16..24].try_into().unwrap()), 60_000_000u64, "window 14 snapshot inflated by mid-window deposit:13fee_exact (caps read the period-start snapshot, never the live total)");
    assert_eq!(u64::from_le_bytes(ws14_13a.data[64..72].try_into().unwrap()), 10_000_000u64, "window 14 exit stock moved during deposit:13fee_exact");
    // (b) holder 27 exits the slice-12 tranche at par: accrued 1,000,000 consumed, penalty 0,
    // position untouched, stock 10,000,000 -> 9,000,000, FeeVault still 0.
    let mut data13b = disc("exit");
    data13b.extend_from_slice(&7u64.to_le_bytes());
    data13b.extend_from_slice(&14u64.to_le_bytes());
    data13b.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix13b = Instruction::new_with_bytes(
        program_id,
        &data13b,
        vec![
            AccountMeta::new(holder_27, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new(window_state_14, false),
            AccountMeta::new(holder_record_27_14, false),
        ],
    );
    let (next, cu) = run(&mollusk, "exit:13accrued_par", &ix13b, &accounts);
    accounts = next;
    let gate13 = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "exit:13accrued_par",
        "cu": cu,
        "result": "Success",
        "note": "slice-13 GATED fee-flow probe (D-5): holder 27 exits the slice-12 tranche 1,000,000 at par on Committed window flow transition 7, window 14 — all-accrued exit takes neither penalty nor fee: accrued consumed, position stays 5,000,000, stock debited by exactly the gross amount",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate13,
    }));
    let hr13b = accounts.iter().find(|(k, _)| *k == holder_record_27_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr13b.data[64..72].try_into().unwrap()), 0u64, "holder 27 accrued not consumed by exit:13accrued_par");
    assert_eq!(u64::from_le_bytes(hr13b.data[56..64].try_into().unwrap()), 5_000_000u64, "holder 27 position moved during exit:13accrued_par (an all-accrued exit takes no penalty and no fee)");
    let ws14_13b = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws14_13b.data[64..72].try_into().unwrap()), 9_000_000u64, "window 14 exit stock not 9,000,000 after exit:13accrued_par");
    let fv13b = accounts.iter().find(|(k, _)| *k == fee_vault).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fv13b.data[8..16].try_into().unwrap()), 0u64, "FeeVault collected_raw moved during exit:13accrued_par");
    // (c) holder 27 exits 1,000,000 of pure excess: penalty exactly 13% (130,000, paid 870,000
    // by the can_exit formula, which carries no FEE_PPM term), position 5,000,000 -> 4,000,000,
    // stock 9,000,000 -> 8,000,000, FeeVault still 0. A second skim would have to live in the
    // gate math; it does not — the only fee constant is never read by can_exit.
    let mut data13c = disc("exit");
    data13c.extend_from_slice(&7u64.to_le_bytes());
    data13c.extend_from_slice(&14u64.to_le_bytes());
    data13c.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix13c = Instruction::new_with_bytes(
        program_id,
        &data13c,
        vec![
            AccountMeta::new(holder_27, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new(window_state_14, false),
            AccountMeta::new(holder_record_27_14, false),
        ],
    );
    let (next, cu) = run(&mollusk, "exit:13excess_penalty", &ix13c, &accounts);
    accounts = next;
    let gate13 = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "exit:13excess_penalty",
        "cu": cu,
        "result": "Success",
        "note": "slice-13 GATED fee-flow probe (D-5): holder 27 exits 1,000,000 of pure excess on Committed window flow transition 7, window 14 — penalty exactly the flat 13% (130,000; paid 870,000 by can_exit, which has no FEE_PPM term, so no second skim exists to take), position 5,000,000 -> 4,000,000",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate13,
    }));
    let hr13c = accounts.iter().find(|(k, _)| *k == holder_record_27_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr13c.data[64..72].try_into().unwrap()), 0u64, "holder 27 accrued moved during exit:13excess_penalty");
    assert_eq!(u64::from_le_bytes(hr13c.data[56..64].try_into().unwrap()), 4_000_000u64, "holder 27 position not 4,000,000 after exit:13excess_penalty");
    let ws14_13c = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws14_13c.data[64..72].try_into().unwrap()), 8_000_000u64, "window 14 exit stock not 8,000,000 after exit:13excess_penalty");
    // Final slice-13 state: the fee flow's negative half is proven, its positive half is not —
    // FeeVault collected_raw is still 0 because collect_fees is unbuilt, and that is recorded
    // as the carried gap, not hidden behind the passing probes.
    let ts13f = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts13f.data[8..16].try_into().unwrap()), 70_000_000u64, "TreasuryState total moved during slice 13");
    assert_eq!(u64::from_le_bytes(ts13f.data[16..24].try_into().unwrap()), 70_000_000u64, "TreasuryState cash moved during slice 13");
    assert_eq!(u64::from_le_bytes(ts13f.data[56..64].try_into().unwrap()), 15u64, "window_count moved during slice 13");
    let ws13_13f = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws13_13f.data[64..72].try_into().unwrap()), 4_000_000u64, "window 13 exit stock moved during slice 13");
    let ws1_13f = accounts.iter().find(|(k, _)| *k == window_state_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws1_13f.data[64..72].try_into().unwrap()), 0u64, "window 1 exit stock moved during slice 13");
    let es13f = accounts.iter().find(|(k, _)| *k == exit_schedule_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(es13f.data[72..80].try_into().unwrap()), 2u64, "schedule tranches_executed moved during slice 13");
    assert_eq!(i64::from_le_bytes(es13f.data[96..104].try_into().unwrap()), 31_608_400i64, "schedule last_block_time moved during slice 13");
    let fv13f = accounts.iter().find(|(k, _)| *k == fee_vault).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fv13f.data[8..16].try_into().unwrap()), 0u64, "FeeVault collected_raw not 0 after slice 13 (collect_fees is unbuilt; a nonzero value here would mean an undocumented collection path)");

    // ---- Slice 14: the remaining §4.2 row `reinvest` (design §4.2, engine-sim T8) ----
    // The program changed under this slice (new instruction, IDL 37 -> 38, errors
    // 85 -> 88, .so sha 82759c51... -> e0f40adf...), so slices 1-13 are re-measured
    // here against the new .so and the diff against the slice-13 golden is reported
    // in the run log — a program change must never silently re-baseline a golden
    // file. The reading implemented, stated plainly because the design row is one
    // line ("claims no cash; increases the position"): the reinvested amount comes
    // out of the holder's accrued unpaid dividend on THIS record and into the same
    // record's locked position — accrued falls by exactly the amount, position rises
    // by exactly the amount, nothing else moves. A partial reinvest is allowed; an
    // amount above the accrued dividend refuses with ReinvestExceedsAccrued (6087),
    // never a partial reinvest of a larger request. No exit stock is debited (the
    // stock is the period budget for cash exits, and a reinvest pays no cash), no
    // penalty is taken (the 13% door prices cash extraction above the allowance),
    // and no TreasuryState total moves (pay_tranche already accounted the dividend;
    // reinvesting re-labels it as position, it does not create value). Holder 27
    // cannot carry this slice — slice 13 consumed its whole tranche — so a fresh
    // holder 29 enrolls in the still-current window 14, collects a 1,000,000 tranche
    // on Committed window-flow transition 7, and reinvests it in two parts.
    // (a) holder 29 enrolls in window 14 (setup): cohort 2 (14 % 12), position 5,000,000.
    let holder_29 = Pubkey::new_from_array([29u8; 32]);
    let (holder_record_29_14, _) = pda(&[b"holder", &14u64.to_le_bytes(), holder_29.as_ref()], &program_id);
    upsert(&mut accounts, (holder_29, funded()));
    upsert(&mut accounts, (holder_record_29_14, empty.clone()));
    let mut data14a = disc("enroll");
    data14a.extend_from_slice(&14u64.to_le_bytes());
    data14a.extend_from_slice(&5_000_000u64.to_le_bytes());
    let (forfeiture_holder_29, _) = pda(&[b"forfeiture", holder_29.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_29) { accounts.push((forfeiture_holder_29, empty.clone())); }
    let ix14a = Instruction::new_with_bytes(
        program_id,
        &data14a,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_14, false),
            AccountMeta::new(holder_record_29_14, false),
            AccountMeta::new(forfeiture_holder_29, false),
            AccountMeta::new(holder_29, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll:14holder29", &ix14a, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "enroll:14holder29", "cu": cu, "result": "Success", "note": "slice-14 setup: holder 29 enrolls in the current window 14 (cohort 2, position 5,000,000) — the reinvest subject; holder 27's tranche is spent, so a fresh record is required"}));
    let hr14a = accounts.iter().find(|(k, _)| *k == holder_record_29_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr14a.data[48..56].try_into().unwrap()), 2u64, "holder 29 cohort not 2 (14 % 12)");
    assert_eq!(u64::from_le_bytes(hr14a.data[56..64].try_into().unwrap()), 5_000_000u64, "holder 29 position not 5,000,000 after enroll:14holder29");
    assert_eq!(hr14a.data[89], 1u8, "holder 29 standing authorization not true at enroll:14holder29");
    // (b) the fresh cohort's tranche: pay_tranche 1,000,000 on holder 29 in window 14
    // (slot 14 % 12 = 2, holder cohort 2) on Committed window-flow transition 7, gated §2.2 ~7K.
    let mut data14b = disc("pay_tranche");
    data14b.extend_from_slice(&7u64.to_le_bytes());
    data14b.extend_from_slice(&14u64.to_le_bytes());
    data14b.extend_from_slice(holder_29.as_ref());
    data14b.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix14b = Instruction::new_with_bytes(
        program_id,
        &data14b,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_14, false),
            AccountMeta::new(holder_record_29_14, false),
        ],
    );
    let (next, cu) = run(&mollusk, "pay_tranche:14holder29", &ix14b, &accounts);
    accounts = next;
    let gate14 = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "pay_tranche:14holder29",
        "cu": cu,
        "result": "Success",
        "note": "slice-14 setup, GATED: operator pay_tranche 1,000,000 on holder 29 in window 14 on Committed window flow transition 7 — the dividend the reinvest probes then fold into position",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate14,
    }));
    let hr14b = accounts.iter().find(|(k, _)| *k == holder_record_29_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr14b.data[64..72].try_into().unwrap()), 1_000_000u64, "holder 29 accrued not 1,000,000 after pay_tranche:14holder29");
    assert_eq!(hr14b.data[98], 1u8, "holder 29 tranche_paid flag not set by pay_tranche:14holder29");
    assert_eq!(u64::from_le_bytes(hr14b.data[56..64].try_into().unwrap()), 5_000_000u64, "holder 29 position moved during pay_tranche:14holder29");
    // (c) partial reinvest: 400,000 of the 1,000,000 accrued dividend folds into the
    // position — accrued 1,000,000 -> 600,000, position 5,000,000 -> 5,400,000, and
    // NOTHING else moves: window 14 stock stays 8,000,000 (no cash claim, no stock
    // debit), TreasuryState stays 70,000,000 (no value created), FeeVault stays 0.
    let mut data14c = disc("reinvest");
    data14c.extend_from_slice(&7u64.to_le_bytes());
    data14c.extend_from_slice(&14u64.to_le_bytes());
    data14c.extend_from_slice(&400_000u64.to_le_bytes());
    let ix14c = Instruction::new_with_bytes(
        program_id,
        &data14c,
        vec![
            AccountMeta::new(holder_29, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_14, false),
            AccountMeta::new(holder_record_29_14, false),
        ],
    );
    let (next, cu) = run(&mollusk, "reinvest:14partial", &ix14c, &accounts);
    accounts = next;
    let gate14 = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "reinvest:14partial",
        "cu": cu,
        "result": "Success",
        "note": "slice-14 FIRST reinvest measurement (design §4.2, T8): holder 29 reinvests 400,000 of a 1,000,000 accrued dividend on Committed window flow transition 7, window 14 — accrued 1,000,000 -> 600,000, position 5,000,000 -> 5,400,000, window 14 exit stock UNCHANGED at 8,000,000 (claims no cash), TreasuryState UNCHANGED (re-labels dividend as position, creates nothing)",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate14,
    }));
    let hr14c = accounts.iter().find(|(k, _)| *k == holder_record_29_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr14c.data[64..72].try_into().unwrap()), 600_000u64, "holder 29 accrued not 600,000 after reinvest:14partial");
    assert_eq!(u64::from_le_bytes(hr14c.data[56..64].try_into().unwrap()), 5_400_000u64, "holder 29 position not 5,400,000 after reinvest:14partial");
    let ws14_14c = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws14_14c.data[64..72].try_into().unwrap()), 8_000_000u64, "window 14 exit stock moved during reinvest:14partial (a reinvest claims no cash and debits no stock)");
    // (d) over-accrued reinvest MUST refuse: 600,001 against 600,000 accrued refuses
    // ReinvestExceedsAccrued (6087) — never a partial reinvest of a larger request —
    // and changes no state.
    let mut data14d = disc("reinvest");
    data14d.extend_from_slice(&7u64.to_le_bytes());
    data14d.extend_from_slice(&14u64.to_le_bytes());
    data14d.extend_from_slice(&600_001u64.to_le_bytes());
    let ix14d = Instruction::new_with_bytes(
        program_id,
        &data14d,
        vec![
            AccountMeta::new(holder_29, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_14, false),
            AccountMeta::new(holder_record_29_14, false),
        ],
    );
    let cu14d = run_expect_err(&mollusk, "reinvest:14over_accrued", &ix14d, &accounts, 6087);
    measured.push(serde_json::json!({
        "instruction": "reinvest:14over_accrued",
        "cu": cu14d,
        "result": "Refused",
        "error": "ReinvestExceedsAccrued",
        "error_code": 6087,
        "note": "slice-14 refusal: holder 29 reinvests 600,001 against 600,000 accrued — refuses ReinvestExceedsAccrued (6087), never a partial reinvest of a larger request; state unchanged, NOT a success CU",
    }));
    let hr14d = accounts.iter().find(|(k, _)| *k == holder_record_29_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr14d.data[64..72].try_into().unwrap()), 600_000u64, "holder 29 accrued moved during reinvest:14over_accrued refusal");
    assert_eq!(u64::from_le_bytes(hr14d.data[56..64].try_into().unwrap()), 5_400_000u64, "holder 29 position moved during reinvest:14over_accrued refusal");
    // (e) the rest reinvests at the inclusive boundary: 600,000 of 600,000 accrued —
    // accrued -> 0, position 5,400,000 -> 6,000,000, stock still 8,000,000.
    let mut data14e = disc("reinvest");
    data14e.extend_from_slice(&7u64.to_le_bytes());
    data14e.extend_from_slice(&14u64.to_le_bytes());
    data14e.extend_from_slice(&600_000u64.to_le_bytes());
    let ix14e = Instruction::new_with_bytes(
        program_id,
        &data14e,
        vec![
            AccountMeta::new(holder_29, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_14, false),
            AccountMeta::new(holder_record_29_14, false),
        ],
    );
    let (next, cu) = run(&mollusk, "reinvest:14rest", &ix14e, &accounts);
    accounts = next;
    let gate14 = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "reinvest:14rest",
        "cu": cu,
        "result": "Success",
        "note": "slice-14: holder 29 reinvests the remaining 600,000 accrued dividend at the inclusive boundary — accrued -> 0, position 5,400,000 -> 6,000,000, window 14 exit stock still 8,000,000",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate14,
    }));
    let hr14e = accounts.iter().find(|(k, _)| *k == holder_record_29_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr14e.data[64..72].try_into().unwrap()), 0u64, "holder 29 accrued not 0 after reinvest:14rest");
    assert_eq!(u64::from_le_bytes(hr14e.data[56..64].try_into().unwrap()), 6_000_000u64, "holder 29 position not 6,000,000 after reinvest:14rest");
    // (f) a zero reinvest MUST refuse InvalidReinvestAmount (6086): it would consume
    // a window execution while moving nothing.
    let mut data14f = disc("reinvest");
    data14f.extend_from_slice(&7u64.to_le_bytes());
    data14f.extend_from_slice(&14u64.to_le_bytes());
    data14f.extend_from_slice(&0u64.to_le_bytes());
    let ix14f = Instruction::new_with_bytes(
        program_id,
        &data14f,
        vec![
            AccountMeta::new(holder_29, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_14, false),
            AccountMeta::new(holder_record_29_14, false),
        ],
    );
    let cu14f = run_expect_err(&mollusk, "reinvest:14zero", &ix14f, &accounts, 6086);
    measured.push(serde_json::json!({
        "instruction": "reinvest:14zero",
        "cu": cu14f,
        "result": "Refused",
        "error": "InvalidReinvestAmount",
        "error_code": 6086,
        "note": "slice-14 refusal: holder 29 reinvests 0 — refuses InvalidReinvestAmount (6086); a zero reinvestment would consume a window execution while moving nothing, NOT a success CU",
    }));
    // Final slice-14 state: the whole tranche is now position, nothing else moved.
    let hr14f = accounts.iter().find(|(k, _)| *k == holder_record_29_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr14f.data[64..72].try_into().unwrap()), 0u64, "holder 29 accrued moved during reinvest:14zero refusal");
    assert_eq!(u64::from_le_bytes(hr14f.data[56..64].try_into().unwrap()), 6_000_000u64, "holder 29 position moved during reinvest:14zero refusal");
    let ts14f = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts14f.data[8..16].try_into().unwrap()), 70_000_000u64, "TreasuryState total moved during slice 14 (reinvest creates no value)");
    assert_eq!(u64::from_le_bytes(ts14f.data[16..24].try_into().unwrap()), 70_000_000u64, "TreasuryState cash moved during slice 14");
    assert_eq!(u64::from_le_bytes(ts14f.data[56..64].try_into().unwrap()), 15u64, "window_count moved during slice 14");
    let ws14_14f = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws14_14f.data[64..72].try_into().unwrap()), 8_000_000u64, "window 14 exit stock moved during slice 14 (reinvest debits no stock)");
    let ws13_14f = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws13_14f.data[64..72].try_into().unwrap()), 4_000_000u64, "window 13 exit stock moved during slice 14");
    let ws1_14f = accounts.iter().find(|(k, _)| *k == window_state_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws1_14f.data[64..72].try_into().unwrap()), 0u64, "window 1 exit stock moved during slice 14");
    let es14f = accounts.iter().find(|(k, _)| *k == exit_schedule_1).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(es14f.data[72..80].try_into().unwrap()), 2u64, "schedule tranches_executed moved during slice 14");
    assert_eq!(i64::from_le_bytes(es14f.data[96..104].try_into().unwrap()), 31_608_400i64, "schedule last_block_time moved during slice 14");
    let fv14f = accounts.iter().find(|(k, _)| *k == fee_vault).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fv14f.data[8..16].try_into().unwrap()), 0u64, "FeeVault collected_raw not 0 after slice 14 (collect_fees is still unbuilt; reinvest touches no fees)");
    assert_eq!(mollusk.sysvars.clock.unix_timestamp, 37_003_600i64, "clock not 37,003,600 after slice 14");

    // ---- Slice 15: the last §4.2 row `collect_fees` (design §4.2, D-5) ----
    // The program changed under this slice (new instruction, IDL 38 -> 39,
    // errors 88 -> 91, .so sha e0f40adf... -> the slice-15 build), so slices
    // 1-14 are re-measured here against the new .so and the diff against the
    // slice-14 golden is reported in the run log — a program change must never
    // silently re-baseline a golden file. The reading implemented, stated
    // plainly because the design row is one paragraph with a gap in it: the
    // operator reports the window's collected 73 bps total under the window's
    // Committed transition with the OPERATOR_SETTLE capability, the FeeVault
    // key must equal Config.fee_vault, and the amount accrues twice — once
    // on the window's fees_collected_raw (the per-window accounting) and once
    // on the vault's collected_raw (the sum over windows). A zero collection
    // refuses, and a capable-operator check is what refuses an agent without
    // OPERATOR_SETTLE. Stated, not passed off: this is the accounting half,
    // bookkeeping not token movement, as in the sibling slices. Slice 18
    // closes the design row's reconciliation gap for this chain: the window's
    // fee schedule is declared FIRST (100,000, below), so the two
    // collections that follow (73,000 + 27,000) land exactly on the declared
    // total — a collection past it would refuse, and a window with no
    // declaration collects nothing (the slice-18 probes after slice 17 prove
    // both). The window is still 14, the transition still Committed
    // transition 7, the operator still [7;32].
    // Slice 18 declaration: operator declares window 14's fee schedule,
    // 100,000, under Committed transition 7 — before any collection, as
    // the reconciliation requires. WindowState layout after slice 18:
    // fees_collected_raw at [121..129], declared_fees_raw at [129..137],
    // fee_schedule_declared flag at [137].
    let mut data18d = disc("declare_fee_schedule");
    data18d.extend_from_slice(&7u64.to_le_bytes());
    data18d.extend_from_slice(&14u64.to_le_bytes());
    data18d.extend_from_slice(&100_000u64.to_le_bytes());
    let ix18d = Instruction::new_with_bytes(
        program_id,
        &data18d,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new(window_state_14, false),
        ],
    );
    let (next, cu) = run(&mollusk, "declare_fee_schedule:18", &ix18d, &accounts);
    accounts = next;
    measured.push(serde_json::json!({
        "instruction": "declare_fee_schedule:18",
        "cu": cu,
        "result": "Success",
        "note": "slice-18 declaration (design §4.2, D-5): operator declares window 14's fee schedule, 100,000, under Committed window flow transition 7 — window 14 declared_fees_raw 0 -> 100,000, fee_schedule_declared false -> true, fees_collected_raw still 0, FeeVault still 0; no §2.2 estimate exists for this instruction, so this value is a baseline, not a gated figure",
    }));
    let ws18d = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws18d.data[129..137].try_into().unwrap()), 100_000u64, "window 14 declared_fees_raw not 100,000 after declare_fee_schedule:18");
    assert_eq!(ws18d.data[137], 1u8, "window 14 fee_schedule_declared flag not set after declare_fee_schedule:18");
    assert_eq!(u64::from_le_bytes(ws18d.data[121..129].try_into().unwrap()), 0u64, "window 14 fees_collected_raw moved during declare_fee_schedule:18 (a declaration collects nothing)");
    // (a) first collection: 73,000 — 73 bps on a 10,000,000 flow, the
    // operator-reported window total for this window's compose-layer flows.
    let mut data15a = disc("collect_fees");
    data15a.extend_from_slice(&7u64.to_le_bytes());
    data15a.extend_from_slice(&14u64.to_le_bytes());
    data15a.extend_from_slice(&73_000u64.to_le_bytes());
    let ix15a = Instruction::new_with_bytes(
        program_id,
        &data15a,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(window_state_14, false),
            AccountMeta::new(fee_vault, false),
        ],
    );
    let (next, cu) = run(&mollusk, "collect_fees:15first", &ix15a, &accounts);
    accounts = next;
    let gate15 = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "collect_fees:15first",
        "cu": cu,
        "result": "Success",
        "note": "slice-15 FIRST collect_fees measurement (design §4.2, D-5): operator collects 73,000 (73 bps on a 10,000,000 flow, operator-reported) in window 14 on Committed window flow transition 7 — window 14 fees_collected_raw 0 -> 73,000, FeeVault collected_raw 0 -> 73,000, window 14 exit stock UNCHANGED at 8,000,000 (a collection claims no cash), TreasuryState UNCHANGED at 70,000,000",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate15,
    }));
    let ws15a = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws15a.data[121..129].try_into().unwrap()), 73_000u64, "window 14 fees_collected_raw not 73,000 after collect_fees:15first");
    assert_eq!(u64::from_le_bytes(ws15a.data[64..72].try_into().unwrap()), 8_000_000u64, "window 14 exit stock moved during collect_fees:15first (a collection claims no cash)");
    let fv15a = accounts.iter().find(|(k, _)| *k == fee_vault).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fv15a.data[8..16].try_into().unwrap()), 73_000u64, "FeeVault collected_raw not 73,000 after collect_fees:15first");
    // (b) a second collection in the same window ACCUMULATES: 27,000 more
    // brings the window total and the vault total both to 100,000. Fees
    // arrive through a window, not once, so no single-collection flag is
    // imposed — each call is a distinct reported collection.
    let mut data15b = disc("collect_fees");
    data15b.extend_from_slice(&7u64.to_le_bytes());
    data15b.extend_from_slice(&14u64.to_le_bytes());
    data15b.extend_from_slice(&27_000u64.to_le_bytes());
    let ix15b = Instruction::new_with_bytes(
        program_id,
        &data15b,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(window_state_14, false),
            AccountMeta::new(fee_vault, false),
        ],
    );
    let (next, cu) = run(&mollusk, "collect_fees:15second", &ix15b, &accounts);
    accounts = next;
    let gate15 = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "collect_fees:15second",
        "cu": cu,
        "result": "Success",
        "note": "slice-15 accumulation: operator collects a further 27,000 in window 14 — window fees_collected_raw 73,000 -> 100,000 and FeeVault collected_raw 73,000 -> 100,000, so the vault total is the sum over windows and the window figure is its own accounting, not a second total",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate15,
    }));
    let ws15b = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws15b.data[121..129].try_into().unwrap()), 100_000u64, "window 14 fees_collected_raw not 100,000 after collect_fees:15second");
    let fv15b = accounts.iter().find(|(k, _)| *k == fee_vault).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fv15b.data[8..16].try_into().unwrap()), 100_000u64, "FeeVault collected_raw not 100,000 after collect_fees:15second");
    // (c) a zero collection MUST refuse InvalidCollectAmount (6090): it
    // would consume a window execution while accounting nothing.
    let mut data15c = disc("collect_fees");
    data15c.extend_from_slice(&7u64.to_le_bytes());
    data15c.extend_from_slice(&14u64.to_le_bytes());
    data15c.extend_from_slice(&0u64.to_le_bytes());
    let ix15c = Instruction::new_with_bytes(
        program_id,
        &data15c,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(window_state_14, false),
            AccountMeta::new(fee_vault, false),
        ],
    );
    let cu15c = run_expect_err(&mollusk, "collect_fees:15zero", &ix15c, &accounts, 6090);
    measured.push(serde_json::json!({
        "instruction": "collect_fees:15zero",
        "cu": cu15c,
        "result": "Refused",
        "error": "InvalidCollectAmount",
        "error_code": 6090,
        "note": "slice-15 refusal: operator collects 0 in window 14 — refuses InvalidCollectAmount (6090); a zero collection would consume a window execution while accounting nothing, NOT a success CU",
    }));
    let ws15c = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws15c.data[121..129].try_into().unwrap()), 100_000u64, "window 14 fees_collected_raw moved during collect_fees:15zero refusal");
    let fv15c = accounts.iter().find(|(k, _)| *k == fee_vault).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fv15c.data[8..16].try_into().unwrap()), 100_000u64, "FeeVault collected_raw moved during collect_fees:15zero refusal");
    // (d) an agent WITHOUT OPERATOR_SETTLE MUST refuse Unauthorized (6000):
    // agent [3;32] is registered and active but holds only the slice-1
    // zero capability, never the operator capability. The signer account
    // itself must be present in the harness map (mollusk requires every
    // account an instruction touches, signer included) — upserted here
    // as a funded system account, the same pattern as holder 29's setup.
    // The vault-mismatch case cannot be probed here — the vault account
    // is PDA-bound, so a foreign vault fails at the account layer before
    // the handler — and is covered by the host gate matrix instead.
    upsert(&mut accounts, (agent, funded()));
    let mut data15d = disc("collect_fees");
    data15d.extend_from_slice(&7u64.to_le_bytes());
    data15d.extend_from_slice(&14u64.to_le_bytes());
    data15d.extend_from_slice(&1_000u64.to_le_bytes());
    let ix15d = Instruction::new_with_bytes(
        program_id,
        &data15d,
        vec![
            AccountMeta::new(agent, true),
            AccountMeta::new_readonly(agent_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(window_state_14, false),
            AccountMeta::new(fee_vault, false),
        ],
    );
    let cu15d = run_expect_err(&mollusk, "collect_fees:15unauthorized", &ix15d, &accounts, 6000);
    measured.push(serde_json::json!({
        "instruction": "collect_fees:15unauthorized",
        "cu": cu15d,
        "result": "Refused",
        "error": "Unauthorized",
        "error_code": 6000,
        "note": "slice-15 refusal: agent [3;32] (registered, active, no OPERATOR_SETTLE) collects 1,000 in window 14 — refuses Unauthorized (6000); capability-gating, not a named operator key, is what authorizes collection, NOT a success CU",
    }));
    let ws15d = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws15d.data[121..129].try_into().unwrap()), 100_000u64, "window 14 fees_collected_raw moved during collect_fees:15unauthorized refusal");
    let fv15d = accounts.iter().find(|(k, _)| *k == fee_vault).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fv15d.data[8..16].try_into().unwrap()), 100_000u64, "FeeVault collected_raw moved during collect_fees:15unauthorized refusal");
    // Slice-17 probe: reusing Settled transition 21 — which already
    // opened window 14 — to open window 15 MUST refuse
    // SourceTransitionConsumed (6092). This is the shape the
    // counter cannot see: window_count is 15, so id 15 passes the
    // sequence check, and only the consumption flag refuses.
    // The transition account is passed writable here because the
    // handler now mutates the flag; the refusal discards the
    // threaded accounts, so no window 15 is created by the probe.
    let mut data17 = disc("open_window");
    data17.extend_from_slice(&21u64.to_le_bytes());
    data17.extend_from_slice(&15u64.to_le_bytes());
    let ix17 = Instruction::new_with_bytes(
        program_id,
        &data17,
        vec![
            AccountMeta::new(treasury_state, false),
            AccountMeta::new_readonly(lending_book, false),
            AccountMeta::new(transition_21, false),
            AccountMeta::new(window_state_15, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let cu17 = run_expect_err(&mollusk, "open_window:15reuse", &ix17, &accounts, 6092);
    measured.push(serde_json::json!({
        "instruction": "open_window:15reuse",
        "cu": cu17,
        "result": "Refused",
        "error": "SourceTransitionConsumed",
        "error_code": 6092,
        "note": "slice-17 refusal: Settled transition 21 already opened window 14, so opening window 15 on it refuses SourceTransitionConsumed (6092) — one Settled treasury.window.v1 transition authorizes exactly one window open; window_count stays 15, no window 15 is created, NOT a success CU",
    }));
    let ts17 = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts17.data[56..64].try_into().unwrap()), 15u64, "window_count moved during open_window:15reuse refusal");
    // Final slice-15 state: the fee flow now has its positive half on-chain
    // as accounting — window 14 collected 100,000, vault total 100,000 —
    // and nothing else moved: the tranche, the reinvested position, the
    // stocks, and the treasury totals are exactly the slice-14 finals.
    let ts15f = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts15f.data[8..16].try_into().unwrap()), 70_000_000u64, "TreasuryState total moved during slice 15 (a collection creates no value)");
    assert_eq!(u64::from_le_bytes(ts15f.data[16..24].try_into().unwrap()), 70_000_000u64, "TreasuryState cash moved during slice 15");
    assert_eq!(u64::from_le_bytes(ts15f.data[56..64].try_into().unwrap()), 15u64, "window_count moved during slice 15");
    let ws15f = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws15f.data[64..72].try_into().unwrap()), 8_000_000u64, "window 14 exit stock moved during slice 15 (a collection claims no cash)");
    assert_eq!(mollusk.sysvars.clock.unix_timestamp, 37_003_600i64, "clock not 37,003,600 after slice 15");

    // ---- Slice 18 probes: the declared-fee-schedule reconciliation ----
    // (a) a second declaration for window 14 MUST refuse
    // FeeScheduleAlreadyDeclared (6093): the schedule is one-time, so it
    // cannot be raised after collections begin — even to the same total.
    let mut data18a = disc("declare_fee_schedule");
    data18a.extend_from_slice(&7u64.to_le_bytes());
    data18a.extend_from_slice(&14u64.to_le_bytes());
    data18a.extend_from_slice(&100_000u64.to_le_bytes());
    let ix18a = Instruction::new_with_bytes(
        program_id,
        &data18a,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new(window_state_14, false),
        ],
    );
    let cu18a = run_expect_err(&mollusk, "declare_fee_schedule:18redeclare", &ix18a, &accounts, 6093);
    measured.push(serde_json::json!({
        "instruction": "declare_fee_schedule:18redeclare",
        "cu": cu18a,
        "result": "Refused",
        "error": "FeeScheduleAlreadyDeclared",
        "error_code": 6093,
        "note": "slice-18 refusal: a second declaration for window 14 refuses FeeScheduleAlreadyDeclared (6093) — the schedule is one-time and cannot be raised after collections, NOT a success CU",
    }));
    let ws18a = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws18a.data[129..137].try_into().unwrap()), 100_000u64, "window 14 declared_fees_raw moved during declare_fee_schedule:18redeclare refusal");
    assert_eq!(u64::from_le_bytes(ws18a.data[121..129].try_into().unwrap()), 100_000u64, "window 14 fees_collected_raw moved during declare_fee_schedule:18redeclare refusal");
    // (b) one unit past the declared total MUST refuse
    // CollectExceedsDeclaredSchedule (6095): window 14 is collected
    // exactly to its 100,000 declaration, so any further positive
    // collection is over-declared, however small.
    let mut data18b = disc("collect_fees");
    data18b.extend_from_slice(&7u64.to_le_bytes());
    data18b.extend_from_slice(&14u64.to_le_bytes());
    data18b.extend_from_slice(&1u64.to_le_bytes());
    let ix18b = Instruction::new_with_bytes(
        program_id,
        &data18b,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(window_state_14, false),
            AccountMeta::new(fee_vault, false),
        ],
    );
    let cu18b = run_expect_err(&mollusk, "collect_fees:18over", &ix18b, &accounts, 6095);
    measured.push(serde_json::json!({
        "instruction": "collect_fees:18over",
        "cu": cu18b,
        "result": "Refused",
        "error": "CollectExceedsDeclaredSchedule",
        "error_code": 6095,
        "note": "slice-18 refusal: collecting 1 more in window 14 (already collected exactly to its 100,000 declaration) refuses CollectExceedsDeclaredSchedule (6095) — the vault total is bounded per window by the prior declaration, NOT a success CU",
    }));
    let ws18b = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws18b.data[121..129].try_into().unwrap()), 100_000u64, "window 14 fees_collected_raw moved during collect_fees:18over refusal");
    let fv18b = accounts.iter().find(|(k, _)| *k == fee_vault).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fv18b.data[8..16].try_into().unwrap()), 100_000u64, "FeeVault collected_raw moved during collect_fees:18over refusal");
    // (c) a collection in an UNDECLARED window MUST refuse
    // FeeScheduleUndeclared (6094): window 13 has no schedule (its
    // declared flag is still false), so no collection is possible
    // there, even under the same Committed transition type — the
    // schedule is per window, not per transition.
    let mut data18c = disc("collect_fees");
    data18c.extend_from_slice(&7u64.to_le_bytes());
    data18c.extend_from_slice(&13u64.to_le_bytes());
    data18c.extend_from_slice(&1_000u64.to_le_bytes());
    let ix18c = Instruction::new_with_bytes(
        program_id,
        &data18c,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(window_state_13, false),
            AccountMeta::new(fee_vault, false),
        ],
    );
    let cu18c = run_expect_err(&mollusk, "collect_fees:18undeclared", &ix18c, &accounts, 6094);
    measured.push(serde_json::json!({
        "instruction": "collect_fees:18undeclared",
        "cu": cu18c,
        "result": "Refused",
        "error": "FeeScheduleUndeclared",
        "error_code": 6094,
        "note": "slice-18 refusal: collecting 1,000 in window 13 (no fee schedule declared) refuses FeeScheduleUndeclared (6094) — no schedule, no collection, NOT a success CU",
    }));
    let ws18c = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(ws18c.data[137], 0u8, "window 13 fee_schedule_declared flag moved during collect_fees:18undeclared refusal");
    assert_eq!(u64::from_le_bytes(ws18c.data[121..129].try_into().unwrap()), 0u64, "window 13 fees_collected_raw moved during collect_fees:18undeclared refusal");
    // (d) a declared ZERO is a real declaration, not an undeclared
    // window: declaring window 13's schedule at 0 succeeds, and a
    // further 1-unit collection then refuses as over-declared
    // (6095), not as undeclared — the flag, not a zero total,
    // distinguishes the two states.
    let mut data18d2 = disc("declare_fee_schedule");
    data18d2.extend_from_slice(&7u64.to_le_bytes());
    data18d2.extend_from_slice(&13u64.to_le_bytes());
    data18d2.extend_from_slice(&0u64.to_le_bytes());
    let ix18d2 = Instruction::new_with_bytes(
        program_id,
        &data18d2,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new(window_state_13, false),
        ],
    );
    let (next, cu) = run(&mollusk, "declare_fee_schedule:18zero", &ix18d2, &accounts);
    accounts = next;
    measured.push(serde_json::json!({
        "instruction": "declare_fee_schedule:18zero",
        "cu": cu,
        "result": "Success",
        "note": "slice-18 declaration: operator declares window 13's fee schedule at 0 — a declared zero is a real declaration (no fees this window), not an undeclared window; no §2.2 estimate exists for this instruction, so this value is a baseline, not a gated figure",
    }));
    let ws18d2 = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(ws18d2.data[137], 1u8, "window 13 fee_schedule_declared flag not set after declare_fee_schedule:18zero");
    assert_eq!(u64::from_le_bytes(ws18d2.data[129..137].try_into().unwrap()), 0u64, "window 13 declared_fees_raw not 0 after declare_fee_schedule:18zero");
    let mut data18e = disc("collect_fees");
    data18e.extend_from_slice(&7u64.to_le_bytes());
    data18e.extend_from_slice(&13u64.to_le_bytes());
    data18e.extend_from_slice(&1u64.to_le_bytes());
    let ix18e = Instruction::new_with_bytes(
        program_id,
        &data18e,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(window_state_13, false),
            AccountMeta::new(fee_vault, false),
        ],
    );
    let cu18e = run_expect_err(&mollusk, "collect_fees:18zeroexceeds", &ix18e, &accounts, 6095);
    measured.push(serde_json::json!({
        "instruction": "collect_fees:18zeroexceeds",
        "cu": cu18e,
        "result": "Refused",
        "error": "CollectExceedsDeclaredSchedule",
        "error_code": 6095,
        "note": "slice-18 refusal: collecting 1 in window 13 (declared schedule 0) refuses CollectExceedsDeclaredSchedule (6095), not FeeScheduleUndeclared — a declared zero is a schedule, NOT a success CU",
    }));
    let ws18e = accounts.iter().find(|(k, _)| *k == window_state_13).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws18e.data[121..129].try_into().unwrap()), 0u64, "window 13 fees_collected_raw moved during collect_fees:18zeroexceeds refusal");

    // ---- Slice 19: deposit↔enroll binding probes, window 14 ----
    // Window 14's funded snapshot is 60,000,000 and its enrolled
    // total after slices 12/14 is 10,000,000 (holder 27's
    // 5,000,000 + holder 29's 5,000,000), so a 50,000,000
    // enrollment lands exactly on the snapshot, and 1 more
    // refuses. Window 14 is still the current window
    // (window_count 15, current 14).
    let holder_30 = Pubkey::new_from_array([30u8; 32]);
    let (holder_record_30_14, _) = pda(&[b"holder", &14u64.to_le_bytes(), holder_30.as_ref()], &program_id);
    upsert(&mut accounts, (holder_30, funded()));
    upsert(&mut accounts, (holder_record_30_14, empty.clone()));
    let mut data19a = disc("enroll");
    data19a.extend_from_slice(&14u64.to_le_bytes());
    data19a.extend_from_slice(&50_000_000u64.to_le_bytes());
    let (forfeiture_holder_30, _) = pda(&[b"forfeiture", holder_30.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_30) { accounts.push((forfeiture_holder_30, empty.clone())); }
    let ix19a = Instruction::new_with_bytes(
        program_id,
        &data19a,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_14, false),
            AccountMeta::new(holder_record_30_14, false),
            AccountMeta::new(forfeiture_holder_30, false),
            AccountMeta::new(holder_30, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll:19boundary", &ix19a, &accounts);
    accounts = next;
    measured.push(serde_json::json!({
        "instruction": "enroll:19boundary",
        "cu": cu,
        "result": "Success",
        "note": "slice-19 boundary: holder 30 enrolls 50,000,000 in window 14, landing the window's cumulative enrolled total exactly on its 60,000,000 funded snapshot (10,000,000 prior + 50,000,000) — the inclusive boundary of the deposit↔enroll binding",
    }));
    let hr19a = accounts.iter().find(|(k, _)| *k == holder_record_30_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr19a.data[56..64].try_into().unwrap()), 50_000_000u64, "holder 30 position not 50,000,000 after enroll:19boundary");
    let ws19a = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws19a.data[16..24].try_into().unwrap()), 60_000_000u64, "window 14 snapshot moved during enroll:19boundary");
    assert_eq!(u64::from_le_bytes(ws19a.data[138..146].try_into().unwrap()), 60_000_000u64, "window 14 enrolled_total_raw not 60,000,000 after enroll:19boundary");
    // One more unit refuses: the window is exactly full.
    let holder_31 = Pubkey::new_from_array([31u8; 32]);
    let (holder_record_31_14, _) = pda(&[b"holder", &14u64.to_le_bytes(), holder_31.as_ref()], &program_id);
    upsert(&mut accounts, (holder_31, funded()));
    upsert(&mut accounts, (holder_record_31_14, empty.clone()));
    let mut data19b = disc("enroll");
    data19b.extend_from_slice(&14u64.to_le_bytes());
    data19b.extend_from_slice(&1u64.to_le_bytes());
    let (forfeiture_holder_31, _) = pda(&[b"forfeiture", holder_31.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_31) { accounts.push((forfeiture_holder_31, empty.clone())); }
    let ix19b = Instruction::new_with_bytes(
        program_id,
        &data19b,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_14, false),
            AccountMeta::new(holder_record_31_14, false),
            AccountMeta::new(forfeiture_holder_31, false),
            AccountMeta::new(holder_31, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let cu19b = run_expect_err(&mollusk, "enroll:19over", &ix19b, &accounts, 6096);
    measured.push(serde_json::json!({
        "instruction": "enroll:19over",
        "cu": cu19b,
        "result": "Refused",
        "error": "EnrollExceedsFundedSnapshot",
        "error_code": 6096,
        "note": "slice-19 refusal: enrolling 1 more in window 14 (already enrolled exactly to its 60,000,000 funded snapshot) refuses EnrollExceedsFundedSnapshot (6096) — enrolled claims may not exceed the deposits the window opened on, NOT a success CU",
    }));
    let ws19b = accounts.iter().find(|(k, _)| *k == window_state_14).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws19b.data[138..146].try_into().unwrap()), 60_000_000u64, "window 14 enrolled_total_raw moved during enroll:19over refusal");

    // Slice-20 end-of-chain probes: the registry gates, in order.
    // A duplicate registration refuses; unregistering an absent
    // mint refuses; removing the live mint closes the door (the
    // next deposit of that mint refuses 6100 with totals
    // unchanged); re-registering restores the chain state for
    // future slices.
    let mut data20a = disc("register_mint");
    data20a.extend_from_slice(mint.as_ref());
    let ix20a = Instruction::new_with_bytes(
        program_id,
        &data20a,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(treasury_state, false),
            AccountMeta::new(authority, true),
        ],
    );
    let cu20a = run_expect_err(&mollusk, "register_mint:20dup", &ix20a, &accounts, 6097);
    measured.push(serde_json::json!({
        "instruction": "register_mint:20dup",
        "cu": cu20a,
        "result": "Refused",
        "error": "MintAlreadyRegistered",
        "error_code": 6097,
        "note": "slice-20 refusal: registering the already-registered deposit mint refuses MintAlreadyRegistered (6097) — registration never double-counts a slot, NOT a success CU",
    }));
    let junk_mint = Pubkey::new_from_array([12u8; 32]);
    let mut data20b = disc("unregister_mint");
    data20b.extend_from_slice(junk_mint.as_ref());
    let ix20b = Instruction::new_with_bytes(
        program_id,
        &data20b,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(treasury_state, false),
            AccountMeta::new(authority, true),
        ],
    );
    let cu20b = run_expect_err(&mollusk, "unregister_mint:20absent", &ix20b, &accounts, 6099);
    measured.push(serde_json::json!({
        "instruction": "unregister_mint:20absent",
        "cu": cu20b,
        "result": "Refused",
        "error": "MintNotRegistered",
        "error_code": 6099,
        "note": "slice-20 refusal: unregistering a mint that was never registered refuses MintNotRegistered (6099) — a no-op must not masquerade as a revocation, NOT a success CU",
    }));
    let mut data20c = disc("unregister_mint");
    data20c.extend_from_slice(mint.as_ref());
    let ix20c = Instruction::new_with_bytes(
        program_id,
        &data20c,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(treasury_state, false),
            AccountMeta::new(authority, true),
        ],
    );
    let (next, cu) = run(&mollusk, "unregister_mint:20", &ix20c, &accounts);
    accounts = next;
    measured.push(serde_json::json!({
        "instruction": "unregister_mint:20",
        "cu": cu,
        "result": "Success",
        "note": "slice-20: genesis authority removes the deposit mint from the registry — the count returns to 0 (fail-closed empty) and the next deposit of that mint must refuse",
    }));
    let ts20c = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(ts20c.data[65], 0u8, "registry count not 0 after unregister_mint:20");
    let mut data20d = disc("deposit");
    data20d.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix20d = Instruction::new_with_bytes(
        program_id,
        &data20d,
        vec![
            AccountMeta::new(treasury_state, false),
            AccountMeta::new(depositor, true),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(source_key, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(token_program_id, false),
            AccountMeta::new_readonly(rent_id, false),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let cu20d = run_expect_err(&mollusk, "deposit:20unregistered", &ix20d, &accounts, 6100);
    measured.push(serde_json::json!({
        "instruction": "deposit:20unregistered",
        "cu": cu20d,
        "result": "Refused",
        "error": "MintNotAccepted",
        "error_code": 6100,
        "note": "slice-20 refusal: depositing the just-unregistered mint (vault, source, and program bindings all valid) refuses MintNotAccepted (6100) — the registry gate, checked last, closes the junk-mint hole, NOT a success CU",
    }));
    let ts20d = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts20d.data[8..16].try_into().unwrap()), 70_000_000u64, "TreasuryState total moved during deposit:20unregistered refusal");
    let mut data20e = disc("register_mint");
    data20e.extend_from_slice(mint.as_ref());
    let ix20e = Instruction::new_with_bytes(
        program_id,
        &data20e,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(treasury_state, false),
            AccountMeta::new(authority, true),
        ],
    );
    let (next, cu) = run(&mollusk, "register_mint:20restore", &ix20e, &accounts);
    accounts = next;
    measured.push(serde_json::json!({
        "instruction": "register_mint:20restore",
        "cu": cu,
        "result": "Success",
        "note": "slice-20 restore: the deposit mint re-registers, returning the chain to its pre-probe registry state (count 1) so future slices inherit a funded, registered mint",
    }));
    let ts20e = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(ts20e.data[65], 1u8, "registry count not 1 after register_mint:20restore");
    assert_eq!(&ts20e.data[66..98], mint.as_ref(), "registry slot 0 is not the deposit mint after register_mint:20restore");

    // Slice-21 end-of-chain probes: native-SOL deposits. The
    // chain enters at T = 70,000,000 (all token deposits); a
    // first SOL deposit of 5,000,000 lamports credits exactly
    // that amount (T = 75,000,000, vault lamports 5,000,000),
    // a second deposit into the existing vault accumulates
    // (T = 76,000,000, vault 6,000,000), and a zero deposit
    // refuses without moving anything. The vault starts as
    // the bare system account the transfer creates.
    let (sol_vault, _) = pda(&[b"sol-vault", treasury_state.as_ref()], &program_id);
    upsert(&mut accounts, (sol_vault, empty.clone()));
    let mut data21a = disc("deposit_sol");
    data21a.extend_from_slice(&5_000_000u64.to_le_bytes());
    let ix21a = Instruction::new_with_bytes(
        program_id,
        &data21a,
        vec![
            AccountMeta::new(treasury_state, false),
            AccountMeta::new(depositor, true),
            AccountMeta::new(sol_vault, false),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "deposit_sol:21", &ix21a, &accounts);
    accounts = next;
    measured.push(serde_json::json!({
        "instruction": "deposit_sol:21",
        "cu": cu,
        "result": "Success",
        "note": "slice-21 FIRST success: a real system transfer of 5,000,000 lamports from the depositor into the sol-vault PDA credits TreasuryState by exactly 5,000,000 (total and cash 70,000,000 -> 75,000,000) — SOL funds the treasury with no mint, no token CPI, no registry gate",
    }));
    let ts21a = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts21a.data[8..16].try_into().unwrap()), 75_000_000u64, "TreasuryState total not 75,000,000 after deposit_sol:21");
    assert_eq!(u64::from_le_bytes(ts21a.data[16..24].try_into().unwrap()), 75_000_000u64, "TreasuryState cash not 75,000,000 after deposit_sol:21");
    let sv21a = accounts.iter().find(|(k, _)| *k == sol_vault).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(sv21a.lamports, 5_000_000u64, "sol-vault lamports not 5,000,000 after deposit_sol:21");
    assert_eq!(sv21a.owner, system_id, "sol-vault owner is not the system program after deposit_sol:21");
    assert!(sv21a.data.is_empty(), "sol-vault has data after deposit_sol:21");
    let mut data21b = disc("deposit_sol");
    data21b.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix21b = Instruction::new_with_bytes(
        program_id,
        &data21b,
        vec![
            AccountMeta::new(treasury_state, false),
            AccountMeta::new(depositor, true),
            AccountMeta::new(sol_vault, false),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "deposit_sol:21again", &ix21b, &accounts);
    accounts = next;
    measured.push(serde_json::json!({
        "instruction": "deposit_sol:21again",
        "cu": cu,
        "result": "Success",
        "note": "slice-21: a second SOL deposit into the existing vault accumulates (total and cash 75,000,000 -> 76,000,000, vault lamports 5,000,000 -> 6,000,000) — the vault is reused, not recreated",
    }));
    let ts21b = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts21b.data[8..16].try_into().unwrap()), 76_000_000u64, "TreasuryState total not 76,000,000 after deposit_sol:21again");
    let sv21b = accounts.iter().find(|(k, _)| *k == sol_vault).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(sv21b.lamports, 6_000_000u64, "sol-vault lamports not 6,000,000 after deposit_sol:21again");
    let mut data21c = disc("deposit_sol");
    data21c.extend_from_slice(&0u64.to_le_bytes());
    let ix21c = Instruction::new_with_bytes(
        program_id,
        &data21c,
        vec![
            AccountMeta::new(treasury_state, false),
            AccountMeta::new(depositor, true),
            AccountMeta::new(sol_vault, false),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let cu21c = run_expect_err(&mollusk, "deposit_sol:21zero", &ix21c, &accounts, 6081);
    measured.push(serde_json::json!({
        "instruction": "deposit_sol:21zero",
        "cu": cu21c,
        "result": "Refused",
        "error": "InvalidDepositAmount",
        "error_code": 6081,
        "note": "slice-21 refusal: a zero SOL deposit refuses InvalidDepositAmount (6081) — a zero deposit funds nothing, NOT a success CU",
    }));
    let ts21c = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts21c.data[8..16].try_into().unwrap()), 76_000_000u64, "TreasuryState total moved during deposit_sol:21zero refusal");


    // ---- Slice 22: cross-record D-3 carry (the carried item from slice 16) ----
    // A default's forfeiture must survive the loan window's record: the
    // holder's NEXT enrolled window is a fresh HolderRecord, and the program
    // cannot enumerate a holder's prior records on-chain, so the forfeiture
    // lives on a holder-scoped ForfeitureRecord PDA ["forfeiture", holder]:
    // settle_default queues one pending slot per default, enroll consumes
    // exactly one and sets the new record's skip flag iff one was pending.
    // On this chain, two independent defaulters carry into a new window 15
    // (opened on Settled transition 22, snapshot 76,000,000 after the SOL
    // deposits, stock 12,666,666): twin D (defaulted window 13) and holder
    // [8] (defaulted window 1) both enroll with skip set and their pending
    // counts consumed to zero, while a fresh holder enrolls clear. The
    // carried slot is then proven at pay_tranche: D's window-15 slot
    // refuses DividendSkipped (6091), so the forfeiture is re-expressed
    // end to end, not just recorded on the loan window's record.
    // (a) CROSS THE BOUNDARY: Settled treasury.window.v1 transition 22
    // (parent 21, [207;32], declared [217;32], now 37,100,000) opens
    // window 15. Its 10 setup entries carry the helper's notes.
    let (observation_22, _) = pda(&[b"observation", &22u64.to_le_bytes()], &program_id);
    let (transition_22, _) = pda(&[b"transition", &22u64.to_le_bytes()], &program_id);
    let (plan_22, _) = pda(&[b"plan", &22u64.to_le_bytes(), proposer.as_ref()], &program_id);
    let (signal_22, signal_22_bump) = pda(
        &[b"signal", &22u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
        &program_id,
    );
    let (evidence_22, _) = pda(&[b"evidence", &22u64.to_le_bytes()], &program_id);
    upsert(&mut accounts, (observation_22, empty.clone()));
    upsert(&mut accounts, (transition_22, empty.clone()));
    upsert(&mut accounts, (plan_22, empty.clone()));
    upsert(&mut accounts, (signal_22, empty.clone()));
    upsert(&mut accounts, (evidence_22, empty.clone()));
    upsert(&mut accounts, (window_state_15, empty.clone()));
    accounts = lifecycle_to_commit(
        &mut mollusk, accounts, &mut measured,
        program_id, clock_id, system_id,
        oracle_record, oracle, proposer_record, proposer, evaluator_record, evaluator,
        chain, stock, policy_sha,
        observation_22, transition_22, plan_22, signal_22, signal_22_bump,
        22, 38_100_000, 39_100_000, [217u8; 32], 21, [207u8; 32],
        [9u8; 32], 37_100_000, ":22", Some(evidence_22),
    );
    let mut data22a = disc("open_window");
    data22a.extend_from_slice(&22u64.to_le_bytes());
    data22a.extend_from_slice(&15u64.to_le_bytes());
    let ix22a = Instruction::new_with_bytes(
        program_id,
        &data22a,
        vec![
            AccountMeta::new(treasury_state, false),
            AccountMeta::new_readonly(lending_book, false),
            AccountMeta::new(transition_22, false),
            AccountMeta::new(window_state_15, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "open_window:15", &ix22a, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "open_window:15", "cu": cu, "result": "Success", "note": "slice-22 setup: window 15 opens on Settled transition 22 (funded snapshot total 76,000,000 after the SOL deposits, exit stock 12,666,666) — the boundary crossing; window_count 15 -> 16, current window 14 -> 15"}));
    let ts22a = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts22a.data[56..64].try_into().unwrap()), 16u64, "window_count not 16 after open_window:15");
    let ws15_22a = accounts.iter().find(|(k, _)| *k == window_state_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws15_22a.data[16..24].try_into().unwrap()), 76_000_000u64, "window 15 snapshot total not 76,000,000");
    assert_eq!(u64::from_le_bytes(ws15_22a.data[64..72].try_into().unwrap()), 12_666_666u64, "window 15 exit stock not 12,666,666");
    assert_eq!(u64::from_le_bytes(ws15_22a.data[72..80].try_into().unwrap()), 22u64, "window 15 source transition not 22");
    // (b) Twin D enrolls in window 15: the slice-11 default queued one
    // pending slot (source window 13), so the fresh record starts with
    // skip set and the pending count is consumed to zero.
    let (holder_record_d_15, _) = pda(&[b"holder", &15u64.to_le_bytes(), holder_d.as_ref()], &program_id);
    upsert(&mut accounts, (holder_record_d_15, empty.clone()));
    let (forfeiture_holder_d_22, _) = pda(&[b"forfeiture", holder_d.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_d_22) { accounts.push((forfeiture_holder_d_22, empty.clone())); }
    let mut data22b = disc("enroll");
    data22b.extend_from_slice(&15u64.to_le_bytes());
    data22b.extend_from_slice(&5_000_000u64.to_le_bytes());
    let ix22b = Instruction::new_with_bytes(
        program_id,
        &data22b,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_15, false),
            AccountMeta::new(holder_record_d_15, false),
            AccountMeta::new(forfeiture_holder_d_22, false),
            AccountMeta::new(holder_d, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll:22D", &ix22b, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "enroll:22D", "cu": cu, "result": "Success", "note": "slice-22 FIRST carry: slice-11 defaulter twin D enrolls in window 15 (cohort 3, position 5,000,000) — the fresh record inherits skip_next_dividend from the queued forfeiture (source window 13) and the pending count is consumed to zero, NOT a clean enrollment"}));
    let hr22b = accounts.iter().find(|(k, _)| *k == holder_record_d_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr22b.data[48..56].try_into().unwrap()), 3u64, "twin D cohort not 3 (15 % 12)");
    assert_eq!(u64::from_le_bytes(hr22b.data[56..64].try_into().unwrap()), 5_000_000u64, "twin D position not 5,000,000 after enroll:22D");
    assert_eq!(hr22b.data[88], 1u8, "twin D skip flag not set by the carry at enroll:22D");
    let fr22b = accounts.iter().find(|(k, _)| *k == forfeiture_holder_d_22).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fr22b.data[40..48].try_into().unwrap()), 0u64, "twin D pending slots not consumed to 0 by enroll:22D");
    assert_eq!(u64::from_le_bytes(fr22b.data[48..56].try_into().unwrap()), 13u64, "twin D forfeiture source window not 13 after enroll:22D");
    // (c) The carried slot is proven at pay_tranche: D's window-15 slot
    // refuses DividendSkipped (6091) on Committed window-flow transition 7.
    let mut data22c = disc("pay_tranche");
    data22c.extend_from_slice(&7u64.to_le_bytes());
    data22c.extend_from_slice(&15u64.to_le_bytes());
    data22c.extend_from_slice(holder_d.as_ref());
    data22c.extend_from_slice(&1_000_000u64.to_le_bytes());
    let ix22c = Instruction::new_with_bytes(
        program_id,
        &data22c,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_15, false),
            AccountMeta::new(holder_record_d_15, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "pay_tranche:22Dskip", &ix22c, &accounts, 6091);
    measured.push(serde_json::json!({
        "instruction": "pay_tranche:22Dskip",
        "cu": cu,
        "result": "Refused",
        "error": "DividendSkipped",
        "error_code": 6091,
        "note": "slice-22 PROOF (D-3 end to end): operator pay_tranche 1,000,000 on twin D's fresh window-15 record REFUSES DividendSkipped (6091) — the forfeiture carried across records at enroll time, so the next eligible slot pays nothing; accrued stays 0, flag stays set, NOT a success CU",
    }));
    let hr22c = accounts.iter().find(|(k, _)| *k == holder_record_d_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr22c.data[64..72].try_into().unwrap()), 0u64, "twin D accrued moved during pay_tranche:22Dskip refusal");
    assert_eq!(hr22c.data[88], 1u8, "twin D skip flag moved during pay_tranche:22Dskip refusal");
    // (d) Second independent defaulter: holder [8] (slice-7 default, source
    // window 1) enrolls in window 15 with the same carry, consumed to zero.
    let (holder_record_8_15, _) = pda(&[b"holder", &15u64.to_le_bytes(), holder.as_ref()], &program_id);
    upsert(&mut accounts, (holder_record_8_15, empty.clone()));
    let (forfeiture_holder_22, _) = pda(&[b"forfeiture", holder.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_22) { accounts.push((forfeiture_holder_22, empty.clone())); }
    let mut data22d = disc("enroll");
    data22d.extend_from_slice(&15u64.to_le_bytes());
    data22d.extend_from_slice(&5_000_000u64.to_le_bytes());
    let ix22d = Instruction::new_with_bytes(
        program_id,
        &data22d,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_15, false),
            AccountMeta::new(holder_record_8_15, false),
            AccountMeta::new(forfeiture_holder_22, false),
            AccountMeta::new(holder, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll:22H", &ix22d, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "enroll:22H", "cu": cu, "result": "Success", "note": "slice-22 second carry: slice-7 defaulter holder [8] enrolls in window 15 (cohort 3, position 5,000,000) — the fresh record inherits skip from the queued forfeiture (source window 1) and the pending count is consumed to zero; the carry is per-holder, not twin-D-specific"}));
    let hr22d = accounts.iter().find(|(k, _)| *k == holder_record_8_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(hr22d.data[88], 1u8, "holder [8] skip flag not set by the carry at enroll:22H");
    let fr22d = accounts.iter().find(|(k, _)| *k == forfeiture_holder_22).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fr22d.data[40..48].try_into().unwrap()), 0u64, "holder [8] pending slots not consumed to 0 by enroll:22H");
    assert_eq!(u64::from_le_bytes(fr22d.data[48..56].try_into().unwrap()), 1u64, "holder [8] forfeiture source window not 1 after enroll:22H");
    // (e) Control: a fresh holder with no default enrolls in window 15
    // clear — the carry creates the forfeiture account empty, it does
    // not manufacture a forfeiture for the innocent.
    let holder_32 = Pubkey::new_from_array([32u8; 32]);
    let (holder_record_32_15, _) = pda(&[b"holder", &15u64.to_le_bytes(), holder_32.as_ref()], &program_id);
    upsert(&mut accounts, (holder_32, funded()));
    upsert(&mut accounts, (holder_record_32_15, empty.clone()));
    let (forfeiture_holder_32, _) = pda(&[b"forfeiture", holder_32.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_32) { accounts.push((forfeiture_holder_32, empty.clone())); }
    let mut data22e = disc("enroll");
    data22e.extend_from_slice(&15u64.to_le_bytes());
    data22e.extend_from_slice(&5_000_000u64.to_le_bytes());
    let ix22e = Instruction::new_with_bytes(
        program_id,
        &data22e,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_15, false),
            AccountMeta::new(holder_record_32_15, false),
            AccountMeta::new(forfeiture_holder_32, false),
            AccountMeta::new(holder_32, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll:22control", &ix22e, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "enroll:22control", "cu": cu, "result": "Success", "note": "slice-22 control: fresh holder 32 enrolls in window 15 (cohort 3, position 5,000,000) with skip clear and zero pending slots — the carry punishes only queued defaulters, NOT a success CU for a forfeiture"}));
    let hr22e = accounts.iter().find(|(k, _)| *k == holder_record_32_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(hr22e.data[88], 0u8, "fresh holder 32 skip flag set without a forfeiture at enroll:22control");
    let fr22e = accounts.iter().find(|(k, _)| *k == forfeiture_holder_32).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fr22e.data[40..48].try_into().unwrap()), 0u64, "fresh holder 32 pending slots nonzero at enroll:22control");
    let ws15_22e = accounts.iter().find(|(k, _)| *k == window_state_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws15_22e.data[138..146].try_into().unwrap()), 15_000_000u64, "window 15 enrolled_total_raw not 15,000,000 after slice 22");
    let hr_d_22 = accounts.iter().find(|(k, _)| *k == holder_record_d).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr_d_22.data[56..64].try_into().unwrap()), 4_000_000u64, "twin D window-13 position moved during slice 22");
    assert_eq!(hr_d_22.data[88], 1u8, "twin D window-13 skip flag moved during slice 22");

    // ---- Slice 23: borrowed-amount reinvestment (the 2026-10-26
    // ~23:58 CST ruling, decided on Carlos's instruction) ----
    // `borrow_reinvest` is its own instruction, atomic with the
    // borrow: the loan is recorded in the lending book AND the
    // locked position grows by exactly the borrowed amount in
    // the same instruction. No cash is disbursed and TreasuryState
    // totals are NOT credited — the offsetting asset is the loan
    // receivable, as in a normal borrow. Capacity is judged on
    // the OWNED position (position minus unrepaid loan-funded
    // increments, tracked on the record), so the leverage spiral
    // is impossible by construction. On this chain, in the still
    // open window 15 (snapshot 76,000,000, stock 12,666,666,
    // enrolled 15,000,000, book outstanding 0, Committed window
    // flow transition 7 authorizing executions): a fresh holder
    // enrolls at 10,000,000, borrow_reinvests the 50% maximum
    // 5,000,000 (position 15,000,000, funded 5,000,000, book
    // 5,000,000), a second concurrent loan refuses
    // LoanAlreadyActive, an exit that would drop the position
    // below the loan refuses ExitBelowLoan (the plain-borrow
    // exit path, unchanged), an over-owned-cap request on a
    // second fresh holder refuses LoanExceedsPosition even
    // though the book is also full (gate order: owned cap
    // before book cap), and a default settlement at maturity
    // returns the position to exactly its pre-loan size
    // (15,000,000 -> 10,000,000) with the funded figure
    // extinguished to zero. The clock is set explicitly to
    // 37,100,000 for this slice, so the loan maturity is
    // 37,100,000 + 60d = 42,284,000.
    mollusk.sysvars.clock.unix_timestamp = 37_100_000;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    // (a) Fresh holder 33 enrolls in window 15 at 10,000,000.
    let holder_33 = Pubkey::new_from_array([33u8; 32]);
    let (holder_record_33_15, _) = pda(&[b"holder", &15u64.to_le_bytes(), holder_33.as_ref()], &program_id);
    upsert(&mut accounts, (holder_33, funded()));
    upsert(&mut accounts, (holder_record_33_15, empty.clone()));
    let (forfeiture_holder_33, _) = pda(&[b"forfeiture", holder_33.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_33) { accounts.push((forfeiture_holder_33, empty.clone())); }
    let mut data23a = disc("enroll");
    data23a.extend_from_slice(&15u64.to_le_bytes());
    data23a.extend_from_slice(&10_000_000u64.to_le_bytes());
    let ix23a = Instruction::new_with_bytes(
        program_id,
        &data23a,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_15, false),
            AccountMeta::new(holder_record_33_15, false),
            AccountMeta::new(forfeiture_holder_33, false),
            AccountMeta::new(holder_33, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll:23", &ix23a, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "enroll:23", "cu": cu, "result": "Success", "note": "slice-23 setup: fresh holder 33 enrolls in window 15 (cohort 3, position 10,000,000, loan-funded 0) — the borrow-reinvest subject; window 15 enrolled total 15,000,000 -> 25,000,000"}));
    let hr23a = accounts.iter().find(|(k, _)| *k == holder_record_33_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr23a.data[48..56].try_into().unwrap()), 3u64, "holder 33 cohort not 3 (15 % 12)");
    assert_eq!(u64::from_le_bytes(hr23a.data[56..64].try_into().unwrap()), 10_000_000u64, "holder 33 position not 10,000,000 after enroll:23");
    assert_eq!(u64::from_le_bytes(hr23a.data[100..108].try_into().unwrap()), 0u64, "holder 33 loan-funded not 0 at enroll:23");
    assert_eq!(hr23a.data[88], 0u8, "holder 33 skip flag set without a forfeiture at enroll:23");
    let ws15_23a = accounts.iter().find(|(k, _)| *k == window_state_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws15_23a.data[138..146].try_into().unwrap()), 25_000_000u64, "window 15 enrolled_total_raw not 25,000,000 after enroll:23");
    // (b) borrow_reinvest SUCCESS at the 50%-of-owned maximum.
    let mut data23b = disc("borrow_reinvest");
    data23b.extend_from_slice(&7u64.to_le_bytes());
    data23b.extend_from_slice(&15u64.to_le_bytes());
    data23b.extend_from_slice(&5_000_000u64.to_le_bytes());
    let ix23b = Instruction::new_with_bytes(
        program_id,
        &data23b,
        vec![
            AccountMeta::new(holder_33, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_15, false),
            AccountMeta::new(holder_record_33_15, false),
            AccountMeta::new(lending_book, false),
        ],
    );
    let (next, cu) = run(&mollusk, "borrow_reinvest", &ix23b, &accounts);
    accounts = next;
    let gate23b = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "borrow_reinvest",
        "cu": cu,
        "result": "Success",
        "note": "slice-23 FIRST success (borrowed-amount reinvestment, the 2026-10-26 ruling): holder 33 borrow-reinvests 5,000,000 on Committed window flow transition 7, window 15 — at the inclusive boundary of BOTH caps (50% of the 10,000,000 owned position AND book cap max_outstanding(76,000,000) = 6,333,333, outstanding 0 + 5,000,000); position 10,000,000 -> 15,000,000, loan-funded 0 -> 5,000,000, book outstanding 0 -> 5,000,000, TreasuryState totals UNCHANGED (no deposit fiction — the offsetting asset is the loan receivable)",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate23b,
    }));
    let hr23b = accounts.iter().find(|(k, _)| *k == holder_record_33_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr23b.data[56..64].try_into().unwrap()), 15_000_000u64, "holder 33 position not 15,000,000 after borrow_reinvest");
    assert_eq!(u64::from_le_bytes(hr23b.data[72..80].try_into().unwrap()), 5_000_000u64, "holder 33 loan not 5,000,000 after borrow_reinvest");
    assert_eq!(i64::from_le_bytes(hr23b.data[80..88].try_into().unwrap()), 42_284_000i64, "holder 33 maturity not 42,284,000 (37,100,000 + 60d) after borrow_reinvest");
    assert_eq!(u64::from_le_bytes(hr23b.data[100..108].try_into().unwrap()), 5_000_000u64, "holder 33 loan-funded not 5,000,000 after borrow_reinvest");
    let lb23b = accounts.iter().find(|(k, _)| *k == lending_book).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(lb23b.data[8..16].try_into().unwrap()), 5_000_000u64, "lending book outstanding not 5,000,000 after borrow_reinvest");
    assert_eq!(u32::from_le_bytes(lb23b.data[16..20].try_into().unwrap()), 1u32, "lending book active loans not 1 after borrow_reinvest");
    let ts23b = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ts23b.data[8..16].try_into().unwrap()), 76_000_000u64, "TreasuryState total moved during borrow_reinvest");
    assert_eq!(u64::from_le_bytes(ts23b.data[16..24].try_into().unwrap()), 76_000_000u64, "TreasuryState cash moved during borrow_reinvest");
    // (c) A second concurrent loan refuses LoanAlreadyActive (6050).
    let mut data23c = disc("borrow_reinvest");
    data23c.extend_from_slice(&7u64.to_le_bytes());
    data23c.extend_from_slice(&15u64.to_le_bytes());
    data23c.extend_from_slice(&1u64.to_le_bytes());
    let ix23c = Instruction::new_with_bytes(
        program_id,
        &data23c,
        vec![
            AccountMeta::new(holder_33, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_15, false),
            AccountMeta::new(holder_record_33_15, false),
            AccountMeta::new(lending_book, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "borrow_reinvest:second_loan", &ix23c, &accounts, 6050);
    measured.push(serde_json::json!({
        "instruction": "borrow_reinvest:second_loan",
        "cu": cu,
        "result": "Refused",
        "error": "LoanAlreadyActive",
        "error_code": 6050,
        "note": "slice-23: a second concurrent borrow_reinvest (1 raw) on holder 33 refuses LoanAlreadyActive — one loan per holder per window, so a second loan cannot tangle settle_default's single principal/maturity settlement; the funded increment cannot be re-borrowed against while its loan is active, NOT a success CU",
    }));
    let hr23c = accounts.iter().find(|(k, _)| *k == holder_record_33_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr23c.data[72..80].try_into().unwrap()), 5_000_000u64, "loan moved during borrow_reinvest:second_loan refusal");
    assert_eq!(u64::from_le_bytes(hr23c.data[100..108].try_into().unwrap()), 5_000_000u64, "loan-funded moved during borrow_reinvest:second_loan refusal");
    // (d) An exit that would drop the position below the loan refuses
    // ExitBelowLoan (6062) — the plain-borrow exit path, unchanged.
    let mut data23d = disc("exit");
    data23d.extend_from_slice(&7u64.to_le_bytes());
    data23d.extend_from_slice(&15u64.to_le_bytes());
    data23d.extend_from_slice(&11_000_000u64.to_le_bytes());
    let ix23d = Instruction::new_with_bytes(
        program_id,
        &data23d,
        vec![
            AccountMeta::new(holder_33, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new(window_state_15, false),
            AccountMeta::new(holder_record_33_15, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "exit:23below_loan", &ix23d, &accounts, 6062);
    measured.push(serde_json::json!({
        "instruction": "exit:23below_loan",
        "cu": cu,
        "result": "Refused",
        "error": "ExitBelowLoan",
        "error_code": 6062,
        "note": "slice-23: holder 33's 11,000,000 exit refuses ExitBelowLoan — it passes the D-10 per-order bound (11,000,000 <= 12,666,666) but would leave position 4,000,000 below the 5,000,000 outstanding loan, so no legitimate settlement is ever met with DefaultExceedsPosition; exit-while-indebted behaves exactly as the plain-borrow path, NOT a success CU",
    }));
    let hr23d = accounts.iter().find(|(k, _)| *k == holder_record_33_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr23d.data[56..64].try_into().unwrap()), 15_000_000u64, "position moved during exit:23below_loan refusal");
    // (e) Over-owned-cap refusal on a second fresh holder: holder 34
    // enrolls at 10,000,000, then a 5,000,001 borrow_reinvest
    // refuses LoanExceedsPosition (6052) — the owned cap is
    // judged before the book cap, so the refusal code is the
    // position code even though the book (5,000,000 +
    // 5,000,001 > 6,333,333) is also full.
    let holder_34 = Pubkey::new_from_array([34u8; 32]);
    let (holder_record_34_15, _) = pda(&[b"holder", &15u64.to_le_bytes(), holder_34.as_ref()], &program_id);
    upsert(&mut accounts, (holder_34, funded()));
    upsert(&mut accounts, (holder_record_34_15, empty.clone()));
    let (forfeiture_holder_34, _) = pda(&[b"forfeiture", holder_34.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == forfeiture_holder_34) { accounts.push((forfeiture_holder_34, empty.clone())); }
    let mut data23e = disc("enroll");
    data23e.extend_from_slice(&15u64.to_le_bytes());
    data23e.extend_from_slice(&10_000_000u64.to_le_bytes());
    let ix23e = Instruction::new_with_bytes(
        program_id,
        &data23e,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_15, false),
            AccountMeta::new(holder_record_34_15, false),
            AccountMeta::new(forfeiture_holder_34, false),
            AccountMeta::new(holder_34, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll:23b", &ix23e, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "enroll:23b", "cu": cu, "result": "Success", "note": "slice-23 setup: fresh holder 34 enrolls in window 15 (cohort 3, position 10,000,000) — the over-cap subject; window 15 enrolled total 25,000,000 -> 35,000,000"}));
    let mut data23e2 = disc("borrow_reinvest");
    data23e2.extend_from_slice(&7u64.to_le_bytes());
    data23e2.extend_from_slice(&15u64.to_le_bytes());
    data23e2.extend_from_slice(&5_000_001u64.to_le_bytes());
    let ix23e2 = Instruction::new_with_bytes(
        program_id,
        &data23e2,
        vec![
            AccountMeta::new(holder_34, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_15, false),
            AccountMeta::new(holder_record_34_15, false),
            AccountMeta::new(lending_book, false),
        ],
    );
    let cu = run_expect_err(&mollusk, "borrow_reinvest:over_owned_cap", &ix23e2, &accounts, 6052);
    measured.push(serde_json::json!({
        "instruction": "borrow_reinvest:over_owned_cap",
        "cu": cu,
        "result": "Refused",
        "error": "LoanExceedsPosition",
        "error_code": 6052,
        "note": "slice-23: holder 34's 5,000,001 borrow_reinvest refuses LoanExceedsPosition — 1 over the 50%-of-owned cap (5,000,000 on a 10,000,000 owned position); the refusal code is the owned-cap code even though the book is also full (5,000,000 outstanding + 5,000,001 > 6,333,333 cap), proving gate order, NOT a success CU",
    }));
    let hr23e = accounts.iter().find(|(k, _)| *k == holder_record_34_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr23e.data[72..80].try_into().unwrap()), 0u64, "loan moved during borrow_reinvest:over_owned_cap refusal");
    assert_eq!(u64::from_le_bytes(hr23e.data[100..108].try_into().unwrap()), 0u64, "loan-funded moved during borrow_reinvest:over_owned_cap refusal");
    // (f) Default settlement at maturity returns the position to
    // exactly its pre-loan size and extinguishes the funded
    // increment: 15,000,000 -> 10,000,000, loan 0, funded 0.
    mollusk.sysvars.clock.unix_timestamp = 42_284_000;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let mut data23f = disc("settle_default");
    data23f.extend_from_slice(&7u64.to_le_bytes());
    data23f.extend_from_slice(&15u64.to_le_bytes());
    data23f.extend_from_slice(holder_33.as_ref());
    let ix23f = Instruction::new_with_bytes(
        program_id,
        &data23f,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_15, false),
            AccountMeta::new(holder_record_33_15, false),
            AccountMeta::new(lending_book, false),
            AccountMeta::new(forfeiture_holder_33, false),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "settle_default:23", &ix23f, &accounts);
    accounts = next;
    let gate23f = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "settle_default:23",
        "cu": cu,
        "result": "Success",
        "note": "slice-23 default proof (D-3): operator-settled default at maturity 42,284,000 on Committed window flow transition 7, window 15 — principal 5,000,000 settles against position, interest forgiven, skip-next-dividend set on the pre-default base 15,000,000, one forfeiture slot queued (source window 15); position returns to exactly its pre-loan size 10,000,000 and loan-funded extinguishes 5,000,000 -> 0, so a defaulted reinvested position keeps nothing it did not own",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate23f,
    }));
    let hr23f = accounts.iter().find(|(k, _)| *k == holder_record_33_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr23f.data[56..64].try_into().unwrap()), 10_000_000u64, "holder 33 position not 10,000,000 after settle_default:23");
    assert_eq!(u64::from_le_bytes(hr23f.data[72..80].try_into().unwrap()), 0u64, "holder 33 loan not cleared by settle_default:23");
    assert_eq!(i64::from_le_bytes(hr23f.data[80..88].try_into().unwrap()), 0i64, "holder 33 maturity not cleared by settle_default:23");
    assert_eq!(u64::from_le_bytes(hr23f.data[100..108].try_into().unwrap()), 0u64, "holder 33 loan-funded not extinguished by settle_default:23");
    assert_eq!(hr23f.data[88], 1u8, "holder 33 skip-next-dividend flag not set by settle_default:23");
    let lb23f = accounts.iter().find(|(k, _)| *k == lending_book).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(lb23f.data[8..16].try_into().unwrap()), 0u64, "lending book outstanding not 0 after settle_default:23");
    assert_eq!(u32::from_le_bytes(lb23f.data[16..20].try_into().unwrap()), 0u32, "lending book active loans not 0 after settle_default:23");
    let fr23f = accounts.iter().find(|(k, _)| *k == forfeiture_holder_33).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fr23f.data[40..48].try_into().unwrap()), 1u64, "holder 33 forfeiture pending slots not 1 after settle_default:23");
    assert_eq!(u64::from_le_bytes(fr23f.data[48..56].try_into().unwrap()), 15u64, "holder 33 forfeiture source window not 15 after settle_default:23");
    let ws15_23f = accounts.iter().find(|(k, _)| *k == window_state_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(ws15_23f.data[138..146].try_into().unwrap()), 35_000_000u64, "window 15 enrolled_total_raw moved during slice 23");

    // ---- Slice 24: repeat default on an existing forfeiture record
    // (the carried measurement from slice 22/23) ----
    // The slice-23 default cleared holder 33's loan, so a plain
    // borrow on the SAME window-15 record is legal again
    // (can_borrow requires existing_loan == 0; it does not
    // check the skip flag, and window 15 is still current).
    // The second default then settles against a ForfeitureRecord
    // that already exists — no init_if_needed creation — so
    // its CU is the repeat-default figure, directly comparable
    // to the first-default 20,323 CU of settle_default:23.
    // (a) Plain borrow 5,000,000 at the still-current clock
    // 42,284,000: maturity 42,284,000 + 60d = 47,468,000.
    // Position stays 10,000,000 (a plain borrow grows no
    // position), loan-funded stays 0, book 0 -> 5,000,000,
    // forfeiture pending stays 1 (borrow never touches it).
    let mut data24a = disc("borrow");
    data24a.extend_from_slice(&7u64.to_le_bytes());
    data24a.extend_from_slice(&15u64.to_le_bytes());
    data24a.extend_from_slice(&5_000_000u64.to_le_bytes());
    let ix24a = Instruction::new_with_bytes(
        program_id,
        &data24a,
        vec![
            AccountMeta::new(holder_33, true),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_15, false),
            AccountMeta::new(holder_record_33_15, false),
            AccountMeta::new(lending_book, false),
        ],
    );
    let (next, cu) = run(&mollusk, "borrow:24", &ix24a, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "borrow:24", "cu": cu, "result": "Success", "note": "slice-24 setup: holder 33 re-borrows 5,000,000 (plain borrow, 50% of the 10,000,000 post-default position, book cap 6,333,333) on the same window-15 record after the slice-23 default cleared its loan — a second loan on a cleared record is legal, and the forfeiture record is untouched by the borrow (pending stays 1)"}));
    let hr24a = accounts.iter().find(|(k, _)| *k == holder_record_33_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr24a.data[56..64].try_into().unwrap()), 10_000_000u64, "holder 33 position moved during borrow:24");
    assert_eq!(u64::from_le_bytes(hr24a.data[72..80].try_into().unwrap()), 5_000_000u64, "holder 33 loan not 5,000,000 after borrow:24");
    assert_eq!(i64::from_le_bytes(hr24a.data[80..88].try_into().unwrap()), 47_468_000i64, "holder 33 maturity not 47,468,000 (42,284,000 + 60d) after borrow:24");
    assert_eq!(u64::from_le_bytes(hr24a.data[100..108].try_into().unwrap()), 0u64, "holder 33 loan-funded moved during borrow:24");
    let lb24a = accounts.iter().find(|(k, _)| *k == lending_book).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(lb24a.data[8..16].try_into().unwrap()), 5_000_000u64, "lending book outstanding not 5,000,000 after borrow:24");
    assert_eq!(u32::from_le_bytes(lb24a.data[16..20].try_into().unwrap()), 1u32, "lending book active loans not 1 after borrow:24");
    let fr24a = accounts.iter().find(|(k, _)| *k == forfeiture_holder_33).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fr24a.data[40..48].try_into().unwrap()), 1u64, "holder 33 forfeiture pending slots moved during borrow:24");
    // (b) Repeat default at maturity: position 10,000,000 ->
    // 5,000,000, loan 0, maturity 0, funded 0, skip flag
    // stays set, book back to 0/0, forfeiture pending
    // 1 -> 2 (a second queued slot; source stays window 15).
    mollusk.sysvars.clock.unix_timestamp = 47_468_000;
    upsert(&mut accounts, mollusk.sysvars.keyed_account_for_clock_sysvar());
    let mut data24b = disc("settle_default");
    data24b.extend_from_slice(&7u64.to_le_bytes());
    data24b.extend_from_slice(&15u64.to_le_bytes());
    data24b.extend_from_slice(holder_33.as_ref());
    let ix24b = Instruction::new_with_bytes(
        program_id,
        &data24b,
        vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(operator_record, false),
            AccountMeta::new_readonly(transition7, false),
            AccountMeta::new_readonly(window_state_15, false),
            AccountMeta::new(holder_record_33_15, false),
            AccountMeta::new(lending_book, false),
            AccountMeta::new(forfeiture_holder_33, false),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "settle_default:24", &ix24b, &accounts);
    accounts = next;
    let gate24b = if (3_500..=14_000).contains(&cu) { "pass" } else { "DESIGN-FINDING" };
    measured.push(serde_json::json!({
        "instruction": "settle_default:24",
        "cu": cu,
        "result": "Success",
        "note": "slice-24 REPEAT-default measurement: operator-settled default at maturity 47,468,000 on Committed window flow transition 7, window 15 — the SAME holder's second default, settling against a ForfeitureRecord that already exists (no init_if_needed creation), so this CU is the repeat-default figure to compare against the first-default 20,323 CU of settle_default:23; principal 5,000,000 settles against position, interest forgiven, skip flag stays set, a second forfeiture slot queues (pending 1 -> 2, source still window 15)",
        "design_estimate_cu": "7000",
        "gate_2x_band_cu": "3500-14000",
        "gate": gate24b,
    }));
    let hr24b = accounts.iter().find(|(k, _)| *k == holder_record_33_15).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(hr24b.data[56..64].try_into().unwrap()), 5_000_000u64, "holder 33 position not 5,000,000 after settle_default:24");
    assert_eq!(u64::from_le_bytes(hr24b.data[72..80].try_into().unwrap()), 0u64, "holder 33 loan not cleared by settle_default:24");
    assert_eq!(i64::from_le_bytes(hr24b.data[80..88].try_into().unwrap()), 0i64, "holder 33 maturity not cleared by settle_default:24");
    assert_eq!(u64::from_le_bytes(hr24b.data[100..108].try_into().unwrap()), 0u64, "holder 33 loan-funded moved during settle_default:24");
    assert_eq!(hr24b.data[88], 1u8, "holder 33 skip-next-dividend flag cleared by settle_default:24");
    let lb24b = accounts.iter().find(|(k, _)| *k == lending_book).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(lb24b.data[8..16].try_into().unwrap()), 0u64, "lending book outstanding not 0 after settle_default:24");
    assert_eq!(u32::from_le_bytes(lb24b.data[16..20].try_into().unwrap()), 0u32, "lending book active loans not 0 after settle_default:24");
    let fr24b = accounts.iter().find(|(k, _)| *k == forfeiture_holder_33).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(u64::from_le_bytes(fr24b.data[40..48].try_into().unwrap()), 2u64, "holder 33 forfeiture pending slots not 2 after settle_default:24");
    assert_eq!(u64::from_le_bytes(fr24b.data[48..56].try_into().unwrap()), 15u64, "holder 33 forfeiture source window not 15 after settle_default:24");

    // Slice-26 end-of-chain probes: the allocation rule registry.
    // Standalone — the registry is not read by any earlier chain
    // instruction, so these probes disturb no earlier state. The
    // registry PDA is created on first use (init_if_needed).
    let (rule_registry, _) = pda(&[b"rule-registry"], &program_id);
    if !accounts.iter().any(|(k, _)| *k == rule_registry) { accounts.push((rule_registry, empty.clone())); }
    let mut data26a = disc("register_rule_version");
    data26a.extend_from_slice(&3u64.to_le_bytes());
    let ix26a = Instruction::new_with_bytes(
        program_id,
        &data26a,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(rule_registry, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_rule_version:26", &ix26a, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "register_rule_version:26", "cu": cu, "result": "Success", "note": "slice-26: genesis authority publishes rule version 3 — the registry PDA is created on first use (init_if_needed), count 0 -> 1, slot 0 = 3"}));
    let rr26a = accounts.iter().find(|(k, _)| *k == rule_registry).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(rr26a.data[8], 1u8, "registry count not 1 after register_rule_version:26");
    assert_eq!(u64::from_le_bytes(rr26a.data[9..17].try_into().unwrap()), 3u64, "registry slot 0 not 3 after register_rule_version:26");
    let mut data26b = disc("register_rule_version");
    data26b.extend_from_slice(&3u64.to_le_bytes());
    let ix26b = Instruction::new_with_bytes(
        program_id,
        &data26b,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(rule_registry, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let cu26b = run_expect_err(&mollusk, "register_rule_version:26dup", &ix26b, &accounts, 6104);
    measured.push(serde_json::json!({"instruction": "register_rule_version:26dup", "cu": cu26b, "result": "Refused", "error": "RuleVersionAlreadyRegistered", "error_code": 6104, "note": "slice-26 refusal: re-registering published version 3 refuses RuleVersionAlreadyRegistered (6104) — registration never double-counts a slot, NOT a success CU"}));
    let mut data26c = disc("register_rule_version");
    data26c.extend_from_slice(&0u64.to_le_bytes());
    let ix26c = Instruction::new_with_bytes(
        program_id,
        &data26c,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(rule_registry, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let cu26c = run_expect_err(&mollusk, "register_rule_version:26zero", &ix26c, &accounts, 6075);
    measured.push(serde_json::json!({"instruction": "register_rule_version:26zero", "cu": cu26c, "result": "Refused", "error": "InvalidRuleVersion", "error_code": 6075, "note": "slice-26 refusal: publishing version 0 refuses InvalidRuleVersion (6075) — 0 is the unpinned sentinel, never a published document, NOT a success CU"}));
    let mut data26d = disc("register_rule_version");
    data26d.extend_from_slice(&1u64.to_le_bytes());
    let ix26d = Instruction::new_with_bytes(
        program_id,
        &data26d,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(rule_registry, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_rule_version:26second", &ix26d, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "register_rule_version:26second", "cu": cu, "result": "Success", "note": "slice-26: a second version publishes into the existing registry (no re-initialization) — count 1 -> 2, slot 1 = 1"}));
    let rr26d = accounts.iter().find(|(k, _)| *k == rule_registry).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(rr26d.data[8], 2u8, "registry count not 2 after register_rule_version:26second");
    assert_eq!(u64::from_le_bytes(rr26d.data[17..25].try_into().unwrap()), 1u64, "registry slot 1 not 1 after register_rule_version:26second");
    let mut data26e = disc("unregister_rule_version");
    data26e.extend_from_slice(&9u64.to_le_bytes());
    let ix26e = Instruction::new_with_bytes(
        program_id,
        &data26e,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(rule_registry, false),
            AccountMeta::new(authority, true),
        ],
    );
    let cu26e = run_expect_err(&mollusk, "unregister_rule_version:26absent", &ix26e, &accounts, 6103);
    measured.push(serde_json::json!({"instruction": "unregister_rule_version:26absent", "cu": cu26e, "result": "Refused", "error": "RuleVersionNotRegistered", "error_code": 6103, "note": "slice-26 refusal: unregistering never-published version 9 refuses RuleVersionNotRegistered (6103) — a no-op must not masquerade as a revocation, NOT a success CU"}));
    let mut data26f = disc("unregister_rule_version");
    data26f.extend_from_slice(&3u64.to_le_bytes());
    let ix26f = Instruction::new_with_bytes(
        program_id,
        &data26f,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(rule_registry, false),
            AccountMeta::new(authority, true),
        ],
    );
    let (next, cu) = run(&mollusk, "unregister_rule_version:26", &ix26f, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "unregister_rule_version:26", "cu": cu, "result": "Success", "note": "slice-26: genesis authority removes version 3 — swap-compaction leaves [1], count 2 -> 1; removal gates future allocates only"}));
    let rr26f = accounts.iter().find(|(k, _)| *k == rule_registry).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(rr26f.data[8], 1u8, "registry count not 1 after unregister_rule_version:26");
    assert_eq!(u64::from_le_bytes(rr26f.data[9..17].try_into().unwrap()), 1u64, "registry slot 0 not 1 after unregister_rule_version:26");
    let mut data26g = disc("register_rule_version");
    data26g.extend_from_slice(&3u64.to_le_bytes());
    let ix26g = Instruction::new_with_bytes(
        program_id,
        &data26g,
        vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(rule_registry, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_rule_version:26restore", &ix26g, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "register_rule_version:26restore", "cu": cu, "result": "Success", "note": "slice-26 restore: version 3 re-publishes after removal, returning the registry to [1, 3], count 2 — a removed version is publishable again, and the published list is the allocate gate's existence proof"}));
    let rr26g = accounts.iter().find(|(k, _)| *k == rule_registry).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(rr26g.data[8], 2u8, "registry count not 2 after register_rule_version:26restore");


    // Slice-27 end-of-chain probes: relevant_groups eligibility.
    // Transition 27 registers with relevant_groups [1] on the
    // existing observation 1 / window policy (no new observation
    // needed: observation_id is its own argument). Clock is
    // 47,468,000 at the tail, so a 100,000,000 expiry is live.
    let (transition27, _) = pda(&[b"transition", &27u64.to_le_bytes()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == transition27) { accounts.push((transition27, empty.clone())); }
    let (plan27_agent, _) = pda(&[b"plan", &27u64.to_le_bytes(), agent.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == plan27_agent) { accounts.push((plan27_agent, empty.clone())); }
    let (signal27_agent, signal27_agent_bump) = pda(&[b"signal", &27u64.to_le_bytes(), &0u32.to_le_bytes(), agent.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == signal27_agent) { accounts.push((signal27_agent, empty.clone())); }
    let (plan27_proposer, _) = pda(&[b"plan", &27u64.to_le_bytes(), proposer.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == plan27_proposer) { accounts.push((plan27_proposer, empty.clone())); }
    let (signal27_proposer, signal27_proposer_bump) = pda(&[b"signal", &27u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()], &program_id);
    if !accounts.iter().any(|(k, _)| *k == signal27_proposer) { accounts.push((signal27_proposer, empty.clone())); }
    let mut data27 = disc("register_transition");
    data27.extend_from_slice(&27u64.to_le_bytes());
    data27.extend_from_slice(&1u64.to_le_bytes());
    data27.extend_from_slice(&type_window());
    data27.extend_from_slice(&100_000_000i64.to_le_bytes());
    data27.extend_from_slice(&[27u8; 32]);
    data27.extend_from_slice(&0u64.to_le_bytes());
    data27.extend_from_slice(&[0u8; 32]);
    data27.extend_from_slice(&vec32(&[]));
    data27.extend_from_slice(&vec_u16(&[1]));
    let ix27 = Instruction::new_with_bytes(
        program_id,
        &data27,
        vec![
            AccountMeta::new_readonly(observation, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(transition27, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_transition:27", &ix27, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "register_transition:27", "cu": cu, "result": "Success", "note": "slice-27: transition 27 registers with relevant_groups [1] — TransitionRecord SPACE 886, frozen list (len 1, group 1) serialized after the constraint set"}));
    let tr27 = accounts.iter().find(|(k, _)| *k == transition27).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(tr27.data.len(), 886, "transition 27 account size not 886 after slice-27");
    // Borsh serializes the variable-length vectors compactly, so the
    // relevant_groups list does NOT sit at the SPACE tail: it follows
    // the frozen constraint set (offset 137 holds its u32 length, each
    // entry 33 bytes; the fields after it sum to 189 bytes, plus the
    // 8-byte discriminator, so the list starts at 330 + 33*n).
    let n27 = u32::from_le_bytes(tr27.data[137..141].try_into().unwrap()) as usize;
    let off27 = 330 + 33 * n27;
    assert_eq!(u32::from_le_bytes(tr27.data[off27..off27 + 4].try_into().unwrap()), 1u32, "transition 27 relevant_groups len not 1");
    assert_eq!(u16::from_le_bytes(tr27.data[off27 + 4..off27 + 6].try_into().unwrap()), 1u16, "transition 27 relevant_groups[0] not 1");
    let mut data27 = disc("open_planning");
    data27.extend_from_slice(&27u64.to_le_bytes());
    let ix27 = Instruction::new_with_bytes(
        program_id,
        &data27,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition27, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "open_planning:27", &ix27, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "open_planning:27", "cu": cu, "result": "Success", "note": "slice-27 setup: transition 27 opens planning (deadline = tail clock + 3600)"}));
    // The disjoint agent (groups [], Admin, active) plans and
    // signals against [1] and refuses NotEligible (6010) on both
    // gates — the kernel's eligible rule, now on-chain.
    let mut data27 = disc("submit_plan");
    data27.extend_from_slice(&27u64.to_le_bytes());
    data27.extend_from_slice(&vec_claims(&[(7u16, 100u64)]));
    data27.extend_from_slice(&100_000_000i64.to_le_bytes());
    let ix27 = Instruction::new_with_bytes(
        program_id,
        &data27,
        vec![
            AccountMeta::new_readonly(agent_record, false),
            AccountMeta::new(transition27, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(plan27_agent, false),
            AccountMeta::new(agent, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let cu27a = run_expect_err(&mollusk, "submit_plan:27disjoint", &ix27, &accounts, 6010);
    measured.push(serde_json::json!({"instruction": "submit_plan:27disjoint", "cu": cu27a, "result": "Refused", "error": "NotEligible", "error_code": 6010, "note": "slice-27 refusal: an agent with no overlapping group plans against relevant_groups [1] and refuses NotEligible (6010) — the group-overlap half of kernel eligibility, NOT a success CU"}));
    let mut data27 = disc("submit_signal");
    data27.extend_from_slice(&27u64.to_le_bytes());
    data27.extend_from_slice(&0u32.to_le_bytes());
    data27.extend_from_slice(&1_000_000u64.to_le_bytes());
    data27.extend_from_slice(&1_000_000u64.to_le_bytes());
    data27.extend_from_slice(&[signal27_agent_bump]);
    let ix27 = Instruction::new_with_bytes(
        program_id,
        &data27,
        vec![
            AccountMeta::new_readonly(agent_record, false),
            AccountMeta::new(transition27, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(signal27_agent, false),
            AccountMeta::new(agent, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let cu27b = run_expect_err(&mollusk, "submit_signal:27disjoint", &ix27, &accounts, 6010);
    measured.push(serde_json::json!({"instruction": "submit_signal:27disjoint", "cu": cu27b, "result": "Refused", "error": "NotEligible", "error_code": 6010, "note": "slice-27 refusal: the same disjoint agent signals against relevant_groups [1] and refuses NotEligible (6010) before any creation or aggregate fold — NOT a success CU"}));
    // The overlapping proposer (group [1]) plans and signals on the
    // same transition without friction — the gate admits, not just
    // refuses.
    let mut data27 = disc("submit_plan");
    data27.extend_from_slice(&27u64.to_le_bytes());
    data27.extend_from_slice(&vec_claims(&[(7u16, 100u64)]));
    data27.extend_from_slice(&100_000_000i64.to_le_bytes());
    let ix27 = Instruction::new_with_bytes(
        program_id,
        &data27,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition27, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(plan27_proposer, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(clock_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_plan:27", &ix27, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_plan:27", "cu": cu, "result": "Success", "note": "slice-27: the overlapping proposer (group [1]) plans on transition 27 — plan_count 0 -> 1"}));
    let mut data27 = disc("submit_signal");
    data27.extend_from_slice(&27u64.to_le_bytes());
    data27.extend_from_slice(&0u32.to_le_bytes());
    data27.extend_from_slice(&1_000_000u64.to_le_bytes());
    data27.extend_from_slice(&1_000_000u64.to_le_bytes());
    data27.extend_from_slice(&[signal27_proposer_bump]);
    let ix27 = Instruction::new_with_bytes(
        program_id,
        &data27,
        vec![
            AccountMeta::new_readonly(proposer_record, false),
            AccountMeta::new(transition27, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new(signal27_proposer, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_signal:27", &ix27, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_signal:27", "cu": cu, "result": "Success", "note": "slice-27: the overlapping proposer signals round 0 on transition 27 — the gate admits an overlapping agent"}));

    let golden = serde_json::json!({
        "harness": "cu-golden-mollusk",
        "mollusk_svm": MOLLUSK_SVM_VERSION,
        "program_id": PROGRAM_ID,
        "program_so_sha256": so_sha256,
        "token_program_id": TOKEN_PROGRAM_ID_STR,
        "token_program_so_sha256": token_elf_sha256,
        "slice": 27,
        "slice_note": "slices 1-8 as before (slice-5/6 refusal probes stand as the unfunded-chain record; slice-7 FUNDED chain: real SPL Token deposit credits TreasuryState 60,000,000, window 1 opens on the funded snapshot, borrow / exit / settle_default / schedule_whale_exit FIRST successes; slice-8 12-window walk: windows 2..13 each open on their own Settled treasury.window.v1 transition (transitions 9..20, parented in sequence), each on the funded snapshot (total 60,000,000, exit stock 10,000,000, FLAG 2,000,000), and the FIRST success measurement execute_whale_tranche (tranche 1 of 5, 2,000,000 raw, on Committed treasury.exit.v1 transition 8, creation window 1, execution window 13 >= lock_until 13), gated §2.2 ~7K (2x band 3,500-14,000)) + slice-9 W3.2 re-expression part 1: tranche-2 weekly cadence on the slice-8 chain — TrancheTooSoon refusals at the tranche-1 clock and at last_block_time + WEEK_SECS - 1, then tranche 2 of 5 (2,000,000 raw) SUCCESS at exactly last_block_time + WEEK_SECS (31,608,400), gated §2.2 ~7K; launch_stage2 unmeasured by design (ThresholdUnset, D-2) + slice-10 W3.2 re-expression part 2: blind-ape bulk exit (T11) on the funded chain — the whole-position order (23,500,000 raw) refuses ExitExceedsLiquidity under the D-10 hard bound (L = 10,000,000), the largest executable bulk exit (8,500,000 raw, the remaining window-1 stock) succeeds with the flat 13% penalty on the full excess (1,105,000 penalty, 7,395,000 paid), and a further window-1 exit refuses ExitStockExhausted with aggregate window-1 exits exactly at L + slice-11 W3.2 re-expression part 3: default-as-exit (T10) on twin holders in window 13 (position 10,000,000 each) — borrow above the cap refuses LoanExceedsPosition, the max loan 5,000,000 succeeds at the inclusive boundary of the 50%-of-position and book caps, pre-maturity settle_default refuses LoanNotMatured, at-maturity settle_default succeeds (position 10,000,000 -> 5,000,000, skip flag set), and the PROBE: pay_tranche AFTER the default SUCCEEDS despite skip_next_dividend set, because can_pay_tranche takes no skip argument — the forfeiture half of T10 is unenforced on the program today, the one-cycle ledgers come out EQUAL (11,000,000 each), and T10 is NOT claimed re-expressed until the carried skip-enforcement item lands; window 13 exit stock ends 4,000,000, clock ends 36,792,400 + slice-12 W3.2 re-expression part 4: cohort-timing at window boundaries (D-11) — enroll into past window 12 refuses NotCurrentWindow, zero-position enroll into current window 13 refuses InvalidPosition, a double enroll in window 13 refuses at the account layer (one record per holder per window), a current-window-13 enroll succeeds (cohort 1); then the boundary is crossed for real (Settled transition 21 opens window 14, window_count 15): the just-closed window 13 now refuses NotCurrentWindow, a window-14 enroll succeeds (cohort 2), a future window-15 enroll refuses at the account layer (no window account exists yet), and the new cohort's pay_tranche 1,000,000 succeeds on its own slot (14 % 12 = 2), gated §2.2 ~7K; window 13 stock ends 4,000,000, window 14 stock 10,000,000, clock ends 37,003,600 + slice-13 W3.2 re-expression part 5: fee-flow edge cases under the single 73 bps stream (D-5) — a second real deposit 10,000,000 credits TreasuryState by exactly 10,000,000 (a funding-time skim would credit 9,927,000) with vault/source conserving exactly and FeeVault collected_raw still 0, and a mid-window deposit does not inflate window 14's snapshot or stock; holder 27's all-accrued exit 1,000,000 takes neither penalty nor fee (position stays 5,000,000, stock 10,000,000 -> 9,000,000) and a further 1,000,000 all-excess exit carries exactly the flat 13% penalty (position 5,000,000 -> 4,000,000, stock 9,000,000 -> 8,000,000, can_exit has no FEE_PPM term so no second skim exists); the negative half of the fee flow is proven, but the positive half — collection and reconciliation — is NOT: FeeVault collected_raw is still 0 because collect_fees is unbuilt, and the 73 bps income identity has no on-chain realization until that remaining §4.2 row lands + slice-14 the remaining §4.2 row `reinvest` (design §4.2, engine-sim T8) on a fresh holder 29 in window 14 — enroll 5,000,000 (cohort 2), pay_tranche 1,000,000 on Committed window-flow transition 7, then reinvest 400,000 (accrued 1,000,000 -> 600,000, position 5,000,000 -> 5,400,000, window 14 exit stock UNCHANGED at 8,000,000, TreasuryState UNCHANGED), a 600,001 reinvest refusing ReinvestExceedsAccrued (6087), the remaining 600,000 reinvesting at the inclusive boundary (accrued -> 0, position -> 6,000,000), and a zero reinvest refusing InvalidReinvestAmount (6086); the reinvest reading is accrued-dividend-only, partial allowed, no stock debit, no penalty, no value created — reinvesting a borrowed amount is NOT this instruction and needs its own ruling; program IDL 37 -> 38 instructions, 85 -> 88 errors, accounts 16, host tests 82 -> 83, and slices 1-13 re-measured against the new .so (sha e0f40adf...) with any shifts reported in the run log, not silently re-baselined + slice-15 the last §4.2 row `collect_fees` (design §4.2, D-5) on the slice-14 chain — operator collects 73,000 (73 bps on a 10,000,000 flow, operator-reported) in window 14 on Committed window flow transition 7 (window fees_collected_raw 0 -> 73,000, FeeVault collected_raw 0 -> 73,000, exit stock still 8,000,000, TreasuryState still 70,000,000), a further 27,000 accumulates in the same window (window and vault both 100,000 — the vault total is the sum over windows), a zero collection refuses InvalidCollectAmount (6090), and an agent without OPERATOR_SETTLE refuses Unauthorized (6000); the fee flow now has its positive half as on-chain accounting, but the design row's numeric reconciliation against the declared total is NOT implemented (the TransitionRecord carries only a declared_hash, not a fee total) and rides as a carried item; program IDL 38 -> 39 instructions, 88 -> 91 errors, accounts 16, host tests 83 -> 84, WindowState SPACE 121 -> 129 (fees_collected_raw appended after bump), and slices 1-14 re-measured against the new .so with any shifts reported in the run log, not silently re-baselined + slice-16 skip-next-dividend enforcement (D-3, the carried item from slice 11): can_pay_tranche takes the skip flag and refuses DividendSkipped (6091) — the slice-11 probe tranche now REFUSES (accrued 0, flag stays set, slot unpaid), twin D's replacement 1,000,000 exit comes from position at the flat 13% penalty (paid 870,000, position 5,000,000 -> 4,000,000), and the one-cycle ledgers are strictly worse for the defaulter (9,870,000 vs 11,000,000), so T10 is claimed re-expressed on the record that carries the flag; the cross-record carry to a later window's fresh record stays carried; program IDL 39 instructions, 91 -> 92 errors, accounts 16, host tests 84 (pay_tranche_gate_matrix gains the skip case, no new test fn), and slices 1-15 re-measured against the new .so with any shifts reported in the run log, not silently re-baselined + slice-17 the open_window source-transition consumption fix (the carried Part B4 item): TransitionRecord gains window_open_applied (SPACE 865 -> 866, appended after the signalled bitmap, no field moves), open_window sets it in the same instruction as the counter advance and window creation, and a second open on the same Settled transition refuses SourceTransitionConsumed (6092) — the slice-17 probe reuses transition 21 (which opened window 14) for window 15 and refuses, window_count stays 15, no window 15 is created; every open_window caller now passes the transition writable, since the handler writes the flag; program IDL 39 instructions, 92 -> 93 errors, accounts 16, host tests 84 (window_open_gate_matrix gains the consumed case, no new test fn), and slices 1-16 re-measured against the new .so with any shifts reported in the run log, not silently re-baselined + slice-18 the declared-fee-schedule reconciliation (the carried item from slice 15): WindowState gains declared_fees_raw + fee_schedule_declared (SPACE 129 -> 138, appended after fees_collected_raw, no field moves), a new instruction declare_fee_schedule sets the window's schedule once under its Committed window flow transition (a second declaration refuses FeeScheduleAlreadyDeclared, 6093), and collect_fees now refuses in an undeclared window (FeeScheduleUndeclared, 6094) and past the declared total (CollectExceedsDeclaredSchedule, 6095) — on this chain window 14's schedule is declared at 100,000 before the slice-15 collections, which land exactly on it (73,000 + 27,000), a further 1 refuses as over-declared, window 13 collects nothing while undeclared, a declared zero for window 13 is a real declaration (a further 1 then refuses as over-declared, not undeclared); the declaration itself is operator-reported, so this slice bounds collections by a prior immutable declaration and makes shortfall visible, it does not independently verify the declaration against compose-layer volume; program IDL 39 -> 40 instructions, 93 -> 96 errors, accounts 16, host tests 84 -> 85 (declare_fee_schedule_gate_matrix added, collect_fees_gate_matrix gains the reconciliation cases), and slices 1-17 re-measured against the new .so with any shifts reported in the run log, not silently re-baselined + slice-19 the deposit↔enroll binding (the carried item from the funding path): WindowState gains enrolled_total_raw (SPACE 138 -> 146, appended after fee_schedule_declared, no field moves), open_window initializes it to 0, and enroll now refuses EnrollExceedsFundedSnapshot (6096) when the window's cumulative enrolled total plus the new position would exceed the window's period-start snapshot total — enrolled positions are bounded by deposits actually held at open, and a mid-window deposit cannot inflate the current window's enrollment budget (design §7). In an unfunded window (snapshot total zero) the cap does not apply: pre-funding enrollments are opted out of contract under the funding-path ruling, permitted and tracked, never refused for exceeding a zero bound — so the slice-4 unfunded window-0 enrollment (10,000,000 on a zero snapshot) still succeeds, and the slice-12/14 window-14 enrollments (5,000,000 + 5,000,000) still succeed under the 60,000,000 snapshot. On the chain, a 50,000,000 enrollment in window 14 lands the cumulative total exactly on the 60,000,000 snapshot (Success, the inclusive boundary), and 1 more refuses 6096 with the enrolled total unchanged at 60,000,000. Honest scope: this is the window-level binding — no per-depositor ledger exists, so one holder's position is not checked against that holder's own deposits; per-holder deposit backing remains future work. Program IDL 40 instructions, 96 -> 97 errors, accounts 16, host tests 85 (enroll_gate_matrix gains the binding cases, no new test fn), and slices 1-18 re-measured against the new .so with any shifts reported in the run log, not silently re-baselined + slice-21 native-SOL deposits (the carried funding-path item): a new instruction deposit_sol transfers lamports by system CPI into the sol-vault PDA ([sol-vault, treasury]) and credits TreasuryState by exactly the transferred amount — first deposit 5,000,000 lamports (total and cash 70,000,000 -> 75,000,000, vault lamports 5,000,000, system-owned, empty data), second deposit 1,000,000 into the existing vault (76,000,000, vault 6,000,000), zero deposit refuses InvalidDepositAmount (6081) with totals unchanged; SOL deposits are NOT registry-gated (no mint to register) and credit lamports raw into the same total/cash ledger token deposits use, so a mixed USDC-raw/lamport total is mixed-scale and no disbursement may treat it as single-currency without a conversion step this slice does not build; program IDL 42 -> 43 instructions, 101 errors, accounts 16, host tests 86 -> 87 (deposit_sol_gate_matrix added), and slices 1-20 re-measured against the new .so with any shifts reported in the run log, not silently re-baselined + slice-22 the cross-record D-3 carry (the carried item from slice 16): a holder-scoped ForfeitureRecord PDA [forfeiture, holder] (SPACE 57, program IDL accounts 16 -> 17, instructions 43, errors 101, host tests 87 -> 89) — settle_default queues one pending slot per default, enroll consumes exactly one and sets the new record's skip flag iff one was pending, so a default forfeits the next enrolled window's slot and no later one; on this chain window 15 opens on Settled transition 22 (snapshot 76,000,000, stock 12,666,666), twin D (defaulted window 13) and holder [8] (defaulted window 1) both enroll in window 15 with skip set and pending consumed to zero, D's window-15 pay_tranche refuses DividendSkipped (6091), and a fresh holder enrolls clear — the D-3 forfeiture is re-expressed end to end; every prior enroll and settle_default call site now passes its holder's forfeiture PDA (init_if_needed, anchor-lang feature enabled with the PDA-bound re-initialization argument in the program Cargo.toml), and slices 1-21 re-measured against the new .so with any shifts reported in the run log, not silently re-baselined + slice-23 borrowed-amount reinvestment (the 2026-10-26 ruling, decided on Carlos's instruction): a new instruction `borrow_reinvest` (program IDL 43 -> 44 instructions, 101 -> 102 errors, accounts 17, host tests 89 -> 90; HolderRecord SPACE 100 -> 108 for the appended loan_funded_raw field) — atomic with the borrow (loan recorded + position grown by exactly the borrowed amount, no cash disbursed, TreasuryState totals unchanged), capacity on the owned position (position minus unrepaid loan-funded increments), one active loan per holder per window, exit-while-indebted unchanged, and a default settlement at maturity returning the position to exactly its pre-loan size with the funded figure extinguished; on this chain in window 15 (snapshot 76,000,000, book cap 6,333,333): holder 33 enrolls 10,000,000, borrow_reinvests 5,000,000 at the inclusive boundary of both caps (position 15,000,000, funded 5,000,000, book 5,000,000, TreasuryState 76,000,000 unchanged), a second concurrent loan refuses LoanAlreadyActive (6050), an 11,000,000 exit refuses ExitBelowLoan (6062), holder 34's 5,000,001 borrow_reinvest refuses LoanExceedsPosition (6052, owned cap judged before book cap), and settle_default at maturity 42,284,000 returns holder 33 to 10,000,000 with funded 0, skip set, and one forfeiture slot queued (source window 15); the owned-cap itself is gate-proven in the host matrix (position 14,000 with funded 4,000 caps at 5,000, not 7,000) because on-chain it is shadowed by the one-active-loan rule until a (partial-)repayment path exists; program IDL 44 instructions, 102 errors, accounts 17, host tests 90, and slices 1-22 re-measured against the new .so with any shifts reported in the run log, not silently re-baselined + slice-24 the repeat-default measurement (the carried item from slices 22-23): holder 33 re-borrows 5,000,000 plain on the same window-15 record after the slice-23 default cleared its loan (position stays 10,000,000, loan-funded stays 0, book 0 -> 5,000,000, forfeiture pending stays 1 — borrow never touches the forfeiture record), then defaults a second time at maturity 47,468,000 against the already-existing ForfeitureRecord (no init_if_needed creation): position 10,000,000 -> 5,000,000, loan 0, funded 0, skip flag stays set, forfeiture pending 1 -> 2 with source still window 15; the settle_default:24 CU is the repeat-default figure, directly comparable to the first-default 20,323 CU of settle_default:23; no program code change (measurement only), program IDL 44 instructions, 102 errors, accounts 17, host tests 90, and slices 1-23 re-measured against the same .so with any shifts reported in the run log, not silently re-baselined + slice-25 the D-2 threshold-B writer path as a genesis parameter: `initialize_treasury` gains `backing_threshold_ppm` (validated by `check_backing_threshold_ppm`: 0 allowed as unset, any value to 1,000,000 ppm allowed, above full backing refuses ThresholdExceedsFullBacking, 6102); the chain pins the initial-testing value 950,000 ppm (95%, relatively close to USDC-style full backing — a genesis choice, not a final ratio), asserted at TreasuryState bytes 48..56; no account moves, no field moves, no new account type; program IDL 44 instructions, 102 -> 103 errors, accounts 17, host tests 90 -> 91 (backing_threshold_genesis_parameter added, launch gate matrix unchanged); `initialize_treasury` measures 26,367 CU (+14 on the slice-24 26,353: the extra argument decode plus the threshold check, the threshold write itself replacing the old zero write); slices 1-24 re-measured against the new .so with any shifts reported in the run log, not silently re-baselined + slice-26 the allocation rule registry (the carried item from the allocate gap): a standalone RuleRegistry PDA [\"rule-registry\"] (SPACE 138, count + 16 versions + bump), genesis-authority-signed register_rule_version / unregister_rule_version (Admin pattern, as register_mint; removal gates future allocates only, never clawing back executed allocations); allocate now reads the registry and refuses an unregistered version (RuleVersionNotRegistered, 6103) with an empty registry accepting nothing — the second allocate gap is closed, the amount-vs-committed-claim binding stays carried. On this chain: register version 3 creates the registry, a duplicate refuses 6104, version 0 refuses 6075, a second version publishes into the existing registry, unregistering absent version 9 refuses 6103, removing version 3 swap-compacts to [1], and re-publishing version 3 restores [1, 3]; program IDL 44 -> 46 instructions, 103 -> 106 errors, accounts 17 -> 18, host tests 91 -> 93 (rule_registry_matrix, rule_registry_space_matches_fields), and slices 1-25 re-measured against the new .so with any shifts reported in the run log, not silently re-baselined + slice-27 the relevant_groups manifest wiring (the carried eligibility gap from the plan/submit slices): TransitionRecord gains relevant_groups (SPACE 866 -> 886, appended after window_open_applied, no field moves), register_transition takes and freezes the list (at most MAX_GROUPS, else InvalidGroups), and submit_plan / submit_signal now enforce the kernel's full eligible rule (active AND group-overlap, NotEligible 6010) — on this chain the disjoint agent (groups []) refuses on both gates against [1] while the overlapping proposer (group [1]) plans and signals cleanly; program IDL 46 instructions, 106 errors, accounts 18, host tests 93 (gate matrices gain the disjoint cases, no new test fn), and slices 1-26 re-measured against the new .so with any shifts reported in the run log, not silently re-baselined",
        "clock": {"phase_a_unix_timestamp": NOW0, "planning_window_s": PLANNING_WINDOW, "phase_b_unix_timestamp": NOW1},
        "instructions": measured,
    });
    std::fs::write(&out_path, serde_json::to_string_pretty(&golden).unwrap())
        .unwrap_or_else(|e| panic!("cannot write golden file {out_path}: {e}"));
    println!("golden written to {out_path} ({} accounts threaded)", accounts.len());
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
