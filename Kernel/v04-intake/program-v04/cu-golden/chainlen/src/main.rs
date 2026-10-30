//! Chain-length CU fixture for `program-v04` (merge-gate item 5, CU leg).
//!
//! For each N in [0, 1, 2, 3, 5], in a FRESH mollusk instance:
//! genesis + seal, then N ordinary bundles driven to COMMITTED at
//! successive heads (each bundle's own 200-tick window, same shape as
//! the golden harness's life chain: two-claims plan, 10 + 5 SCALE),
//! then a governance tail (GrantRole action, unanimous council,
//! empty-claims plan) registered at the moved head with
//! prior_bundle_count = N, driven through commit, settlement
//! evidence, settle, and `record_outcome_governance`. The measured
//! value is the outcome instruction's CU at that prior count.
//!
//! The heap leg (host fixture `chain_length_outcome_heap_vs_priors`)
//! already proved linear heap growth with a predict-5 fit; this
//! binary proves or refutes the same shape in real SBF compute
//! units. Keys are throwaway local-test keys, never real secrets.

use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::Mollusk;
use sha2::{Digest, Sha256};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::str::FromStr;

const PROGRAM_ID: &str = "2CQ58pLCb5AsaKxLudcbuiPHGphtyv9cKtDKK5mdxBvr";
const SYSTEM_ID: &str = "11111111111111111111111111111111";
const MOLLUSK_SVM_VERSION: &str = "0.16.4";
const TOOLCHAIN: &str = "Anchor 1.2.2 / Solana 4.3.2 / platform-tools v1.57";

const SCALE: u64 = 1_000_000;
const RECORDER: [u8; 32] = [0xB4; 32];
const TT_TYPE: [u8; 32] = [9; 32];
const ZERO_HASH: [u8; 32] = [0; 32];
const TAIL_TID: [u8; 32] = [0xD0; 32];
const TAIL_OBS: [u8; 32] = [0xC9; 32];
const TAIL_DECLARED: [u8; 32] = [0xF0; 32];

fn assert_mollusk_stamp_matches_lock() {
    let lock_path = format!("{}/Cargo.lock", env!("CARGO_MANIFEST_DIR"));
    let lock = std::fs::read_to_string(&lock_path)
        .unwrap_or_else(|e| panic!("cannot read {lock_path} for stamp check: {e}"));
    let mut in_package = false;
    let mut is_mollusk = false;
    let mut resolved: Option<String> = None;
    for line in lock.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            in_package = true;
            is_mollusk = false;
        } else if in_package && line == "name = \"mollusk-svm\"" {
            is_mollusk = true;
        } else if in_package && is_mollusk && line.starts_with("version = ") {
            resolved = Some(line.trim_start_matches("version = ").trim_matches('"').to_string());
            break;
        }
    }
    let resolved = resolved.expect("mollusk-svm entry missing from harness Cargo.lock");
    assert_eq!(
        resolved, MOLLUSK_SVM_VERSION,
        "mollusk stamp drift: would stamp {MOLLUSK_SVM_VERSION} but lock resolves {resolved}"
    );
}

fn disc(name: &str) -> Vec<u8> {
    Sha256::digest(format!("global:{name}").as_bytes())[..8].to_vec()
}

fn pda(seeds: &[&[u8]], program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(seeds, program_id)
}

fn tag(s: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..s.len()].copy_from_slice(s);
    out
}

fn u64le(v: u64) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

fn u16le(v: u16) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// Hand-serialized `StoredGovAction` for `GrantRole { who: agent,
/// role: Guardian }`, copied from the golden harness (1524 bytes).
fn stored_grant_role_action(agent: [u8; 32]) -> Vec<u8> {
    let mut b = Vec::with_capacity(1524);
    b.push(0u8);
    b.extend_from_slice(&[0u8; 32]);
    b.extend_from_slice(&agent);
    b.push(0u8);
    b.extend_from_slice(&0u16.to_le_bytes());
    b.extend_from_slice(&0u16.to_le_bytes());
    b.extend_from_slice(&[0u8; 32]);
    b.extend_from_slice(&0u64.to_le_bytes());
    b.push(0u8);
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u64.to_le_bytes());
    b.extend_from_slice(&0u64.to_le_bytes());
    b.extend_from_slice(&0u64.to_le_bytes());
    b.push(0u8);
    b.extend_from_slice(&0u64.to_le_bytes());
    b.extend_from_slice(&0u64.to_le_bytes());
    b.extend_from_slice(&0u64.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&[0u8; 32 * 16]);
    b.push(0u8);
    b.extend_from_slice(&[0u8; 2 * 16]);
    b.push(0u8);
    b.extend_from_slice(&[0u8; 32 * 8]);
    b.extend_from_slice(&[0u8; 8 * 8]);
    b.push(0u8);
    b.extend_from_slice(&[0u8; 32 * 15]);
    b.push(0u8);
    assert_eq!(b.len(), 1524, "StoredGovAction layout must total 1524 bytes");
    b
}

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

