//! Attempt-2 enrollment scenario (enrollment slice, 2026-11-03):
//! proves, against the SBF binary, the five properties the
//! identity-only attempt-1 binary could not satisfy —
//! (a) after `initialize_kernel`, the AgentRegistry exists with
//!     authority = the initializing signer and the 11 enrollable
//!     slots pre-seeded in profile order, all unoccupied;
//! (b) `enroll_agent` by a non-authority signer refuses
//!     (NotEnrollmentAuthority, 6047);
//! (c) a FRESH keypair enrolled as OBSERVER successfully ingests
//!     the golden life observation (the identity-only binary
//!     refused any real key with NotObserver, 6009);
//! (d) a FRESH keypair enrolled as ag(1) registers + replays the
//!     golden life transition (identity refused the proposer with
//!     UnknownProposer, 6008);
//! (e) an unenrolled fresh keypair is still refused at ingest
//!     (6009 semantics preserved).
//! Plus a bonus refusal: re-enrolling an occupied agent refuses
//! AgentAlreadyEnrolled (6049) — no silent replace.
//!
//! Note on (c)/(d) keys: each role gets its OWN fresh keypair —
//! one pubkey can occupy at most one registry slot
//! (PubkeyAlreadyEnrolled), so a single key cannot be both the
//! OBSERVER and ag(1); that is the pinned design, not a scenario
//! shortcut. Keys are derived from a hash of the run instant so
//! every run uses genuinely fresh pubkeys that are not profile
//! identities. Mollusk does not verify signatures (see the golden
//! harness header); the program-side gates judged here are the
//! registry contents and the mapped resolution, which are exactly
//! what a real signature would feed on-chain.
//!
//! This binary is additive: it writes its own report
//! (cu-golden-enroll-attempt2.json) and never touches the golden
//! measurement files.

use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::Mollusk;
use sha2::{Digest, Sha256};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::str::FromStr;

const PROGRAM_ID: &str = "CesWhN1MyhEqYT2wu6m7NgPz4SigBqz7oz2BGrKRHNdj";
const SYSTEM_ID: &str = "11111111111111111111111111111111";
const MOLLUSK_SVM_VERSION: &str = "0.16.4";

const LIFE_TID: [u8; 32] = [0xD4; 32];
const LIFE_OBS: [u8; 32] = [0xC2; 32];
const TT_TYPE: [u8; 32] = [9; 32];
const DECLARED_HASH: [u8; 32] = [0xE1; 32];
const ZERO_HASH: [u8; 32] = [0; 32];
const OBSERVER: [u8; 32] = [0xA1; 32];
const AG1: [u8; 32] = [1; 32];

fn disc(name: &str) -> Vec<u8> {
    Sha256::digest(format!("global:{name}").as_bytes())[..8].to_vec()
}

fn pda(seeds: &[&[u8]], program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(seeds, program_id)
}

fn u64le(v: u64) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// A genuinely fresh pubkey for this run (hash of label + instant):
/// never a profile identity, never enrolled unless the scenario
/// enrolls it.
fn fresh_key(label: &str) -> Pubkey {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut input = b"enroll-attempt2:".to_vec();
    input.extend_from_slice(label.as_bytes());
    input.extend_from_slice(&nanos.to_le_bytes());
    Pubkey::new_from_array(Sha256::digest(&input).into())
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
    assert!(result.program_result.is_ok(), "{label} failed: {:?}", result.program_result);
    (result.resulting_accounts, result.compute_units_consumed)
}

