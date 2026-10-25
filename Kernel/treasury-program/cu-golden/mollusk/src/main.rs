//! W4.3 CU golden-test harness (mollusk-svm path), slice 2.
//!
//! Extends the slice-1 genesis/admin chain with the first GATED
//! measurement: a full governance lifecycle (policy install,
//! observation, transition, planning, plan, signal, constraint
//! report, reconcile, commit) executed in-process (no validator, no
//! network, no devnet, no mainnet). Every instruction must succeed;
//! any failure aborts with a non-zero exit, so a golden file is never
//! written from a failed chain.
//!
//! Slice 2's `commit` is gated against design §2.2 (25,000-40,000 CU
//! EST): a measurement missing that band by more than 2x
//! (outside 12,500-80,000 CU) is recorded as a DESIGN-FINDING, never
//! tuned away. The chain here is minimal (1 plan, 1 signal, 1
//! constraint, 1 stock), so the gated value is the minimal-chain
//! commit cost; larger transitions cost more and are measured next.
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
    let fee_vault = Pubkey::new_from_array([2u8; 32]);
    let agent = Pubkey::new_from_array([3u8; 32]);
    let proposer = Pubkey::new_from_array([4u8; 32]);
    let evaluator = Pubkey::new_from_array([5u8; 32]);
    let oracle = Pubkey::new_from_array([6u8; 32]);
    let (config, _) = pda(&[b"config"], &program_id);
    let (chain, _) = pda(&[b"chain"], &program_id);
    let (agent_record, _) = pda(&[b"agent", agent.as_ref()], &program_id);
    let (proposer_record, _) = pda(&[b"agent", proposer.as_ref()], &program_id);
    let (evaluator_record, _) = pda(&[b"agent", evaluator.as_ref()], &program_id);
    let (oracle_record, _) = pda(&[b"agent", oracle.as_ref()], &program_id);
    let (stock, _) = pda(&[b"stock", &7u16.to_le_bytes()], &program_id);
    let (policy, _) = pda(&[b"policy", type_window().as_ref()], &program_id);
    let (observation, _) = pda(&[b"observation", &1u64.to_le_bytes()], &program_id);
    let (transition, _) = pda(&[b"transition", &1u64.to_le_bytes()], &program_id);
    let (plan, _) = pda(&[b"plan", &1u64.to_le_bytes(), proposer.as_ref()], &program_id);
    let (signal, _) = pda(
        &[b"signal", &1u64.to_le_bytes(), &0u32.to_le_bytes(), proposer.as_ref()],
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
        (plan, empty.clone()),
        (signal, empty.clone()),
        (authority, funded()),
        (proposer, funded()),
        (evaluator, funded()),
        (oracle, funded()),
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

    let golden = serde_json::json!({
        "harness": "cu-golden-mollusk",
        "mollusk_svm": MOLLUSK_SVM_VERSION,
        "program_id": PROGRAM_ID,
        "program_so_sha256": so_sha256,
        "slice": 2,
        "slice_note": "slice-1 genesis/admin chain + slice-2 full governance lifecycle to a committed transition; commit is the first GATED measurement (design §2.2 25-40K EST, 2x band 12.5-80K)",
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