fn find<'a>(accounts: &'a [(Pubkey, Account)], key: &Pubkey) -> &'a Account {
    accounts
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, a)| a)
        .unwrap_or_else(|| panic!("account {key} missing from resulting accounts"))
}

fn u64_at(data: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(data[off..off + 8].try_into().unwrap())
}

struct Bundle {
    record: Pubkey,
    obs: Pubkey,
    signal: Pubkey,
    plan: Pubkey,
}

impl Bundle {
    fn metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.record, false),
            AccountMeta::new_readonly(self.obs, false),
            AccountMeta::new_readonly(self.signal, false),
            AccountMeta::new_readonly(self.plan, false),
        ]
    }
}

/// Drive one ordinary bundle to COMMITTED and return its declared hash.
/// `i` is the bundle index (0-based): parent (i, declared_{i-1}),
/// ingest clock 200*(i+1), commit clock 200*(i+2). `priors` are the
/// already-committed bundles, in head order.
#[allow(clippy::too_many_arguments)]
fn drive_bundle(
    mollusk: &Mollusk,
    accounts: Vec<(Pubkey, Account)>,
    program_id: &Pubkey,
    system_id: &Pubkey,
    kernel_state: &Pubkey,
    payer: &Pubkey,
    seat1: &Pubkey,
    observer: &Pubkey,
    signaler: &Pubkey,
    i: usize,
    priors: &[Bundle],
) -> (Vec<(Pubkey, Account)>, Bundle, [u8; 32]) {
    let tid = [0xB0 + i as u8; 32];
    let obs_id = [0xC0 + i as u8; 32];
    let declared = [0xE0 + i as u8; 32];
    let (record, _) = pda(&[b"transition", tid.as_ref()], program_id);
    let (obs, _) = pda(&[b"observation", obs_id.as_ref()], program_id);
    let round0_le = 0u64.to_le_bytes();
    let (signal, _) = pda(
        &[b"signal", tid.as_ref(), signaler.as_ref(), &round0_le],
        program_id,
    );
    let (plan, _) = pda(&[b"plan", tid.as_ref(), signaler.as_ref()], program_id);
    let bundle = Bundle {
        record,
        obs,
        signal,
        plan,
    };
    let mut accounts = accounts;
    // Fresh accounts for this bundle (mollusk keeps unknown keys empty,
    // but threading explicitly keeps the set complete).
    let empty = Account {
        lamports: 0,
        data: vec![],
        owner: *system_id,
        executable: false,
        rent_epoch: 0,
    };
    for k in [record, obs, signal, plan] {
        if !accounts.iter().any(|(a, _)| a == &k) {
            accounts.push((k, empty.clone()));
        }
    }
    let prior_metas: Vec<AccountMeta> =
        priors.iter().flat_map(|b| b.metas()).collect();
    let clock = 200 * (i as u64 + 1);
    let parent_index = i as u64;
    let parent_hash = if i == 0 {
        ZERO_HASH
    } else {
        [0xE0 + (i as u8 - 1); 32]
    };
    let stage_ix = |name: &str| -> Vec<u8> {
        let mut d = disc(name);
        d.extend_from_slice(&tid);
        d.extend_from_slice(&obs_id);
        d.extend_from_slice(&(i as u32).to_le_bytes());
        d
    };
    let stage_accounts = |ks_mut: bool| -> Vec<AccountMeta> {
        let mut m = vec![
            if ks_mut {
                AccountMeta::new(*kernel_state, false)
            } else {
                AccountMeta::new_readonly(*kernel_state, false)
            },
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new_readonly(*payer, true),
        ];
        m.extend(prior_metas.clone());
        m
    };

    // register_transition at the current head.
    let mut data = disc("register_transition");
    data.extend_from_slice(&tid);
    data.extend_from_slice(&TT_TYPE);
    data.extend_from_slice(&u64le(parent_index));
    data.extend_from_slice(&parent_hash);
    data.extend_from_slice(&declared);
    data.extend_from_slice(&obs_id);
    data.extend_from_slice(&u64le(10_100));
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new_readonly(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new(*seat1, true),
            AccountMeta::new_readonly(*system_id, false),
        ],
    );
    let (next, _) = run(mollusk, &format!("register_bundle{i}"), &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 0, "bundle {i} must start REGISTERED");

    // ingest_observation at the ingest clock.
    let mut data = disc("ingest_observation");
    data.extend_from_slice(&obs_id);
    data.extend_from_slice(&u64le(clock));
    data.extend_from_slice(&u64le(9100));
    data.extend_from_slice(&u64le(900_000));
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new_readonly(*kernel_state, false),
            AccountMeta::new(obs, false),
            AccountMeta::new(*observer, true),
            AccountMeta::new_readonly(*system_id, false),
        ],
    );
    let (next, _) = run(mollusk, &format!("ingest_bundle{i}"), &ix, &accounts);
    accounts = next;

    // replay -> EVALUATED -> ELIGIBLE.
    let ix = Instruction::new_with_bytes(
        *program_id,
        &stage_ix("replay_transition"),
        vec![
            AccountMeta::new_readonly(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new(*seat1, true),
        ]
        .into_iter()
        .chain(prior_metas.clone())
        .collect(),
    );
    let (next, _) = run(mollusk, &format!("replay_bundle{i}"), &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 1, "bundle {i} must advance to KERNEL_REGISTERED");
    let ix = Instruction::new_with_bytes(
        *program_id,
        &stage_ix("evaluate_transition"),
        stage_accounts(false),
    );
    let (next, _) = run(mollusk, &format!("evaluate_bundle{i}"), &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 2, "bundle {i} must advance to EVALUATED");
    let ix = Instruction::new_with_bytes(
        *program_id,
        &stage_ix("mark_eligible"),
        stage_accounts(true),
    );
    let (next, _) = run(mollusk, &format!("mark_eligible_bundle{i}"), &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 3, "bundle {i} must advance to ELIGIBLE");

    // submit_signal round 0 by ag(2).
    let mut data = disc("submit_signal");
    data.extend_from_slice(&tid);
    data.extend_from_slice(&obs_id);
    data.extend_from_slice(&u64le(0));
    data.extend_from_slice(&u64le(SCALE));
    data.extend_from_slice(&u64le(SCALE));
    data.push(1u8);
    data.extend_from_slice(&(i as u32).to_le_bytes());
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new_readonly(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new(signal, false),
            AccountMeta::new(*signaler, true),
            AccountMeta::new_readonly(*system_id, false),
        ]
        .into_iter()
        .chain(prior_metas.clone())
        .collect(),
    );
    let (next, _) = run(mollusk, &format!("signal_bundle{i}"), &ix, &accounts);
    accounts = next;

    // advance to the consensus deadline, select, open planning.
    let mut data = disc("advance_clock");
    data.extend_from_slice(&u64le(clock + 100));
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new(*kernel_state, false),
            AccountMeta::new_readonly(*payer, true),
        ],
    );
    let (next, _) = run(mollusk, &format!("advance_bundle{i}_select"), &ix, &accounts);
    accounts = next;
    let mut metas = stage_accounts(false);
    metas.push(AccountMeta::new_readonly(signal, false));
    let ix = Instruction::new_with_bytes(*program_id, &stage_ix("select_candidate"), metas);
    let (next, _) = run(mollusk, &format!("select_bundle{i}"), &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 4, "bundle {i} must advance to SELECTED");
    let mut metas = vec![
        AccountMeta::new_readonly(*kernel_state, false),
        AccountMeta::new(record, false),
        AccountMeta::new_readonly(obs, false),
        AccountMeta::new_readonly(*seat1, true),
    ];
    metas.extend(prior_metas.clone());
    metas.push(AccountMeta::new_readonly(signal, false));
    let ix = Instruction::new_with_bytes(*program_id, &stage_ix("open_planning"), metas);
    let (next, _) = run(mollusk, &format!("open_bundle{i}"), &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 5, "bundle {i} must advance to PLANNING");

    // submit_plan: tranche 10 SCALE + lending 5 SCALE, expiry clock+300.
    let mut data = disc("submit_plan");
    data.extend_from_slice(&tid);
    data.extend_from_slice(&obs_id);
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&u64le(10 * SCALE));
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&u64le(5 * SCALE));
    data.extend_from_slice(&u64le(clock + 300));
    data.extend_from_slice(&(i as u32).to_le_bytes());
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new_readonly(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new(plan, false),
            AccountMeta::new(*signaler, true),
            AccountMeta::new_readonly(*system_id, false),
        ]
        .into_iter()
        .chain(prior_metas.clone())
        .chain([AccountMeta::new_readonly(signal, false)])
        .collect(),
    );
    let (next, _) = run(mollusk, &format!("plan_bundle{i}"), &ix, &accounts);
    accounts = next;

    // advance to the planning deadline, reconcile/validate/declare/commit.
    let mut data = disc("advance_clock");
    data.extend_from_slice(&u64le(clock + 200));
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new(*kernel_state, false),
            AccountMeta::new_readonly(*payer, true),
        ],
    );
    let (next, _) = run(mollusk, &format!("advance_bundle{i}_commit"), &ix, &accounts);
    accounts = next;
    for name in ["reconcile_plans", "validate_transition", "declare_transition"] {
        let mut metas = stage_accounts(false);
        metas.push(AccountMeta::new_readonly(signal, false));
        metas.push(AccountMeta::new_readonly(plan, false));
        let ix = Instruction::new_with_bytes(*program_id, &stage_ix(name), metas);
        let (next, _) = run(mollusk, &format!("{name}_bundle{i}"), &ix, &accounts);
        accounts = next;
    }
    let mut metas = stage_accounts(true);
    metas.push(AccountMeta::new_readonly(signal, false));
    metas.push(AccountMeta::new_readonly(plan, false));
    let ix = Instruction::new_with_bytes(*program_id, &stage_ix("commit_transition"), metas);
    let (next, _) = run(mollusk, &format!("commit_bundle{i}"), &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 10, "bundle {i} must advance to COMMITTED");
    let ks = find(&accounts, kernel_state);
    assert_eq!(u64_at(&ks.data, 9), (i as u64) + 1, "bundle {i} commit must move the head");
    assert_eq!(&ks.data[17..49], &declared, "bundle {i} commit must move the head to its declared hash");
    (accounts, bundle, declared)
}

