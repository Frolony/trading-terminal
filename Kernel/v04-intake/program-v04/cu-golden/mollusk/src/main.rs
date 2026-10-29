//! V0.4 program CU golden-test harness (mollusk-svm path), slice 1.
//!
//! Measures real compute units per instruction for `program-v04` by
//! executing the real `.so` in-process (mollusk-svm 0.16.2) — not
//! estimates, not host-test timings. Every measured instruction must
//! succeed; any failure aborts with a non-zero exit, so a golden file
//! is never written from a failed chain.
//!
//! Slice 1 chain (genesis + governance registration): `probe_kernel`,
//! `initialize_kernel`, `seal_kernel`, `advance_clock` (100 -> 200),
//! `register_transition` (ordinary), `ingest_observation`
//! (observer `[0xA1;32]`, timestamp 100, expiry 10100, confidence
//! 500,000), `register_governance_transition` (seat ag(1) = `[1;32]`,
//! `GOV_ADJUSTABLE`).
//!
//! BLOCKED, NOT MEASURED — `approve_governance` and
//! `revise_governance_approval` (slice-49 finding, recorded in the
//! golden file's `blocked` section and in the program README, not
//! hidden): under mollusk both refuse with Anchor 3005
//! (`AccountNotEnoughKeys`, attributed to `kernel_state`) before any
//! handler gate runs, despite correct account metas. Ten probes
//! proved the failure intrinsic to these two instructions under
//! mollusk, not to account state, meta order, payer identity, or
//! data size: reversed metas, a minimal account vec, a non-seat
//! payer, a fully pre-initialized valid approval record, and a
//! padded `advance_clock` (1516 bytes of data, Success) all leave
//! the 3005 unchanged, while `replay_transition` on the same
//! governance record reaches its third account (3012) as expected
//! and truncated revise data gives the args-deserialize error (102)
//! instead. The stack-frame execution check the cu-golden freeze was
//! queued to carry (approve handler 5,760 bytes, revise 5,696, both
//! over the 4,096-byte limit) is therefore NOT closed by this slice:
//! it stays queued behind resolving the 3005. A second, independent
//! design finding rides the same instructions: `approve`/`revise`
//! carry the 1524-byte `StoredGovAction` by value, so instruction
//! data alone is 1564 bytes — plus a 64-byte signature that already
//! exceeds Solana's 1232-byte transaction size limit before any
//! account key or blockhash, so no real caller could submit either
//! instruction as a single transaction as currently encoded. That
//! encoding choice (slice 33) also explains the stack-frame overages.
//! Both findings need a design decision (compact action encoding or
//! an account-carried action); neither is redesigned in this slice.
//!
//! No §2.2-style estimate exists for the v0.4 program (the design
//! doc's §2.2 estimates are for the v0.2 treasury program), so every
//! slice-1 value is recorded as BASELINE, not gated. Later slices
//! extend the chain down the stage instructions (replay, evaluate,
//! mark_eligible, submit_signal, select, open_planning, submit_plan,
//! reconcile, validate, declare, commit, and the governance stage
//! chain) once their setup chains are scripted; nothing here claims
//! those measurements.
//!
//! Keys below are throwaway local-test keys, never real secrets. The
//! council seat is ag(1) = [1;32] and the observer is [0xA1;32],
//! matching the program's pinned launch profile (identity mapping:
//! a signer's agent id IS its pubkey bytes). Mollusk does not verify
//! signatures, so these pubkeys sign in-harness exactly as a real
//! seat/observer signature would on-chain.

use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::Mollusk;
use sha2::{Digest, Sha256};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::str::FromStr;

const PROGRAM_ID: &str = "FzxnCtig4asxnCxGhjcbCDY5zkfcpfTb5GcXNL9UaGg6";
const SYSTEM_ID: &str = "11111111111111111111111111111111";
const MOLLUSK_SVM_VERSION: &str = "0.16.2";

// Pinned launch profile constants (program-v04 lib.rs, slices 7/33).
const GOV_TID: [u8; 32] = [0xD1; 32];
const ORD_TID: [u8; 32] = [0xD2; 32];
const DECLARED_HASH: [u8; 32] = [0xE1; 32];
const GOV_OBS: [u8; 32] = [0xC0; 32];
const INGEST_OBS: [u8; 32] = [0xC1; 32];
const ZERO_HASH: [u8; 32] = [0; 32];
const EXPIRES_AT: u64 = 10_100;

