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
//! instead. A second, independent design finding rides the same
//! instructions: `approve`/`revise` carry the 1524-byte
//! `StoredGovAction` by value, so instruction data alone is 1564
//! bytes — plus a 64-byte signature that already exceeds Solana's
//! 1232-byte transaction size limit before any account key or
//! blockhash, so no real caller could submit either instruction as
//! a single transaction as currently encoded.
//!
//! SLICE-50 ADDENDUM — the 3005 ROOT CAUSE IS PROVEN, and it is
//! the stack frame, not the harness: the SBF linker reports the
//! `__global` wrapper frames at approve 5,760 bytes and revise
//! 5,696 bytes, both over the 4,096-byte BPF stack limit. A scratch
//! variant program proved it by controlled experiment (same
//! binary, same accounts, same data): a probe with fields
//! byte-identical to `ReviseGovernanceApproval` and the same
//! `StoredGovAction` arg but a trivial body loads fine (3012 on
//! the empty approval record, as it should); the same probe with
//! revise's exact body overflows identically (5,696 bytes) and
//! 3005s identically. Anchor-debug confirms the failure inside the
//! first field's `try_accounts` with an empty slice. So the frame
//! overflow corrupts the wrapper's accounts slice (undefined
//! behavior by the linker's own warning; here deterministically a
//! zero-length slice, hence 3005 at `kernel_state`). This is NOT
//! mollusk-specific: the same overflow breaks on-chain execution
//! too. Shape, data size, meta order, payer identity, and account
//! state were separately exonerated (`submit_signal` with the same
//! `init` + payer-key-seed shape loads; a 1,568-byte `submit_plan`
//! loads). FORWARD WARNING: the sibling-select wrappers
//! (`select_candidate_governance_with_siblings`,
//! `select_candidate_mixed_with_siblings`, 4,224 bytes each) also
//! overflow and are predicted to fail the same way when the chain
//! reaches them; `try_accounts`-frame overflows (e.g.
//! `ApproveGovernance::try_accounts` 4,480) are NOT the
//! discriminator (a fitting-wrapper probe whose `try_accounts`
//! overflows at 4,544 still loads) — only the `__global` wrapper
//! frame decides it. The tx-size finding above stands
//! independently, so the pending design decision (compact action
//! encoding or an account-carried action) is now the fix for BOTH
//! findings, and any fix must be frame-measured as well as
//! tx-measured. Neither is redesigned in this slice; both
//! instructions stay `Blocked` until a frame-fitting fix lands
//! and is re-measured.
//!
//! SLICE-52/53 ADDENDUM — the fix LANDED as the account-carried
//! action (program slices 51–53, Carlos's 2026-10-29 ruling):
//! `init_gov_action` + `append_gov_action_chunk` (512-byte
//! chunks) + `finalize_gov_action` carry the 1,524-byte action
//! in a `GovActionAccount` PDA, `register_governance_transition`
//! refuses an unfinalized or foreign-written account, and
//! `approve`/`revise` read the finalized bytes instead of taking
//! the action by value. Slice 53 frame-measured the rewired
//! wrappers fitting 2,048 bytes (≥2,048 bytes of margin under
//! 4,096). This harness now runs the full account-carried chain
//! — init, three chunks, finalize, registration, approve, revise
//! — so the slice-49 `Blocked` entries are replaced by measured
//! runtime proof. The action used is `GrantRole` (kind 0, role
//! Guardian) to agent `[0x99;32]`, hand-serialized here in the
//! program's fixed `StoredGovAction` Borsh layout (1,524 bytes,
//! asserted); its category is `GOV_ADJUSTABLE` (all non-policy
//! actions fall there), matching the registered record's type.
//! Revise re-binds the same finalized action (the account is
//! write-once sealed, so no second action exists for this
//! transition id — revise's runtime proof is that the wrapper
//! loads and the handler runs, with the approval count and the
//! stored hash unchanged, exactly as slice 48 specifies).
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