/// Drive the governance tail at the moved head with `n` prior bundles
/// and return the `record_outcome_governance` CU. `accounts` threads
/// the mollusk state; prior bundles stay COMMITTED throughout.
#[allow(clippy::too_many_arguments)]
fn drive_tail_outcome_cu(
    mollusk: &Mollusk,
    accounts: Vec<(Pubkey, Account)>,
    program_id: &Pubkey,
    system_id: &Pubkey,
    kernel_state: &Pubkey,
    payer: &Pubkey,
    seat1: &Pubkey,
    observer: &Pubkey,
    signaler: &Pubkey,
    ag3: &Pubkey,
    recorder: &Pubkey,
    n: usize,
    priors: &[Bundle],
    parent_hash: [u8; 32],
) -> (Vec<(Pubkey, Account)>, u64) {
    let mut accounts = accounts;
    let (record, _) = pda(&[b"transition", TAIL_TID.as_ref()], program_id);
    let (obs, _) = pda(&[b"observation", TAIL_OBS.as_ref()], program_id);
    let (action_account, _) = pda(
        &[b"gov-action", TAIL_TID.as_ref(), seat1.as_ref()],
        program_id,
    );
    let (approval1, _) = pda(
        &[b"gov-approval", TAIL_TID.as_ref(), seat1.as_ref()],
        program_id,
    );
    let (approval2, _) = pda(
        &[b"gov-approval", TAIL_TID.as_ref(), signaler.as_ref()],
        program_id,
    );
    let (approval3, _) = pda(
        &[b"gov-approval", TAIL_TID.as_ref(), ag3.as_ref()],
        program_id,
    );
    let round0_le = 0u64.to_le_bytes();
    let (signal, _) = pda(
        &[b"signal", TAIL_TID.as_ref(), signaler.as_ref(), &round0_le],
        program_id,
    );
    let (plan, _) = pda(&[b"plan", TAIL_TID.as_ref(), signaler.as_ref()], program_id);
    let (evidence, _) = pda(&[b"evidence", TAIL_TID.as_ref()], program_id);
    let (rep, _) = pda(&[b"reputation", signaler.as_ref()], program_id);
    let empty = Account {
        lamports: 0,
        data: vec![],
        owner: *system_id,
        executable: false,
        rent_epoch: 0,
    };
    for k in [
        record, obs, action_account, approval1, approval2, approval3, signal, plan,
        evidence, rep,
    ] {
        if !accounts.iter().any(|(a, _)| a == &k) {
            accounts.push((k, empty.clone()));
        }
    }
    let prior_metas: Vec<AccountMeta> =
        priors.iter().flat_map(|b| b.metas()).collect();
    let approvals = vec![
        AccountMeta::new_readonly(approval1, false),
        AccountMeta::new_readonly(approval2, false),
        AccountMeta::new_readonly(approval3, false),
    ];
    let tail_stage_ix = |name: &str| -> Vec<u8> {
        let mut d = disc(name);
        d.extend_from_slice(&TAIL_TID);
        d.extend_from_slice(&TAIL_OBS);
        d.extend_from_slice(&(n as u32).to_le_bytes());
        d
    };
    let clock = 200 * (n as u64 + 1);

    // Account-carried action: GrantRole Guardian for the proposer.
    let action_bytes = stored_grant_role_action([0x99; 32]);
    let expected_hash: [u8; 32] = Sha256::digest(&action_bytes).into();
    let mut data = disc("init_gov_action");
    data.extend_from_slice(&TAIL_TID);
    data.extend_from_slice(&expected_hash);
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new(action_account, false),
            AccountMeta::new(*seat1, true),
            AccountMeta::new_readonly(*system_id, false),
        ],
    );
    let (next, _) = run(mollusk, "init_gov_action_tail", &ix, &accounts);
    accounts = next;
    for (offset, chunk) in [
        (0u16, &action_bytes[0..512]),
        (512u16, &action_bytes[512..1024]),
        (1024u16, &action_bytes[1024..1524]),
    ] {
        let mut data = disc("append_gov_action_chunk");
        data.extend_from_slice(&TAIL_TID);
        data.extend_from_slice(&u16le(offset));
        data.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        data.extend_from_slice(chunk);
        let ix = Instruction::new_with_bytes(
            *program_id,
            &data,
            vec![
                AccountMeta::new(action_account, false),
                AccountMeta::new(*seat1, true),
            ],
        );
        let (next, _) = run(
            mollusk,
            &format!("append_gov_action_tail@{offset}"),
            &ix,
            &accounts,
        );
        accounts = next;
    }
    let mut data = disc("finalize_gov_action");
    data.extend_from_slice(&TAIL_TID);
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new(action_account, false),
            AccountMeta::new(*seat1, true),
        ],
    );
    let (next, _) = run(mollusk, "finalize_gov_action_tail", &ix, &accounts);
    accounts = next;

    // register_governance_transition at the moved head.
    let gov_type = tag(b"GOV_ADJUSTABLE");
    let mut data = disc("register_governance_transition");
    data.extend_from_slice(&TAIL_TID);
    data.extend_from_slice(&gov_type);
    data.extend_from_slice(&u64le(n as u64));
    data.extend_from_slice(&parent_hash);
    data.extend_from_slice(&TAIL_DECLARED);
    data.extend_from_slice(&TAIL_OBS);
    data.extend_from_slice(&u64le(10_100));
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new_readonly(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(action_account, false),
            AccountMeta::new(*seat1, true),
            AccountMeta::new_readonly(*system_id, false),
        ],
    );
    let (next, _) = run(mollusk, "register_governance_tail", &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 0, "tail must start REGISTERED");

    // approve_governance, one per council seat.
    for (seat, approval) in [
        (*seat1, approval1),
        (*signaler, approval2),
        (*ag3, approval3),
    ] {
        let mut data = disc("approve_governance");
        data.extend_from_slice(&TAIL_TID);
        let ix = Instruction::new_with_bytes(
            *program_id,
            &data,
            vec![
                AccountMeta::new_readonly(*kernel_state, false),
                AccountMeta::new(record, false),
                AccountMeta::new_readonly(*seat1, false),
                AccountMeta::new_readonly(action_account, false),
                AccountMeta::new(approval, false),
                AccountMeta::new(seat, true),
                AccountMeta::new_readonly(*system_id, false),
            ],
        );
        let (next, _) = run(mollusk, "approve_governance_tail", &ix, &accounts);
        accounts = next;
    }
    assert_eq!(
        u64_at(&find(&accounts, &record).data, 257),
        3,
        "tail approval count must reach 3 (unanimous council)"
    );

    // ingest_observation at the tail clock.
    let mut data = disc("ingest_observation");
    data.extend_from_slice(&TAIL_OBS);
    data.extend_from_slice(&u64le(clock));
    data.extend_from_slice(&u64le(10_100));
    data.extend_from_slice(&u64le(900_000));
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new_readonly(*kernel_state, false),
            AccountMeta::new(obs, false),
            AccountMeta::new(*observer, true),
            AccountMeta::new_readonly(*system_id, false),
        ],
    );
    let (next, _) = run(mollusk, "ingest_observation_tail", &ix, &accounts);
    accounts = next;

    // replay / evaluate / mark_eligible with the stage tail.
    let stage_tail: Vec<AccountMeta> = approvals
        .iter()
        .cloned()
        .chain([AccountMeta::new_readonly(action_account, false)])
        .collect();
    let ix = Instruction::new_with_bytes(
        *program_id,
        &tail_stage_ix("replay_governance"),
        vec![
            AccountMeta::new_readonly(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new(*seat1, true),
        ]
        .into_iter()
        .chain(prior_metas.clone())
        .chain(stage_tail.clone())
        .collect(),
    );
    let (next, _) = run(mollusk, "replay_governance_tail", &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 1, "tail must advance to KERNEL_REGISTERED");
    let ix = Instruction::new_with_bytes(
        *program_id,
        &tail_stage_ix("evaluate_governance"),
        vec![
            AccountMeta::new_readonly(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new_readonly(*payer, true),
        ]
        .into_iter()
        .chain(prior_metas.clone())
        .chain(stage_tail.clone())
        .collect(),
    );
    let (next, _) = run(mollusk, "evaluate_governance_tail", &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 2, "tail must advance to EVALUATED");
    let ix = Instruction::new_with_bytes(
        *program_id,
        &tail_stage_ix("mark_eligible_governance"),
        vec![
            AccountMeta::new(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new_readonly(*payer, true),
        ]
        .into_iter()
        .chain(prior_metas.clone())
        .chain(stage_tail.clone())
        .collect(),
    );
    let (next, _) = run(mollusk, "mark_eligible_governance_tail", &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 3, "tail must advance to ELIGIBLE");

    // submit_signal_governance round 0 by ag(2).
    let mut data = disc("submit_signal_governance");
    data.extend_from_slice(&TAIL_TID);
    data.extend_from_slice(&TAIL_OBS);
    data.extend_from_slice(&u64le(0));
    data.extend_from_slice(&u64le(SCALE));
    data.extend_from_slice(&u64le(SCALE));
    data.push(1u8);
    data.extend_from_slice(&(n as u32).to_le_bytes());
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new_readonly(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new(signal, false),
            AccountMeta::new(*signaler, true),
            AccountMeta::new_readonly(*system_id, false),
        ]
        .into_iter()
        .chain(prior_metas.clone())
        .chain(stage_tail.clone())
        .collect(),
    );
    let (next, _) = run(mollusk, "submit_signal_governance_tail", &ix, &accounts);
    accounts = next;

    // advance to the consensus deadline, select, open planning.
    let mut data = disc("advance_clock");
    data.extend_from_slice(&u64le(clock + 100));
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new(*kernel_state, false),
            AccountMeta::new_readonly(*payer, true),
        ],
    );
    let (next, _) = run(mollusk, "advance_tail_select", &ix, &accounts);
    accounts = next;
    let ix = Instruction::new_with_bytes(
        *program_id,
        &tail_stage_ix("select_candidate_governance"),
        vec![
            AccountMeta::new_readonly(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new_readonly(*payer, true),
        ]
        .into_iter()
        .chain(prior_metas.clone())
        .chain(approvals.clone())
        .chain([
            AccountMeta::new_readonly(signal, false),
            AccountMeta::new_readonly(action_account, false),
        ])
        .collect(),
    );
    let (next, _) = run(mollusk, "select_candidate_governance_tail", &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 4, "tail must advance to SELECTED");
    let ix = Instruction::new_with_bytes(
        *program_id,
        &tail_stage_ix("open_planning_governance"),
        vec![
            AccountMeta::new_readonly(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new_readonly(*seat1, true),
        ]
        .into_iter()
        .chain(prior_metas.clone())
        .chain(approvals.clone())
        .chain([
            AccountMeta::new_readonly(signal, false),
            AccountMeta::new_readonly(action_account, false),
        ])
        .collect(),
    );
    let (next, _) = run(mollusk, "open_planning_governance_tail", &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 5, "tail must advance to PLANNING");

    // submit_plan_governance: empty claims, expiry clock+300.
    let mut data = disc("submit_plan_governance");
    data.extend_from_slice(&TAIL_TID);
    data.extend_from_slice(&TAIL_OBS);
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&u64le(clock + 300));
    data.extend_from_slice(&(n as u32).to_le_bytes());
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new_readonly(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new(plan, false),
            AccountMeta::new(*signaler, true),
            AccountMeta::new_readonly(*system_id, false),
        ]
        .into_iter()
        .chain(prior_metas.clone())
        .chain(approvals.clone())
        .chain([
            AccountMeta::new_readonly(signal, false),
            AccountMeta::new_readonly(action_account, false),
        ])
        .collect(),
    );
    let (next, _) = run(mollusk, "submit_plan_governance_tail", &ix, &accounts);
    accounts = next;

    // advance to the planning deadline, reconcile/validate/declare.
    let mut data = disc("advance_clock");
    data.extend_from_slice(&u64le(clock + 200));
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new(*kernel_state, false),
            AccountMeta::new_readonly(*payer, true),
        ],
    );
    let (next, _) = run(mollusk, "advance_tail_commit", &ix, &accounts);
    accounts = next;
    for name in [
        "reconcile_plans_governance",
        "validate_transition_governance",
        "declare_transition_governance",
    ] {
        let ix = Instruction::new_with_bytes(
            *program_id,
            &tail_stage_ix(name),
            vec![
                AccountMeta::new_readonly(*kernel_state, false),
                AccountMeta::new(record, false),
                AccountMeta::new_readonly(obs, false),
                AccountMeta::new_readonly(*payer, true),
            ]
            .into_iter()
            .chain(prior_metas.clone())
            .chain(approvals.clone())
            .chain([
                AccountMeta::new_readonly(signal, false),
                AccountMeta::new_readonly(plan, false),
                AccountMeta::new_readonly(action_account, false),
            ])
            .collect(),
        );
        let (next, _) = run(mollusk, &format!("{name}_tail"), &ix, &accounts);
        accounts = next;
    }

    // commit_governance -> COMMITTED; the head moves to the tail hash.
    let ix = Instruction::new_with_bytes(
        *program_id,
        &tail_stage_ix("commit_governance"),
        vec![
            AccountMeta::new(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new_readonly(*seat1, false),
            AccountMeta::new_readonly(action_account, false),
            AccountMeta::new_readonly(*payer, true),
        ]
        .into_iter()
        .chain(prior_metas.clone())
        .chain([
            AccountMeta::new_readonly(signal, false),
            AccountMeta::new_readonly(plan, false),
        ])
        .chain(approvals.clone())
        .collect(),
    );
    let (next, _) = run(mollusk, "commit_governance_tail", &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 10, "tail must advance to COMMITTED");

    // init_reputation for ag(2) so the outcome can attribute it.
    let mut data = disc("init_reputation");
    data.extend_from_slice(signaler.as_ref());
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new(*kernel_state, false),
            AccountMeta::new(rep, false),
            AccountMeta::new(*payer, true),
            AccountMeta::new_readonly(*system_id, false),
        ],
    );
    let (next, _) = run(mollusk, "init_reputation_tail", &ix, &accounts);
    accounts = next;

    // record_settlement_evidence at the tail declared hash.
    let mut data = disc("record_settlement_evidence");
    data.extend_from_slice(&TAIL_TID);
    data.extend_from_slice(&TAIL_OBS);
    data.extend_from_slice(&TAIL_DECLARED);
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new_readonly(*kernel_state, false),
            AccountMeta::new_readonly(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new(evidence, false),
            AccountMeta::new(*observer, true),
            AccountMeta::new_readonly(*system_id, false),
        ],
    );
    let (next, _) = run(mollusk, "record_settlement_evidence_tail", &ix, &accounts);
    accounts = next;

    // settle_governance_transition -> SETTLED.
    let ix = Instruction::new_with_bytes(
        *program_id,
        &tail_stage_ix("settle_governance_transition"),
        vec![
            AccountMeta::new(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new_readonly(*seat1, false),
            AccountMeta::new_readonly(action_account, false),
            AccountMeta::new_readonly(evidence, false),
            AccountMeta::new_readonly(*payer, true),
        ]
        .into_iter()
        .chain(prior_metas.clone())
        .chain([
            AccountMeta::new_readonly(signal, false),
            AccountMeta::new_readonly(plan, false),
        ])
        .chain(approvals.clone())
        .collect(),
    );
    let (next, _) = run(mollusk, "settle_governance_tail", &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 11, "tail must advance to SETTLED");

    // record_outcome_governance by RECORDER, attribution (ag(2), +10).
    // THIS is the measured instruction.
    let mut data = disc("record_outcome_governance");
    data.extend_from_slice(&TAIL_TID);
    data.extend_from_slice(&TAIL_OBS);
    data.extend_from_slice(&(n as u32).to_le_bytes());
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(signaler.as_ref());
    data.extend_from_slice(&10i64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        *program_id,
        &data,
        vec![
            AccountMeta::new(*kernel_state, false),
            AccountMeta::new(record, false),
            AccountMeta::new_readonly(obs, false),
            AccountMeta::new_readonly(*seat1, false),
            AccountMeta::new_readonly(action_account, false),
            AccountMeta::new_readonly(evidence, false),
            AccountMeta::new_readonly(*recorder, true),
        ]
        .into_iter()
        .chain(prior_metas.clone())
        .chain([
            AccountMeta::new_readonly(signal, false),
            AccountMeta::new_readonly(plan, false),
        ])
        .chain(approvals.clone())
        .chain([AccountMeta::new(rep, false)])
        .collect(),
    );
    let (next, cu) = run(mollusk, "record_outcome_governance_tail", &ix, &accounts);
    accounts = next;
    assert_eq!(find(&accounts, &record).data[216], 12, "tail must advance to EVALUATED_OUTCOME");
    assert_eq!(
        u64_at(&find(&accounts, &rep).data, 40),
        50_000_010,
        "exact tail outcome credit must move ag(2) reputation to 50,000,010"
    );
    (accounts, cu)
}