fn disc(name: &str) -> Vec<u8> {
    Sha256::digest(format!("global:{name}").as_bytes())[..8].to_vec()
}

fn pda(seeds: &[&[u8]], program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(seeds, program_id)
}

/// Zero-padded ASCII tag, mirroring the kernel crate's `tag()`.
fn tag(s: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..s.len()].copy_from_slice(s);
    out
}

fn u64le(v: u64) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// Runs one instruction, asserts success, prints its measured CU,
/// and returns the threaded accounts plus the measured CU.
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

fn main() {
    let out_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "cu-golden-mollusk.json".to_string());
    let so_dir = std::env::var("SBF_OUT_DIR")
        .unwrap_or_else(|_| "target/deploy".to_string());
    let so_path = format!("{so_dir}/program_v04.so");
    let so_bytes = std::fs::read(&so_path)
        .unwrap_or_else(|e| panic!("cannot read program .so at {so_path}: {e}"));
    let so_sha256 = hex_encode(&Sha256::digest(&so_bytes));

    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let system_id = Pubkey::from_str(SYSTEM_ID).unwrap();
    let mollusk = Mollusk::new(&program_id, "program_v04");

    // Throwaway local-test payers. Seat 1 and the observer are pinned
    // profile identities; the others are arbitrary signers.
    let payer = Pubkey::new_from_array([0x50; 32]);
    let seat1 = Pubkey::new_from_array([1; 32]);
    let observer = Pubkey::new_from_array([0xA1; 32]);
    let proposer = Pubkey::new_from_array([0x99; 32]);

    let (kernel_state, _) = pda(&[b"kernel-state"], &program_id);
    let (gov_record, _) = pda(&[b"transition", GOV_TID.as_ref()], &program_id);
    let (ord_record, _) = pda(&[b"transition", ORD_TID.as_ref()], &program_id);
    let (obs_record, _) = pda(&[b"observation", INGEST_OBS.as_ref()], &program_id);

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
        (gov_record, empty.clone()),
        (ord_record, empty.clone()),
        (obs_record, empty.clone()),
        (payer, funded.clone()),
        (seat1, funded.clone()),
        (observer, funded.clone()),
        (proposer, funded.clone()),
    ];

    let mut measured: Vec<serde_json::Value> = Vec::new();
    let mut push = |label: &str, cu: u64| {
        measured.push(serde_json::json!({
            "instruction": label,
            "cu": cu,
            "result": "Success",
            "note": "slice-1 baseline (no design estimate exists for the v0.4 program)",
        }));
    };

    // probe_kernel: payer only, no accounts created.
    let ix = Instruction::new_with_bytes(
        program_id,
        &disc("probe_kernel"),
        vec![AccountMeta::new_readonly(payer, true)],
    );
    let (next, cu) = run(&mollusk, "probe_kernel", &ix, &accounts);
    accounts = next;
    push("probe_kernel", cu);

    // initialize_kernel: kernel-state PDA init.
    let ix = Instruction::new_with_bytes(
        program_id,
        &disc("initialize_kernel"),
        vec![
            AccountMeta::new(kernel_state, false),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "initialize_kernel", &ix, &accounts);
    accounts = next;
    push("initialize_kernel", cu);
    // Genesis state: unsealed, head 0, clock 0.
    let ks = find(&accounts, &kernel_state);
    assert_eq!(ks.data[8], 0, "kernel state must start unsealed");
    assert_eq!(u64_at(&ks.data, 9), 0, "genesis head_index must be 0");
    assert_eq!(u64_at(&ks.data, 49), 0, "genesis clock must be 0");

    // seal_kernel: sealed flag + clock 100 + head 0.
    let ix = Instruction::new_with_bytes(
        program_id,
        &disc("seal_kernel"),
        vec![
            AccountMeta::new(kernel_state, false),
            AccountMeta::new_readonly(payer, true),
        ],
    );
    let (next, cu) = run(&mollusk, "seal_kernel", &ix, &accounts);
    accounts = next;
    push("seal_kernel", cu);
    let ks = find(&accounts, &kernel_state);
    assert_eq!(ks.data[8], 1, "seal must set sealed");
    assert_eq!(u64_at(&ks.data, 49), 100, "sealed clock must be 100");

    // advance_clock to 200 (forward-only; sealed clock 100 -> 200).
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
    let (next, cu) = run(&mollusk, "advance_clock", &ix, &accounts);
    accounts = next;
    push("advance_clock", cu);
    let ks = find(&accounts, &kernel_state);
    assert_eq!(u64_at(&ks.data, 49), 200, "clock must advance to 200");

    // register_transition (ordinary): sealed head parent (0, zero hash).
    let mut data = disc("register_transition");
    data.extend_from_slice(&ORD_TID);
    data.extend_from_slice(&tag(b"TT_PLACEHOLDER"));
    data.extend_from_slice(&u64le(0));
    data.extend_from_slice(&ZERO_HASH);
    data.extend_from_slice(&DECLARED_HASH);
    data.extend_from_slice(&GOV_OBS);
    data.extend_from_slice(&u64le(EXPIRES_AT));
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(kernel_state, false),
            AccountMeta::new(ord_record, false),
            AccountMeta::new(proposer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_transition", &ix, &accounts);
    accounts = next;
    push("register_transition", cu);
    // TransitionRecord stage byte: 8 + 32 + 32 + 8 + 32 + 32 + 32 + 8 + 32 = 216.
    let rec = find(&accounts, &ord_record);
    assert_eq!(rec.data[216], 0, "ordinary record must start REGISTERED");

    // ingest_observation: observer [0xA1;32], timestamp 100 <= clock 200,
    // expiry 10100 > 200, confidence 500_000 <= SCALE.
    let mut data = disc("ingest_observation");
    data.extend_from_slice(&INGEST_OBS);
    data.extend_from_slice(&u64le(100));
    data.extend_from_slice(&u64le(EXPIRES_AT));
    data.extend_from_slice(&u64le(500_000));
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(kernel_state, false),
            AccountMeta::new(obs_record, false),
            AccountMeta::new(observer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "ingest_observation", &ix, &accounts);
    accounts = next;
    push("ingest_observation", cu);

    // register_governance_transition: seat ag(1), GOV_ADJUSTABLE type,
    // sealed head parent (0, zero hash).
    let gov_type = tag(b"GOV_ADJUSTABLE");
    let mut data = disc("register_governance_transition");
    data.extend_from_slice(&GOV_TID);
    data.extend_from_slice(&gov_type);
    data.extend_from_slice(&u64le(0));
    data.extend_from_slice(&ZERO_HASH);
    data.extend_from_slice(&DECLARED_HASH);
    data.extend_from_slice(&GOV_OBS);
    data.extend_from_slice(&u64le(EXPIRES_AT));
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(kernel_state, false),
            AccountMeta::new(gov_record, false),
            AccountMeta::new(seat1, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_governance_transition", &ix, &accounts);
    accounts = next;
    push("register_governance_transition", cu);
    let rec = find(&accounts, &gov_record);
    assert_eq!(rec.data[216], 0, "governance record must start REGISTERED");
    // approval_count offset: stage 216 + 1 + 8 + 8 + 8 + 8 + 8 = 257.
    assert_eq!(u64_at(&rec.data, 257), 0, "approval count must start 0");

    let out = serde_json::json!({
        "program": "program_v04",
        "program_id": PROGRAM_ID,
        "so_sha256": so_sha256,
        "mollusk_svm": MOLLUSK_SVM_VERSION,
        "toolchain": "Anchor 1.2.1 / Solana 4.1.2 / platform-tools v1.54",
        "slice": "cu-golden slice 1: genesis + governance registration",
        "measured": measured,
        "blocked": [
            {
                "instruction": "approve_governance",
                "result": "Blocked",
                "error": "Anchor 3005 AccountNotEnoughKeys (kernel_state), before any handler gate",
                "note": "See main.rs header: ten probes proved the failure intrinsic under mollusk; the stack-frame execution check is NOT closed by this slice.",
            },
            {
                "instruction": "revise_governance_approval",
                "result": "Blocked",
                "error": "Anchor 3005 AccountNotEnoughKeys (kernel_state), before any handler gate",
                "note": "See main.rs header: same ten-probe evidence as approve_governance.",
            },
        ],
        "design_finding": "approve/revise carry the 1524-byte StoredGovAction by value (1564 bytes of instruction data); with a 64-byte signature that already exceeds Solana's 1232-byte transaction size limit before any account key or blockhash, so neither instruction could be submitted as a single real transaction as currently encoded. Needs a design decision (compact action encoding or account-carried action); not redesigned in this slice.",
    });
    std::fs::write(&out_path, serde_json::to_string_pretty(&out).unwrap())
        .unwrap_or_else(|e| panic!("cannot write {out_path}: {e}"));
    println!("wrote {out_path}");
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