fn run_refused(
    mollusk: &Mollusk,
    label: &str,
    ix: &Instruction,
    accounts: &[(Pubkey, Account)],
    expect_code: &str,
) -> (Vec<(Pubkey, Account)>, u64, String) {
    let result = mollusk.process_instruction(ix, accounts);
    println!(
        "{label}: program_result={:?} cu={}",
        result.program_result, result.compute_units_consumed
    );
    assert!(result.program_result.is_err(), "{label} was expected to refuse but succeeded");
    let err = format!("{:?}", result.program_result);
    assert!(
        err.contains(expect_code),
        "{label} must refuse with error {expect_code}, got {err}"
    );
    (result.resulting_accounts, result.compute_units_consumed, err)
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

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Registry slot byte offset: 41 + index * 65; occupied at +64.
fn slot_off(index: usize) -> usize {
    41 + index * 65
}

fn main() {
    // Same stamp==resolved enforcement as the golden harness.
    let lock_path = format!("{}/Cargo.lock", env!("CARGO_MANIFEST_DIR"));
    let lock = std::fs::read_to_string(&lock_path)
        .unwrap_or_else(|e| panic!("cannot read {lock_path}: {e}"));
    assert!(
        lock.contains(&format!("name = \"mollusk-svm\"\nversion = \"{MOLLUSK_SVM_VERSION}\"")),
        "mollusk stamp drift: lock does not resolve mollusk-svm {MOLLUSK_SVM_VERSION}"
    );

    let out_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "cu-golden-enroll-attempt2.json".to_string());
    let so_dir = std::env::var("SBF_OUT_DIR").unwrap_or_else(|_| "target/deploy".to_string());
    let so_path = format!("{so_dir}/program_v04.so");
    let so_bytes = std::fs::read(&so_path)
        .unwrap_or_else(|e| panic!("cannot read program .so at {so_path}: {e}"));
    let so_sha256 = hex_encode(&Sha256::digest(&so_bytes));

    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let system_id = Pubkey::from_str(SYSTEM_ID).unwrap();
    let mut mollusk = Mollusk::new(&program_id, "program_v04");
    // The shipped 256 KiB heap-frame contract (see golden harness).
    mollusk.compute_budget.heap_size = 256 * 1024;

    // The initializing payer is the enrollment authority.
    let payer = Pubkey::new_from_array([0x50; 32]);
    let non_authority = fresh_key("non-authority");
    let obs_key = fresh_key("observer");
    let ag1_key = fresh_key("ag1");
    let stranger = fresh_key("stranger");
    for k in [non_authority, obs_key, ag1_key, stranger] {
        assert!(
            ![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], [5u8; 32], [6u8; 32], OBSERVER]
                .contains(&k.to_bytes()),
            "fresh key must not collide with a profile identity"
        );
    }

    let (kernel_state, _) = pda(&[b"kernel-state"], &program_id);
    let (agent_registry, _) = pda(&[b"agent-registry"], &program_id);
    let (life_record, _) = pda(&[b"transition", LIFE_TID.as_ref()], &program_id);
    let (life_obs, _) = pda(&[b"observation", LIFE_OBS.as_ref()], &program_id);

    let empty = Account { lamports: 0, data: vec![], owner: system_id, executable: false, rent_epoch: 0 };
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
        (agent_registry, empty.clone()),
        (life_record, empty.clone()),
        (life_obs, empty.clone()),
        (payer, funded.clone()),
        (non_authority, funded.clone()),
        (obs_key, funded.clone()),
        (ag1_key, funded.clone()),
        (stranger, funded.clone()),
    ];

    let mut steps: Vec<serde_json::Value> = Vec::new();
    macro_rules! step {
        ($label:expr, $cu:expr, $result:expr, $note:expr) => {
            steps.push(serde_json::json!({
                "step": $label, "cu": $cu, "result": $result, "note": $note,
            }));
        };
    }

    // initialize_kernel: creates KernelState AND the AgentRegistry.
    let ix = Instruction::new_with_bytes(
        program_id,
        &disc("initialize_kernel"),
        vec![
            AccountMeta::new(kernel_state, false),
            AccountMeta::new(agent_registry, false),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "initialize_kernel", &ix, &accounts);
    accounts = next;
    step!("initialize_kernel", cu, "Success", "setup");
    // (a) registry exists, authority = initializing signer, 11
    // pre-seeded slots in profile order, all unoccupied.
    let reg = find(&accounts, &agent_registry);
    assert_eq!(reg.data.len(), 756, "registry must carry the full seeded layout");
    assert_eq!(reg.data[8..40], payer.to_bytes(), "(a) registry authority must be the initializing signer");
    for (i, expected) in [
        [1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], [5u8; 32], [6u8; 32],
        OBSERVER, [0xB1u8; 32], [0xB2u8; 32], [0xB3u8; 32], [0xB4u8; 32],
    ].iter().enumerate() {
        let off = slot_off(i);
        assert_eq!(&reg.data[off..off + 32], expected, "(a) slot {i} must be pre-seeded in profile order");
        assert_eq!(reg.data[off + 64], 0, "(a) slot {i} must start unoccupied");
    }
    step!("assert_registry_after_initialize", 0, "Success",
           "(a) registry exists with authority = initializing signer; 11 slots pre-seeded, none occupied");

    // seal + advance to the golden life clock (200).
    let ix = Instruction::new_with_bytes(
        program_id,
        &disc("seal_kernel"),
        vec![AccountMeta::new(kernel_state, false), AccountMeta::new_readonly(payer, true)],
    );
    let (next, cu) = run(&mollusk, "seal_kernel", &ix, &accounts);
    accounts = next;
    step!("seal_kernel", cu, "Success", "setup");
    let mut data = disc("advance_clock");
    data.extend_from_slice(&u64le(200));
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![AccountMeta::new(kernel_state, false), AccountMeta::new_readonly(payer, true)],
    );
    let (next, cu) = run(&mollusk, "advance_clock_200", &ix, &accounts);
    accounts = next;
    step!("advance_clock_200", cu, "Success", "setup: golden life clock");

    // (b) enroll by a non-authority signer refuses 6047.
    let mut data = disc("enroll_agent");
    data.extend_from_slice(&OBSERVER);
    data.extend_from_slice(obs_key.as_ref());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(agent_registry, false),
            AccountMeta::new_readonly(non_authority, true),
        ],
    );
    let (next, cu, err) = run_refused(&mollusk, "enroll_by_non_authority", &ix, &accounts, "6047");
    accounts = next;
    step!("enroll_by_non_authority", cu, "Refused",
           format!("(b) non-authority enroll refuses NotEnrollmentAuthority: {err}"));
    let reg = find(&accounts, &agent_registry);
    assert_eq!(reg.data[slot_off(6) + 64], 0, "(b) refused enroll must leave the OBSERVER slot unoccupied");

    // Enroll the fresh observer key as OBSERVER (authority signs).
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(agent_registry, false),
            AccountMeta::new_readonly(payer, true),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll_observer", &ix, &accounts);
    accounts = next;
    step!("enroll_observer", cu, "Success", "authority enrolls the fresh key as OBSERVER");
    let reg = find(&accounts, &agent_registry);
    assert_eq!(reg.data[slot_off(6) + 64], 1, "OBSERVER slot must be occupied after enroll");
    assert_eq!(reg.data[slot_off(6) + 32..slot_off(6) + 64], obs_key.to_bytes(),
               "OBSERVER slot must hold the fresh key");

    // Enroll the fresh ag(1) key as ag(1) (authority signs).
    let mut data = disc("enroll_agent");
    data.extend_from_slice(&AG1);
    data.extend_from_slice(ag1_key.as_ref());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(agent_registry, false),
            AccountMeta::new_readonly(payer, true),
        ],
    );
    let (next, cu) = run(&mollusk, "enroll_ag1", &ix, &accounts);
    accounts = next;
    step!("enroll_ag1", cu, "Success", "authority enrolls the fresh key as ag(1)");
    let reg = find(&accounts, &agent_registry);
    assert_eq!(reg.data[slot_off(0) + 64], 1, "ag(1) slot must be occupied after enroll");
    assert_eq!(reg.data[slot_off(0) + 32..slot_off(0) + 64], ag1_key.to_bytes(),
               "ag(1) slot must hold the fresh key");

    // Bonus refusal: re-enrolling the occupied OBSERVER slot
    // refuses AgentAlreadyEnrolled (6049) — no silent replace.
    let mut data = disc("enroll_agent");
    data.extend_from_slice(&OBSERVER);
    data.extend_from_slice(stranger.as_ref());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(agent_registry, false),
            AccountMeta::new_readonly(payer, true),
        ],
    );
    let (next, cu, err) = run_refused(&mollusk, "enroll_occupied_agent", &ix, &accounts, "6049");
    accounts = next;
    step!("enroll_occupied_agent", cu, "Refused",
           format!("bonus: occupied agent slot refuses AgentAlreadyEnrolled: {err}"));

    // (e) an unenrolled fresh keypair is still refused at ingest.
    let mut data = disc("ingest_observation");
    data.extend_from_slice(&LIFE_OBS);
    data.extend_from_slice(&u64le(200));
    data.extend_from_slice(&u64le(9100));
    data.extend_from_slice(&u64le(900_000));
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(kernel_state, false),
            AccountMeta::new_readonly(agent_registry, false),
            AccountMeta::new(life_obs, false),
            AccountMeta::new(stranger, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu, err) = run_refused(&mollusk, "ingest_unenrolled", &ix, &accounts, "6009");
    accounts = next;
    step!("ingest_unenrolled", cu, "Refused",
           format!("(e) unenrolled fresh key still refuses NotObserver: {err}"));

    // (c) the enrolled fresh OBSERVER key ingests the golden life
    // observation (ts 200 == clock, expiry 9100, confidence 900,000).
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(kernel_state, false),
            AccountMeta::new_readonly(agent_registry, false),
            AccountMeta::new(life_obs, false),
            AccountMeta::new(obs_key, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "ingest_enrolled_observer", &ix, &accounts);
    accounts = next;
    step!("ingest_enrolled_observer", cu, "Success",
           "(c) enrolled fresh OBSERVER key ingests the golden observation (identity-only binary refused 6009)");
    let ob = find(&accounts, &life_obs);
    assert_eq!(ob.data[64..96], obs_key.to_bytes(), "(c) observation must name the fresh observer key");
    assert_eq!(u64_at(&ob.data, 40), 200, "(c) observation timestamp must be 200");

    // (d) the enrolled fresh ag(1) key registers the golden life
    // transition (TT type, parent (0, zero), declared [0xE1;32],
    // observation LIFE_OBS, expires 10100).
    let mut data = disc("register_transition");
    data.extend_from_slice(&LIFE_TID);
    data.extend_from_slice(&TT_TYPE);
    data.extend_from_slice(&u64le(0));
    data.extend_from_slice(&ZERO_HASH);
    data.extend_from_slice(&DECLARED_HASH);
    data.extend_from_slice(&LIFE_OBS);
    data.extend_from_slice(&u64le(10_100));
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(kernel_state, false),
            AccountMeta::new(life_record, false),
            AccountMeta::new(ag1_key, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_transition_enrolled_ag1", &ix, &accounts);
    accounts = next;
    step!("register_transition_enrolled_ag1", cu, "Success",
           "(d) enrolled fresh ag(1) key registers the golden life transition");
    let rec = find(&accounts, &life_record);
    assert_eq!(rec.data[184..216], ag1_key.to_bytes(), "(d) record proposer must be the fresh ag(1) key");
    assert_eq!(rec.data[216], 0, "(d) record must start REGISTERED");

    // (d) replay: the stored proposer (the fresh key) resolves
    // through the registry to ag(1); the stored observer resolves
    // to OBSERVER. Identity refused this with UnknownProposer.
    let mut data = disc("replay_transition");
    data.extend_from_slice(&LIFE_TID);
    data.extend_from_slice(&LIFE_OBS);
    data.extend_from_slice(&0u32.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(kernel_state, false),
            AccountMeta::new_readonly(agent_registry, false),
            AccountMeta::new(life_record, false),
            AccountMeta::new_readonly(life_obs, false),
            AccountMeta::new_readonly(payer, true),
        ],
    );
    let (next, cu) = run(&mollusk, "replay_transition_enrolled", &ix, &accounts);
    accounts = next;
    step!("replay_transition_enrolled", cu, "Success",
           "(d) replay succeeds for the enrolled proposer + observer (identity refused 6008/6009)");
    assert_eq!(find(&accounts, &life_record).data[216], 1,
               "(d) record must advance to KERNEL_REGISTERED");

    let out = serde_json::json!({
        "scenario": "enroll-attempt2",
        "program": "program_v04",
        "program_id": PROGRAM_ID,
        "so_sha256": so_sha256,
        "mollusk_svm": MOLLUSK_SVM_VERSION,
        "fresh_keys": {
            "observer": obs_key.to_string(),
            "ag1": ag1_key.to_string(),
            "stranger_unenrolled": stranger.to_string(),
            "non_authority": non_authority.to_string(),
        },
        "steps": steps,
        "properties_proven": ["(a) registry authority + seeded slots", "(b) non-authority enroll refused 6047",
            "(c) enrolled OBSERVER ingests golden observation", "(d) enrolled ag(1) registers + replays golden life transition",
            "(e) unenrolled fresh key still refused 6009", "bonus: occupied agent re-enroll refused 6049"],
    });
    std::fs::write(&out_path, serde_json::to_string_pretty(&out).unwrap())
        .unwrap_or_else(|e| panic!("cannot write {out_path}: {e}"));
    println!("wrote {out_path}");
}