fn measure_at_priors(n: usize, so_path: &str, so_sha256: &str) -> u64 {
    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let system_id = Pubkey::from_str(SYSTEM_ID).unwrap();
    let mut mollusk = Mollusk::new(&program_id, "program_v04");
    mollusk.compute_budget.heap_size = 256 * 1024;
    let _ = so_path;
    let _ = so_sha256;

    let payer = Pubkey::new_from_array([0x50; 32]);
    let seat1 = Pubkey::new_from_array([1; 32]);
    let observer = Pubkey::new_from_array([0xA1; 32]);
    let signaler = Pubkey::new_from_array([2; 32]);
    let ag3 = Pubkey::new_from_array([3; 32]);
    let recorder = Pubkey::new_from_array(RECORDER);

    let (kernel_state, _) = pda(&[b"kernel-state"], &program_id);
    let empty = Account {
        lamports: 0,
        data: vec![],
        owner: system_id,
        executable: false,
        rent_epoch: 0,
    };
    let funded = Account {
        lamports: 10_000_000_000,
        data: vec![],
        owner: system_id,
        executable: false,
        rent_epoch: 0,
    };
    let mut accounts: Vec<(Pubkey, Account)> = vec![
        keyed_account_for_system_program(),
        mollusk.sysvars.keyed_account_for_clock_sysvar(),
        (kernel_state, empty.clone()),
        (payer, funded.clone()),
        (seat1, funded.clone()),
        (observer, funded.clone()),
        (signaler, funded.clone()),
        (ag3, funded.clone()),
        (recorder, funded.clone()),
    ];

    // Genesis: initialize + seal (clock 100), advance to 200.
    let ix = Instruction::new_with_bytes(
        program_id,
        &disc("initialize_kernel"),
        vec![
            AccountMeta::new(kernel_state, false),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, _) = run(&mollusk, "initialize_kernel", &ix, &accounts);
    accounts = next;
    let ix = Instruction::new_with_bytes(
        program_id,
        &disc("seal_kernel"),
        vec![
            AccountMeta::new(kernel_state, false),
            AccountMeta::new_readonly(payer, true),
        ],
    );
    let (next, _) = run(&mollusk, "seal_kernel", &ix, &accounts);
    accounts = next;
    let mut data = disc("advance_clock");
    data.extend_from_slice(&u64le(200));
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(kernel_state, false),
            AccountMeta::new_readonly(payer, true),
        ],
    );
    let (next, _) = run(&mollusk, "advance_clock_200", &ix, &accounts);
    accounts = next;

    // N ordinary bundles to COMMITTED.
    let mut priors: Vec<Bundle> = Vec::new();
    let mut parent_hash = ZERO_HASH;
    for i in 0..n {
        let (next, bundle, declared) = drive_bundle(
            &mollusk,
            accounts,
            &program_id,
            &system_id,
            &kernel_state,
            &payer,
            &seat1,
            &observer,
            &signaler,
            i,
            &priors,
        );
        accounts = next;
        parent_hash = declared;
        priors.push(bundle);
    }

    let (_, cu) = drive_tail_outcome_cu(
        &mollusk,
        accounts,
        &program_id,
        &system_id,
        &kernel_state,
        &payer,
        &seat1,
        &observer,
        &signaler,
        &ag3,
        &recorder,
        n,
        &priors,
        parent_hash,
    );
    println!("CHAINLEN_CU|priors={n}|cu={cu}");
    cu
}

