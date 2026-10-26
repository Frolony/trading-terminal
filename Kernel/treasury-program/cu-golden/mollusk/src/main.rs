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
    // 6. register_agent(proposer, [], influence 1_000_000, 0)
    let mut data = disc("register_agent");
    data.extend_from_slice(proposer.as_ref());
    data.extend_from_slice(&vec_u16(&[]));
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
    let mut data = disc("initialize_treasury");
    data.extend_from_slice(&2u64.to_le_bytes());
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
    measured.push(serde_json::json!({"instruction": "initialize_treasury", "cu": cu, "result": "Success", "note": "slice-4 baseline (no §2.2 estimate)"}));
    let ts = accounts.iter().find(|(k, _)| *k == treasury_state).map(|(_, a)| a.clone()).unwrap();
    assert_eq!(ts.data.len(), 65, "TreasuryState size changed");
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
            AccountMeta::new_readonly(transition3, false),
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
    assert_eq!(ws.data.len(), 121, "WindowState size changed");
    assert_eq!(u64::from_le_bytes(ws.data[8..16].try_into().unwrap()), 0u64, "window_id not 0");
    assert_eq!(u64::from_le_bytes(ws.data[72..80].try_into().unwrap()), 3u64, "window source transition not 3");

    // (d) enroll the holder into window 0 (cohort 0 = 0 mod 12).
    let mut data = disc("enroll");
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&10_000_000u64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state, false),
            AccountMeta::new(holder_record, false),
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
            AccountMeta::new_readonly(transition6, false),
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
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(treasury_state, false),
            AccountMeta::new(window_state_1, false),
            AccountMeta::new(holder_record_1, false),
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
                AccountMeta::new_readonly(transition_w, false),
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

    let golden = serde_json::json!({
        "harness": "cu-golden-mollusk",
        "mollusk_svm": MOLLUSK_SVM_VERSION,
        "program_id": PROGRAM_ID,
        "program_so_sha256": so_sha256,
        "token_program_id": TOKEN_PROGRAM_ID_STR,
        "token_program_so_sha256": token_elf_sha256,
        "slice": 8,
        "slice_note": "slices 1-7 as before (slice-5/6 refusal probes stand as the unfunded-chain record; slice-7 FUNDED chain: real SPL Token deposit credits TreasuryState 60,000,000, window 1 opens on the funded snapshot, borrow / exit / settle_default / schedule_whale_exit FIRST successes) + slice-8 12-window walk: windows 2..13 each open on their own Settled treasury.window.v1 transition (transitions 9..20, parented in sequence), each on the funded snapshot (total 60,000,000, exit stock 10,000,000, FLAG 2,000,000), and the FIRST success measurement execute_whale_tranche (tranche 1 of 5, 2,000,000 raw, on Committed treasury.exit.v1 transition 8, creation window 1, execution window 13 >= lock_until 13), gated §2.2 ~7K (2x band 3,500-14,000); tranche 2's weekly cadence (last_block_time + WEEK_SECS) is not measured here; launch_stage2 unmeasured by design (ThresholdUnset, D-2)",
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