fn u16le(v: u16) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// Hand-serialized `StoredGovAction` for `GrantRole { who: agent,
/// role: Guardian }` in the program's fixed Borsh field order
/// (lib.rs `StoredGovAction`: kind, ttype, agent, role, resource,
/// group, cap_id, amount_raw, flag, seats_u32, policy fields,
/// set_ids[16], set_count, group_ids[16], group_count,
/// cap_ids[8], cap_amounts[8], cap_count, council_seats[15],
/// council_count). All non-kind fields zero except the agent and
/// the role code; 1,524 bytes, asserted — the same length
/// finalize demands before it hashes.
fn stored_grant_role_action(agent: [u8; 32]) -> Vec<u8> {
    let mut b = Vec::with_capacity(1524);
    b.push(0u8); // kind: GrantRole
    b.extend_from_slice(&[0u8; 32]); // ttype
    b.extend_from_slice(&agent);
    b.push(0u8); // role: Guardian
    b.extend_from_slice(&0u16.to_le_bytes()); // resource
    b.extend_from_slice(&0u16.to_le_bytes()); // group
    b.extend_from_slice(&[0u8; 32]); // cap_id
    b.extend_from_slice(&0u64.to_le_bytes()); // amount_raw
    b.push(0u8); // flag
    b.extend_from_slice(&0u32.to_le_bytes()); // seats_u32
    b.extend_from_slice(&0u64.to_le_bytes()); // policy_quorum_raw
    b.extend_from_slice(&0u64.to_le_bytes()); // policy_confidence_raw
    b.extend_from_slice(&0u64.to_le_bytes()); // policy_min_obs_raw
    b.push(0u8); // policy_require_objectives
    b.extend_from_slice(&0u64.to_le_bytes()); // policy_consensus_window
    b.extend_from_slice(&0u64.to_le_bytes()); // policy_planning_window
    b.extend_from_slice(&0u64.to_le_bytes()); // policy_execution_window
    b.extend_from_slice(&0u32.to_le_bytes()); // policy_max_plans
    b.extend_from_slice(&0u32.to_le_bytes()); // policy_settlement_threshold
    b.extend_from_slice(&0u32.to_le_bytes()); // policy_max_rounds
    b.extend_from_slice(&[0u8; 32 * 16]); // set_ids
    b.push(0u8); // set_count
    b.extend_from_slice(&[0u8; 2 * 16]); // group_ids
    b.push(0u8); // group_count
    b.extend_from_slice(&[0u8; 32 * 8]); // cap_ids
    b.extend_from_slice(&[0u8; 8 * 8]); // cap_amounts
    b.push(0u8); // cap_count
    b.extend_from_slice(&[0u8; 32 * 15]); // council_seats
    b.push(0u8); // council_count
    assert_eq!(b.len(), 1524, "StoredGovAction layout must total 1524 bytes");
    b
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
    // Account-carried action: writer is the registering proposer
    // (seat1), so the PDA seeds bind it to seat1; the approval
    // record is seeded by the approving seat (also seat1 here).
    let (gov_action_account, _) = pda(
        &[b"gov-action", GOV_TID.as_ref(), seat1.as_ref()],
        &program_id,
    );
    let (approval_record, _) = pda(
        &[b"gov-approval", GOV_TID.as_ref(), seat1.as_ref()],
        &program_id,
    );

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
        (gov_action_account, empty.clone()),
        (approval_record, empty.clone()),
        (payer, funded.clone()),
        (seat1, funded.clone()),
        (observer, funded.clone()),
        (proposer, funded.clone()),
    ];

    let mut measured: Vec<serde_json::Value> = Vec::new();
    macro_rules! push {
        ($label:expr, $cu:expr) => {
            measured.push(serde_json::json!({
                "instruction": $label,
                "cu": $cu,
                "result": "Success",
                "note": "slice-1 baseline (no design estimate exists for the v0.4 program)",
            }));
        };
    }

    // probe_kernel: payer only, no accounts created.
    let ix = Instruction::new_with_bytes(
        program_id,
        &disc("probe_kernel"),
        vec![AccountMeta::new_readonly(payer, true)],
    );
    let (next, cu) = run(&mollusk, "probe_kernel", &ix, &accounts);
    accounts = next;
    push!("probe_kernel", cu);

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
    push!("initialize_kernel", cu);
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
    push!("seal_kernel", cu);
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
    push!("advance_clock", cu);
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
    push!("register_transition", cu);
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
    push!("ingest_observation", cu);

    // Account-carried action chain (slices 51–53): the writer is
    // seat1, the registering proposer. Expected hash is the
    // SHA-256 of the full serialized action — the same hash
    // finalize computes on-chain from the accumulated bytes.
    let action_bytes = stored_grant_role_action([0x99; 32]);
    let expected_hash: [u8; 32] = Sha256::digest(&action_bytes).into();

    // init_gov_action: creates the GovActionAccount PDA.
    let mut data = disc("init_gov_action");
    data.extend_from_slice(&GOV_TID);
    data.extend_from_slice(&expected_hash);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(gov_action_account, false),
            AccountMeta::new(seat1, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "init_gov_action", &ix, &accounts);
    accounts = next;
    push!("init_gov_action", cu);

    // append_gov_action_chunk x3: 512 + 512 + 500 = 1524, the
    // sequential-prefix rule (offset == written_len) enforced.
    for (offset, chunk) in [(0u16, &action_bytes[0..512]), (512u16, &action_bytes[512..1024]), (1024u16, &action_bytes[1024..1524])] {
        let mut data = disc("append_gov_action_chunk");
        data.extend_from_slice(&GOV_TID);
        data.extend_from_slice(&u16le(offset));
        data.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        data.extend_from_slice(chunk);
        let ix = Instruction::new_with_bytes(
            program_id,
            &data,
            vec![
                AccountMeta::new(gov_action_account, false),
                AccountMeta::new(seat1, true),
            ],
        );
        let (next, cu) = run(
            &mollusk,
            &format!("append_gov_action_chunk@{offset}"),
            &ix,
            &accounts,
        );
        accounts = next;
        measured.push(serde_json::json!({
            "instruction": format!("append_gov_action_chunk@{offset}"),
            "cu": cu,
            "result": "Success",
            "note": "slice-2 account-carried chain (program slices 51-53)",
        }));
    }

    // finalize_gov_action: decodes, hashes on-chain, binds to
    // the expected hash, seals the account write-once.
    let mut data = disc("finalize_gov_action");
    data.extend_from_slice(&GOV_TID);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(gov_action_account, false),
            AccountMeta::new(seat1, true),
        ],
    );
    let (next, cu) = run(&mollusk, "finalize_gov_action", &ix, &accounts);
    accounts = next;
    push!("finalize_gov_action", cu);
    // GovActionAccount layout: 8 + 32 + 32 + 32 (expected) + 32
    // (action_hash) + 2 (written_len) + 1 (finalized).
    let ga = find(&accounts, &gov_action_account);
    assert_eq!(ga.data[104..136], expected_hash, "finalize must store the expected hash");
    assert_eq!(u16::from_le_bytes(ga.data[136..138].try_into().unwrap()), 1524, "finalize requires the full 1524 bytes written");
    assert_eq!(ga.data[138], 1, "finalize must seal the account");

    // register_governance_transition: seat ag(1), GOV_ADJUSTABLE type,
    // sealed head parent (0, zero hash). Slice-52 account list:
    // the finalized action account rides between the record and
    // the payer, binding writer == proposer at registration.
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
            AccountMeta::new_readonly(gov_action_account, false),
            AccountMeta::new(seat1, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_governance_transition", &ix, &accounts);
    accounts = next;
    push!("register_governance_transition", cu);
    let rec = find(&accounts, &gov_record);
    assert_eq!(rec.data[216], 0, "governance record must start REGISTERED");
    // approval_count offset: stage 216 + 1 + 8 + 8 + 8 + 8 + 8 = 257.
    assert_eq!(u64_at(&rec.data, 257), 0, "approval count must start 0");

    // approve_governance (slice-49 runtime proof): seat1 approves
    // the proposer's finalized action. Writer rides explicitly
    // (the approving seat is not the writer in general); here
    // writer == seat == proposer, which the gates allow.
    let mut data = disc("approve_governance");
    data.extend_from_slice(&GOV_TID);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(kernel_state, false),
            AccountMeta::new(gov_record, false),
            AccountMeta::new_readonly(seat1, false),
            AccountMeta::new_readonly(gov_action_account, false),
            AccountMeta::new(approval_record, false),
            AccountMeta::new(seat1, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "approve_governance", &ix, &accounts);
    accounts = next;
    push!("approve_governance", cu);
    let rec = find(&accounts, &gov_record);
    assert_eq!(u64_at(&rec.data, 257), 1, "approve must increment the approval count to 1");
    // GovApprovalRecord: 8 + 32 (transition_id) + 32 (seat) = 72.
    let ap = find(&accounts, &approval_record);
    assert_eq!(ap.data[72..104], expected_hash, "approval must bind the finalized action hash");

    // revise_governance_approval (slice-49 runtime proof): the
    // same seat re-binds the same finalized action — the only
    // action this transition id can carry. Count and hash must
    // stand unchanged, per slice 48.
    let mut data = disc("revise_governance_approval");
    data.extend_from_slice(&GOV_TID);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(kernel_state, false),
            AccountMeta::new(gov_record, false),
            AccountMeta::new(approval_record, false),
            AccountMeta::new_readonly(seat1, false),
            AccountMeta::new_readonly(gov_action_account, false),
            AccountMeta::new(seat1, true),
        ],
    );
    let (next, cu) = run(&mollusk, "revise_governance_approval", &ix, &accounts);
    accounts = next;
    push!("revise_governance_approval", cu);
    let rec = find(&accounts, &gov_record);
    assert_eq!(u64_at(&rec.data, 257), 1, "revise must leave the approval count at 1");
    let ap = find(&accounts, &approval_record);
    assert_eq!(ap.data[72..104], expected_hash, "revise must keep the same action hash");

    let out = serde_json::json!({
        "program": "program_v04",
        "program_id": PROGRAM_ID,
        "so_sha256": so_sha256,
        "mollusk_svm": MOLLUSK_SVM_VERSION,
        "toolchain": "Anchor 1.2.1 / Solana 4.1.2 / platform-tools v1.54",
        "slice": "cu-golden slice 2: genesis + governance registration + account-carried approval chain (program slices 51-53)",
        "measured": measured,
        "blocked": [],
        "design_finding_resolved": "Slice-49/50 findings CLOSED by the account-carried action (program slices 51-53, Carlos's 2026-10-29 ruling): approve/revise no longer carry the 1524-byte StoredGovAction by value (the 1564-byte instruction-data / tx-size finding is gone — the largest new instruction is a 512-byte chunk), and the rewired wrappers frame-measure fitting 2048 bytes (slice 53). This run proves both instructions execute in-process against the real .so: approve stores the finalized action hash and increments the approval count to 1; revise re-binds the same finalized action with count and hash unchanged.",
    });
    std::fs::write(&out_path, serde_json::to_string_pretty(&out).unwrap())
        .unwrap_or_else(|e| panic!("cannot write {out_path}: {e}"));
    println!("wrote {out_path}");
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