fn main() {
    assert_mollusk_stamp_matches_lock();
    let out_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "chainlen-cu.json".to_string());
    let so_dir = std::env::var("SBF_OUT_DIR").unwrap_or_else(|_| "target/deploy".to_string());
    let so_path = format!("{so_dir}/program_v04.so");
    let so_bytes = std::fs::read(&so_path)
        .unwrap_or_else(|e| panic!("cannot read program .so at {so_path}: {e}"));
    let so_sha256 = hex_encode(&Sha256::digest(&so_bytes));

    let mut rows = Vec::new();
    for n in [0usize, 1, 2, 3, 5] {
        let cu = measure_at_priors(n, &so_path, &so_sha256);
        rows.push(serde_json::json!({"priors": n, "cu": cu, "result": "Success"}));
    }
    let out = serde_json::json!({
        "program": "program_v04",
        "program_id": PROGRAM_ID,
        "so_sha256": so_sha256,
        "mollusk_svm": MOLLUSK_SVM_VERSION,
        "toolchain": TOOLCHAIN,
        "fixture": "chain-length CU leg: record_outcome_governance CU at 0/1/2/3/5 claims-carrying priors (two-claims ordinary bundles to COMMITTED + unanimous governance tail through settle/outcome)",
        "rows": rows,
    });
    std::fs::write(&out_path, serde_json::to_string_pretty(&out).unwrap())
        .unwrap_or_else(|e| panic!("cannot write {out_path}: {e}"));
    println!("wrote {out_path}");
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
