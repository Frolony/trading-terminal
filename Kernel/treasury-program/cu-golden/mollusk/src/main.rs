//! W4.3 CU golden-test harness (mollusk-svm path), slice 4.
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
    let (signal, _) = pda(
        &[b"signal", &1u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
        &program_id,
    );
    let (policy_init, _) = pda(&[b"policy", type_init().as_ref()], &program_id);
    let (observation2, _) = pda(&[b"observation", &2u64.to_le_bytes()], &program_id);
    let (transition2, _) = pda(&[b"transition", &2u64.to_le_bytes()], &program_id);
    let (evidence2, _) = pda(&[b"evidence", &2u64.to_le_bytes()], &program_id);
    let (plan2, _) = pda(&[b"plan", &2u64.to_le_bytes(), proposer.as_ref()], &program_id);
    let (signal2, _) = pda(
        &[b"signal", &2u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
        &program_id,
    );
    let (observation3, _) = pda(&[b"observation", &3u64.to_le_bytes()], &program_id);
    let (transition3, _) = pda(&[b"transition", &3u64.to_le_bytes()], &program_id);
    let (plan3, _) = pda(&[b"plan", &3u64.to_le_bytes(), proposer.as_ref()], &program_id);
    let (signal3, _) = pda(
        &[b"signal", &3u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
        &program_id,
    );
    let (policy_sha, _) = pda(&[b"policy", type_window_sha().as_ref()], &program_id);
    let (evidence3, _) = pda(&[b"evidence", &3u64.to_le_bytes()], &program_id);
    let (observation4, _) = pda(&[b"observation", &4u64.to_le_bytes()], &program_id);
    let (transition4, _) = pda(&[b"transition", &4u64.to_le_bytes()], &program_id);
    let (plan4, _) = pda(&[b"plan", &4u64.to_le_bytes(), proposer.as_ref()], &program_id);
    let (signal4, _) = pda(
        &[b"signal", &4u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
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
    assert_eq!(cfg.data.len(), 74, "Config account size changed");
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
            AccountMeta::new_readonly(config, false),
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
            AccountMeta::new_readonly(config, false),
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
            AccountMeta::new_readonly(config, false),
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
            AccountMeta::new_readonly(config, false),
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
            AccountMeta::new_readonly(config, false),
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

    let golden = serde_json::json!({
        "harness": "cu-golden-mollusk",
        "mollusk_svm": MOLLUSK_SVM_VERSION,
        "program_id": PROGRAM_ID,
        "program_so_sha256": so_sha256,
        "slice": 5,
        "slice_note": "slice-1 genesis/admin chain + slice-2 full governance lifecycle + slice-3 evidence and exact settle + slice-4 init lifecycle to a settled transition, initialize_treasury, open_window (window 0), enroll, flow lifecycle stopped at commit, and the first GATED value-moving measurement pay_tranche (design §2.2 ~7K EST, 2x band 3.5-14K) + slice-5 blocked-family refusal probes (borrow BookCapExceeded 6053, exit ExitExceedsLiquidity 6060, settle_default NoActiveLoan 6055 — refusal CU, NOT success CU); launch_stage2 unmeasured by design (ThresholdUnset, D-2); borrow/exit/settle_default success CU remain blocked on a funding path that puts a nonzero total into TreasuryState, which no instruction on-chain provides yet",
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
