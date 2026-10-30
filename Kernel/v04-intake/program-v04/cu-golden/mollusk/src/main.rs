//! V0.4 program CU golden-test harness (mollusk-svm path), slice 1.
//!
//! Measures real compute units per instruction for `program-v04` by
//! executing the real `.so` in-process (mollusk-svm 0.16.4) — not
//! estimates, not host-test timings. Every measured instruction must
//! succeed; any failure aborts with a non-zero exit, so a golden file
//! is never written from a failed chain.
//!
//! Slice 3 chain = slice 2 (genesis + governance registration +
//! account-carried approval chain) plus the close chain: an
//! abandoned second action account (init + one chunk, never
//! finalized) closed for rent reclaim, and a refused close on
//! the finalized GOV account. Slice 1 chain (genesis +
//! governance registration): `probe_kernel`,
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
//! SLICE-62 ADDENDUM — golden freeze. This slice adds the
//! reputation-init chain (`init_reputation` for a sealed-profile
//! agent at the genesis midpoint, plus the re-init refusal that
//! proves the record one-time) and freezes the resulting golden
//! for the current `.so` (sha in the golden file): all prior
//! measurements must reproduce exactly, since the program is
//! unchanged since slice 61. What the freeze does NOT cover,
//! stated plainly: `record_outcome` / `record_outcome_governance`
//! with reputation tails — those need a SETTLED transition, which
//! needs the full lifecycle chain (replay through settle) driven
//! in mollusk, and that chain is the next slice's work, not
//! something this freeze claims. No outcome-tail CU appears in
//! this golden file.
//!
//! SLICE-63 ADDENDUM — partial lifecycle chain, and a BLOCKING
//! FINDING (roadmap F4). The chain is a NEW ordinary transition
//! (the slice-1 ordinary record cannot replay: it names
//! observation `[0xC0;32]`, which was never ingested — its
//! observation account is the `[0xC1;32]` record, a
//! registration-only fixture, not a lifecycle input):
//! register TT `[9;32]` with proposer ag(1), then ingest
//! observation `[0xC2;32]` (timestamp 200, expiry 9100,
//! confidence 900,000) by the launch observer — both measured
//! and green. The next step, `replay_transition`, DOES NOT
//! EXECUTE in-process: it panics with the program log
//! "Error: memory allocation failed, out of memory" at
//! ~36.7k CU (mollusk `UnknownError(ProgramFailedToComplete)`),
//! reproduced in a minimal probe (initialize -> seal ->
//! register -> ingest -> replay) at the default heap AND at
//! mollusk's maximum configurable heap (256 KiB; 512 KiB is
//! rejected by the VM as "invalid heap size"). This is a
//! panic, not a gate refusal, so it is recorded under
//! `blocked`, never under `refusals` or `measured`. The
//! program is unchanged since slice 61, so every prior
//! measurement still reproduces exactly. The full lifecycle
//! chain (replay through settle to an outcome with a
//! reputation tail) and the governance outcome tail ride the
//! F4 fix slice, not this one: no stage instruction has ever
//! executed in-process before — every prior slice measured
//! only genesis / registration / approval / close chains,
//! none of which rebuild the sealed kernel the way the stage
//! helpers do (`resolve_proposer` builds one sealed kernel,
//! `kernel_at_head_from_entries` builds a second, then
//! ingest + register run on it). Host-test helper suites
//! pass; the SBF execution path does not. The F4 fix is
//! port-side work (both builders live in program-v04's own
//! helpers, not in the delivered crate), and its first job is
//! to find the allocation that exceeds the maximum heap —
//! the two-build shape alone does not explain it, since
//! `seal_kernel` (one full genesis build) fits in the
//! default 32 KiB heap.
//!
//! SLICE-65 ADDENDUM — full lifecycle chain, replay through outcome.
//! The slice-64 chain continues past replay at clock 200: evaluate,
//! mark_eligible (window 200/300), a round-0 signal by ag(2) (raw
//! SCALE, support SCALE — weight 2.0, confidence 1.0, clearing the
//! 1.0 quorum and the 0.5 threshold alone, as in the host tests),
//! advance to the 300 deadline, select, open_planning by the
//! proposer seat1 (planning deadline 400), a two-claim plan by ag(2)
//! (tranche 10 * SCALE, lending book 5 * SCALE, expiring 500),
//! advance to 400, reconcile (compatible), validate, declare,
//! commit (head moves to 1 at the declared hash), settlement
//! evidence recorded by the launch observer at the declared hash,
//! settle (exact), init_reputation for ag(2), and record_outcome
//! by RECORDER with attribution (ag(2), +10) — the reputation tail
//! slice 62 explicitly left unmeasured. All prior measurements
//! must reproduce exactly; the program is unchanged since slice 64.
//!
//! SLICE-66 ADDENDUM (2026-10-30) — F6 direction (c) lands in the
//! program: a custom 256 KiB SBF heap allocator (PROGRAM_ALLOCATOR,
//! `custom-heap` default feature) replaces the entrypoint default, and
//! this harness requests the matching 256 KiB heap frame for every
//! instruction (see the heap_size assignment in main). If the full
//! slice-65 chain now completes, the golden re-freezes as slice 66:
//! prior values shift by the 56 CU heap-cost delta of the 256 KiB frame
//! (plus any allocator-code delta, expected zero — same BumpAllocator
//! code, only the length constant changes), and the slice-65 stage
//! values land for the first time.
//!
//! SLICE-65 STATUS (2026-10-30): PARTIAL — evaluate (25,872 CU),
//! mark_eligible (28,179 CU), and submit_signal (35,311 CU) execute
//! in-process, but select_candidate panics out-of-memory at
//! 27,977 CU (finding F6: select's longer replay exceeds the
//! effective 32 KiB SBF heap under cumulative accounting — host
//! peak live 27,837 bytes, total 31,237 bytes). The steps past
//! select below are scripted and asserted but UNPROVEN in-process
//! until F6 is resolved; the golden file is not rewritten from
//! this partial chain.
//!
//! SLICE-64 ADDENDUM — F4 FIXED, replay executes in-process.
//! The stage helpers no longer build two kernels: the new
//! `resolve_proposer_on_kernel` judges the proposer on the single
//! rebuilt head kernel every `*_from_records` helper and chain
//! helper already builds (27 sites), instead of `resolve_proposer`
//! building a second sealed kernel that SBF heap accounting never
//! reclaims within the instruction. Host battery 457/457, anchor
//! build exit 0, all slice-62 measurements reproduce exactly, and
//! the slice-63 chain now continues past ingest: `replay_transition`
//! succeeds (CU in the golden file) and advances the lifecycle
//! record to KERNEL_REGISTERED at 25,435 CU. Semantic note, stated plainly:
//! proposer resolution now judges the rebuilt head roster, not
//! sealed genesis — a governance-registered proposer resolves
//! where sealed-genesis resolution refused it, with the kernel's
//! own registration gate, judging the same rebuilt kernel, as the
//! backstop. The `blocked` list is empty again; the full lifecycle
//! chain (evaluate through settle) is the next slice's work.
//!
//! SLICE-3 ADDENDUM — close_gov_action, the last unmeasured
//! slice-51 instruction, now measured: a second, abandoned
//! action account (init + one 512-byte chunk, never finalized)
//! closes with its rent returned to the writer, and close on
//! the finalized GOV account refuses with
//! GovActionAlreadyFinalized, leaving the sealed account
//! untouched — the finalized-stays-open lifetime rule proven
//! at runtime. Refusal CU is recorded under `refusals`, never
//! as a success measurement.
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
const MOLLUSK_SVM_VERSION: &str = "0.16.4";
const TOOLCHAIN: &str = "Anchor 1.2.2 / Solana 4.3.2 / platform-tools v1.57";

/// Stamp==resolved check (Carlos's 2026-10-29 migration condition 2):
/// the golden JSON's `mollusk_svm` field must equal the version the
/// harness's own Cargo.lock actually resolves. A drifted lock (or a
/// copy run against a different lock) aborts before any measurement
/// is written, so the stamp can never again silently mislabel the
/// measurement environment (the 0.16.2-stamp-under-0.16.4 gap).
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
        "mollusk stamp drift: golden would stamp {MOLLUSK_SVM_VERSION} but lock resolves {resolved}"
    );
}

// Pinned launch profile constants (program-v04 lib.rs, slices 7/33).
const GOV_TID: [u8; 32] = [0xD1; 32];
const ABANDON_TID: [u8; 32] = [0xD3; 32];
const ORD_TID: [u8; 32] = [0xD2; 32];
// Slice-63 full-lifecycle chain: its own transition + observation.
const LIFE_TID: [u8; 32] = [0xD4; 32];
const LIFE_OBS: [u8; 32] = [0xC2; 32];
// Launch-profile ordinary transition type (lib.rs `TT`).
const TT_TYPE: [u8; 32] = [9; 32];
// Kernel Amount SCALE (coordination-kernel-rs num.rs): 1.0 in raw units.
const SCALE: u64 = 1_000_000;
// Launch RECORDER identity (lib.rs): the outcome recorder role holder.
const RECORDER: [u8; 32] = [0xB4; 32];
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

/// Runs one instruction expected to REFUSE, asserts the failure,
/// prints its measured CU, and returns the threaded accounts plus
/// the measured CU and the error debug string. A refusal's CU is
/// still real compute consumed, so it is recorded in the golden
/// file's `refusals` section, never in `measured`.
fn run_refused(
    mollusk: &Mollusk,
    label: &str,
    ix: &Instruction,
    accounts: &[(Pubkey, Account)],
) -> (Vec<(Pubkey, Account)>, u64, String) {
    let result = mollusk.process_instruction(ix, accounts);
    println!(
        "{label}: program_result={:?} cu={}",
        result.program_result, result.compute_units_consumed
    );
    assert!(
        result.program_result.is_err(),
        "{label} was expected to refuse but succeeded",
    );
    let err = format!("{:?}", result.program_result);
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

fn main() {
    assert_mollusk_stamp_matches_lock();
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
    let mut mollusk = Mollusk::new(&program_id, "program_v04");
    // Slice 66 (F6 direction (c)): the program now carries a custom 256 KiB
    // SBF heap allocator (PROGRAM_ALLOCATOR), and its bump allocator
    // allocates top-down from the end of that range — so the harness must
    // request the matching 256 KiB heap frame for every instruction, or
    // the first allocation faults outside the mapped region. This is the
    // client contract the program's slice-66 comment states, not a
    // harness-only tweak: any real client must include a
    // request_heap_frame(256 * 1024) instruction. Heap cost at 256 KiB
    // is 56 CU over the 32 KiB default, so golden values re-freeze with
    // that uniform shift on top of any allocator-code delta.
    mollusk.compute_budget.heap_size = 256 * 1024;

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
    // Abandoned action (slice-3 close chain): a second action
    // account under its own transition id, half-written and never
    // finalized — the rent-reclaim path's intended input.
    let (abandon_account, _) = pda(
        &[b"gov-action", ABANDON_TID.as_ref(), seat1.as_ref()],
        &program_id,
    );
    // Slice-62 golden-freeze chain: the reputation record for
    // seat1 (ag(1) = [1;32]), a sealed-profile agent, so
    // `init_reputation` builds at the genesis midpoint.
    let (rep_record, rep_bump) = pda(&[b"reputation", seat1.as_ref()], &program_id);
    // Slice-63 lifecycle chain accounts: a fresh ordinary
    // transition + observation (the chain stops at the F4
    // replay block, so no signal/plan/evidence/reputation
    // accounts are needed yet).
    let (life_record, _) = pda(&[b"transition", LIFE_TID.as_ref()], &program_id);
    let (life_obs, _) = pda(&[b"observation", LIFE_OBS.as_ref()], &program_id);
    // Slice-65 lifecycle chain accounts: the round-0 signal by
    // ag(2), its plan, the settlement evidence, and ag(2)'s
    // reputation record (init'd just before record_outcome).
    let signaler = Pubkey::new_from_array([2; 32]);
    let recorder = Pubkey::new_from_array(RECORDER);
    let round0_le = 0u64.to_le_bytes();
    let (life_signal, _) = pda(
        &[b"signal", LIFE_TID.as_ref(), signaler.as_ref(), &round0_le],
        &program_id,
    );
    let (life_plan, _) = pda(&[b"plan", LIFE_TID.as_ref(), signaler.as_ref()], &program_id);
    let (life_evidence, _) = pda(&[b"evidence", LIFE_TID.as_ref()], &program_id);
    let (rep_ag2, _) = pda(&[b"reputation", signaler.as_ref()], &program_id);

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
        (abandon_account, empty.clone()),
        (rep_record, empty.clone()),
        (life_record, empty.clone()),
        (life_obs, empty.clone()),
        (life_signal, empty.clone()),
        (life_plan, empty.clone()),
        (life_evidence, empty.clone()),
        (rep_ag2, empty.clone()),
        (payer, funded.clone()),
        (seat1, funded.clone()),
        (observer, funded.clone()),
        (proposer, funded.clone()),
        (signaler, funded.clone()),
        (recorder, funded.clone()),
    ];

    let mut measured: Vec<serde_json::Value> = Vec::new();
    // Slice 62: refusals are collected from the start (the
    // re-init refusal below) and the close chain appends later.
    let mut refusals: Vec<serde_json::Value> = Vec::new();
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

    // SLICE-62 ADDENDUM — golden freeze, reputation init chain.
    // `init_reputation` for seat1, a sealed-profile agent: the
    // record builds at the genesis midpoint (50 * SCALE =
    // 50,000,000 raw) with the PDA bump, funded by the payer.
    // A second init on the same account must refuse (the `init`
    // constraint makes the record one-time), and its CU is
    // recorded under `refusals`, never as a success.
    let mut data = disc("init_reputation");
    data.extend_from_slice(seat1.as_ref());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(kernel_state, false),
            AccountMeta::new(rep_record, false),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "init_reputation", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({
        "instruction": "init_reputation",
        "cu": cu,
        "result": "Success",
        "note": "golden freeze slice 62: reputation init chain (program slice 61)",
    }));
    // ReputationRecord layout: 8 (discriminator) + 32 (agent) +
    // 8 (reputation_raw) + 1 (bump).
    let rp = find(&accounts, &rep_record);
    assert_eq!(rp.data[8..40], seat1.to_bytes(), "reputation record must name seat1");
    assert_eq!(u64_at(&rp.data, 40), 50_000_000, "reputation must start at the genesis midpoint");
    assert_eq!(rp.data[48], rep_bump, "reputation record must carry its PDA bump");

    // Re-init must refuse and leave the genesis value untouched.
    let mut data = disc("init_reputation");
    data.extend_from_slice(seat1.as_ref());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(kernel_state, false),
            AccountMeta::new(rep_record, false),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu, err) = run_refused(&mollusk, "init_reputation_again", &ix, &accounts);
    accounts = next;
    refusals.push(serde_json::json!({
        "instruction": "init_reputation_again",
        "cu": cu,
        "result": "Refused",
        "error": err,
        "note": "golden freeze slice 62: the `init` constraint makes a reputation record one-time — no caller can reset a reputation to genesis after outcomes move it",
    }));
    let rp = find(&accounts, &rep_record);
    assert_eq!(u64_at(&rp.data, 40), 50_000_000, "refused re-init must leave the genesis value untouched");

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

    // SLICE-3 ADDENDUM — close_gov_action, the last unmeasured
    // slice-51 instruction. Two proofs: (a) the rent-reclaim
    // success path on an abandoned account — init + one 512-byte
    // chunk, never finalized, then close, with the rent returned
    // to the writer; (b) the lifetime gate — close on the
    // finalized GOV account refuses with
    // GovActionAlreadyFinalized and leaves that account sealed
    // and untouched (the post-settle reclaim decision is still
    // open, but the finalized-stays-open rule is now runtime
    // proven, not just host-tested).
    // (`refusals` was declared at the top for the slice-62
    // re-init refusal; this chain appends to it.)

    // (a1) init the abandoned account (writer seat1, same action
    // bytes' hash as the expected hash — finalization never runs,
    // so the hash is never checked here).
    let mut data = disc("init_gov_action");
    data.extend_from_slice(&ABANDON_TID);
    data.extend_from_slice(&expected_hash);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(abandon_account, false),
            AccountMeta::new(seat1, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "init_gov_action_abandoned", &ix, &accounts);
    accounts = next;
    push!("init_gov_action_abandoned", cu);
    let ab = find(&accounts, &abandon_account);
    assert_eq!(ab.data[138], 0, "abandoned account must start unfinalized");

    // (a2) one chunk: half-written, 512 of 1524 bytes.
    let mut data = disc("append_gov_action_chunk");
    data.extend_from_slice(&ABANDON_TID);
    data.extend_from_slice(&u16le(0));
    data.extend_from_slice(&(512u32).to_le_bytes());
    data.extend_from_slice(&action_bytes[0..512]);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(abandon_account, false),
            AccountMeta::new(seat1, true),
        ],
    );
    let (next, cu) = run(&mollusk, "append_gov_action_chunk_abandoned", &ix, &accounts);
    accounts = next;
    push!("append_gov_action_chunk_abandoned", cu);
    let ab = find(&accounts, &abandon_account);
    assert_eq!(u16::from_le_bytes(ab.data[136..138].try_into().unwrap()), 512, "abandoned account must hold exactly one chunk");

    // (a3) close: rent returns to the writer, account emptied.
    let pre_writer_lamports = find(&accounts, &seat1).lamports;
    let pre_abandon_lamports = find(&accounts, &abandon_account).lamports;
    assert!(pre_abandon_lamports > 0, "abandoned account must hold rent before close");
    let mut data = disc("close_gov_action");
    data.extend_from_slice(&ABANDON_TID);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(abandon_account, false),
            AccountMeta::new(seat1, true),
        ],
    );
    let (next, cu) = run(&mollusk, "close_gov_action", &ix, &accounts);
    accounts = next;
    push!("close_gov_action", cu);
    assert_eq!(
        find(&accounts, &seat1).lamports,
        pre_writer_lamports + pre_abandon_lamports,
        "close must return the abandoned account's rent to the writer"
    );
    // Mollusk dropping the closed account is the same outcome as
    // an emptied account, so only the present case is asserted.
    if let Some((_, closed)) = accounts.iter().find(|(k, _)| k == &abandon_account) {
        assert_eq!(closed.lamports, 0, "closed account must hold no lamports");
        assert!(closed.data.is_empty(), "closed account must be emptied");
    }

    // (b) close on the finalized GOV account must refuse, and
    // the sealed account must stand untouched afterwards.
    let mut data = disc("close_gov_action");
    data.extend_from_slice(&GOV_TID);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(gov_action_account, false),
            AccountMeta::new(seat1, true),
        ],
    );
    let (next, cu, err) = run_refused(&mollusk, "close_gov_action_finalized", &ix, &accounts);
    accounts = next;
    // Mollusk reports the failure as Failure(Custom(6034)); the
    // program log on the same execution names it
    // GovActionAlreadyFinalized (error 6034, lib.rs:5043).
    assert!(
        err.contains("6034"),
        "finalized close must refuse with error 6034 (GovActionAlreadyFinalized), got {err}"
    );
    refusals.push(serde_json::json!({
        "instruction": "close_gov_action_finalized",
        "cu": cu,
        "result": "Refused",
        "error": "GovActionAlreadyFinalized (6034)",
        "note": "slice-3 lifetime gate: a finalized account never closes (Rule A, Carlos 2026-10-29)",
    }));
    let ga = find(&accounts, &gov_action_account);
    assert_eq!(ga.data[138], 1, "refused close must leave the finalized account sealed");
    assert_eq!(ga.data[104..136], expected_hash, "refused close must leave the finalized hash untouched");

    // SLICE-63 ADDENDUM — partial lifecycle chain (see the header
    // addendum and roadmap F4): register + ingest only. The next
    // step, `replay_transition`, panics with an out-of-memory
    // alloc failure in-process, so it is NOT run here — it is
    // recorded under `blocked` in the golden JSON, never as a
    // success or a refusal. The chain resumes past replay once
    // the F4 fix lands.
    let life_note = "slice-63/64 lifecycle chain (register + ingest; replay unblocked by the slice-64 F4 fix)";
    // Register the lifecycle transition: TT, proposer ag(1)
    // (seat1), sealed head parent (0, zero hash), observation
    // LIFE_OBS, expires 10100.
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
            AccountMeta::new(seat1, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "register_transition_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "register_transition_life", "cu": cu, "result": "Success", "note": life_note}));
    assert_eq!(find(&accounts, &life_record).data[216], 0, "lifecycle record must start REGISTERED");

    // Ingest the lifecycle observation: observer [0xA1;32],
    // timestamp 200 (== clock), expiry 9100, confidence 900,000.
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
            AccountMeta::new(life_obs, false),
            AccountMeta::new(observer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "ingest_observation_life", &ix, &accounts);
    measured.push(serde_json::json!({"instruction": "ingest_observation_life", "cu": cu, "result": "Success", "note": life_note}));
    // ObservationRecord layout: 8 + 32 (id) + 8 (timestamp) +
    // 8 (expiry) + 8 (confidence) + 32 (observer) + 1 (bump).
    let ob = find(&next, &life_obs);
    assert_eq!(u64_at(&ob.data, 40), 200, "lifecycle observation timestamp must be 200");
    assert_eq!(ob.data[64..96], observer.to_bytes(), "lifecycle observation must name the launch observer");

    // SLICE-64 — F4 fixed: the lifecycle chain continues past ingest.
    // Replay runs kernel ingest_observation + register_transition from
    // the stored record pair (no prior bundles: the lifecycle parent is
    // the sealed genesis head) and must advance the record to
    // KERNEL_REGISTERED (stage byte 1 at offset 216).
    accounts = next;
    let mut data = disc("replay_transition");
    data.extend_from_slice(&LIFE_TID);
    data.extend_from_slice(&LIFE_OBS);
    data.extend_from_slice(&0u32.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(kernel_state, false),
            AccountMeta::new(life_record, false),
            AccountMeta::new_readonly(life_obs, false),
            AccountMeta::new(seat1, true),
        ],
    );
    let (next, cu) = run(&mollusk, "replay_transition_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "replay_transition_life", "cu": cu, "result": "Success", "note": "slice-64 F4 fix: first stage instruction to execute in-process; single-build refactor (resolve_proposer_on_kernel, 27 sites)"}));
    assert_eq!(find(&accounts, &life_record).data[216], 1, "lifecycle record must advance to KERNEL_REGISTERED");

    // SLICE-65 — full lifecycle chain (see the header addendum).
    // Clock is 200; the record is KERNEL_REGISTERED at the sealed
    // genesis head (0, zero hash). Stage offsets in
    // TransitionRecord (SPACE 271): stage 216, eligible_at 217,
    // consensus_deadline 225, planning_deadline 233, signal_count
    // 241, plan_count 249, approval_count 257, replans 265 (u32),
    // compatible 269. KernelState: sealed 8, head_index 9,
    // head_hash 17..49, clock 49.
    let life_note65 = "slice-65 full lifecycle chain (evaluate through outcome with reputation tail)";
    // Small helper data prefix: disc + LIFE_TID + LIFE_OBS + u32 0
    // (prior_bundle_count: the lifecycle parent is the sealed
    // genesis head, so there are no prior bundles).
    let stage_ix = |name: &str| -> Vec<u8> {
        let mut d = disc(name);
        d.extend_from_slice(&LIFE_TID);
        d.extend_from_slice(&LIFE_OBS);
        d.extend_from_slice(&0u32.to_le_bytes());
        d
    };
    let stage_accounts = |ks_mut: bool| -> Vec<AccountMeta> {
        vec![
            if ks_mut { AccountMeta::new(kernel_state, false) } else { AccountMeta::new_readonly(kernel_state, false) },
            AccountMeta::new(life_record, false),
            AccountMeta::new_readonly(life_obs, false),
            AccountMeta::new_readonly(payer, true),
        ]
    };

    // evaluate_transition @200 -> EVALUATED (2).
    let ix = Instruction::new_with_bytes(program_id, &stage_ix("evaluate_transition"), stage_accounts(false));
    let (next, cu) = run(&mollusk, "evaluate_transition_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "evaluate_transition_life", "cu": cu, "result": "Success", "note": life_note65}));
    assert_eq!(find(&accounts, &life_record).data[216], 2, "lifecycle record must advance to EVALUATED");

    // mark_eligible @200 -> ELIGIBLE (3), window 200/300, and the
    // sibling census reset to 1 at the genesis parent (select
    // proves the census; no direct KernelState census assert — its
    // layout past the clock is not pinned by this harness).
    let ix = Instruction::new_with_bytes(program_id, &stage_ix("mark_eligible"), stage_accounts(true));
    let (next, cu) = run(&mollusk, "mark_eligible_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "mark_eligible_life", "cu": cu, "result": "Success", "note": life_note65}));
    let rec = find(&accounts, &life_record);
    assert_eq!(rec.data[216], 3, "lifecycle record must advance to ELIGIBLE");
    assert_eq!(u64_at(&rec.data, 217), 200, "eligibility window must open at the 200 clock");
    assert_eq!(u64_at(&rec.data, 225), 300, "consensus deadline must be 200 + the 100-tick window");

    // submit_signal round 0 @200 by ag(2): raw SCALE, support
    // SCALE, has_support true. No priors (first signal).
    let mut data = disc("submit_signal");
    data.extend_from_slice(&LIFE_TID);
    data.extend_from_slice(&LIFE_OBS);
    data.extend_from_slice(&u64le(0));
    data.extend_from_slice(&u64le(SCALE));
    data.extend_from_slice(&u64le(SCALE));
    data.push(1u8);
    data.extend_from_slice(&0u32.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(kernel_state, false),
            AccountMeta::new(life_record, false),
            AccountMeta::new_readonly(life_obs, false),
            AccountMeta::new(life_signal, false),
            AccountMeta::new(signaler, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_signal_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_signal_life", "cu": cu, "result": "Success", "note": life_note65}));
    let rec = find(&accounts, &life_record);
    assert_eq!(rec.data[216], 3, "signals must not advance the stage");
    assert_eq!(u64_at(&rec.data, 241), 1, "signal_count must move to 1");
    // SignalRecord layout: 8 + 32 + 32 + 8 (round) + 8 (raw) +
    // 8 (support) + 1 (has_support) + 8 (submitted_at) + 1 (bump).
    let sg = find(&accounts, &life_signal);
    assert_eq!(u64_at(&sg.data, 72), 0, "signal must be round 0");
    assert_eq!(u64_at(&sg.data, 80), SCALE, "signal raw must be SCALE");
    assert_eq!(u64_at(&sg.data, 97), 200, "signal must be submitted at clock 200");

    // advance_clock to the 300 consensus deadline.
    let mut data = disc("advance_clock");
    data.extend_from_slice(&u64le(300));
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(kernel_state, false),
            AccountMeta::new_readonly(payer, true),
        ],
    );
    let (next, cu) = run(&mollusk, "advance_clock_300", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "advance_clock_300", "cu": cu, "result": "Success", "note": life_note65}));
    assert_eq!(u64_at(&find(&accounts, &kernel_state).data, 49), 300, "clock must advance to 300");

    // select_candidate @300 (closed window) with the one prior
    // signal -> SELECTED (4).
    let mut metas = stage_accounts(false);
    metas.push(AccountMeta::new_readonly(life_signal, false));
    let ix = Instruction::new_with_bytes(program_id, &stage_ix("select_candidate"), metas);
    let (next, cu) = run(&mollusk, "select_candidate_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "select_candidate_life", "cu": cu, "result": "Success", "note": life_note65}));
    assert_eq!(find(&accounts, &life_record).data[216], 4, "lifecycle record must advance to SELECTED");

    // open_planning @300 by the proposer seat1 -> PLANNING (5),
    // planning deadline 400.
    let ix = Instruction::new_with_bytes(
        program_id,
        &stage_ix("open_planning"),
        vec![
            AccountMeta::new_readonly(kernel_state, false),
            AccountMeta::new(life_record, false),
            AccountMeta::new_readonly(life_obs, false),
            AccountMeta::new_readonly(seat1, true),
            AccountMeta::new_readonly(life_signal, false),
        ],
    );
    let (next, cu) = run(&mollusk, "open_planning_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "open_planning_life", "cu": cu, "result": "Success", "note": life_note65}));
    let rec = find(&accounts, &life_record);
    assert_eq!(rec.data[216], 5, "lifecycle record must advance to PLANNING");
    assert_eq!(u64_at(&rec.data, 233), 400, "planning deadline must be 300 + the 100-tick window");

    // submit_plan @300 by ag(2): tranche 10 * SCALE + lending
    // book 5 * SCALE (both far inside the 1000 / 500 * SCALE
    // stocks), expiring 500 (past the planning deadline).
    let mut data = disc("submit_plan");
    data.extend_from_slice(&LIFE_TID);
    data.extend_from_slice(&LIFE_OBS);
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&u64le(10 * SCALE));
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&u64le(5 * SCALE));
    data.extend_from_slice(&u64le(500));
    data.extend_from_slice(&0u32.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(kernel_state, false),
            AccountMeta::new(life_record, false),
            AccountMeta::new_readonly(life_obs, false),
            AccountMeta::new(life_plan, false),
            AccountMeta::new(signaler, true),
            AccountMeta::new_readonly(system_id, false),
            AccountMeta::new_readonly(life_signal, false),
        ],
    );
    let (next, cu) = run(&mollusk, "submit_plan_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "submit_plan_life", "cu": cu, "result": "Success", "note": life_note65}));
    let rec = find(&accounts, &life_record);
    assert_eq!(rec.data[216], 5, "plans must not advance the stage");
    assert_eq!(u64_at(&rec.data, 249), 1, "plan_count must move to 1");

    // advance_clock to the 400 planning deadline.
    let mut data = disc("advance_clock");
    data.extend_from_slice(&u64le(400));
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(kernel_state, false),
            AccountMeta::new_readonly(payer, true),
        ],
    );
    let (next, cu) = run(&mollusk, "advance_clock_400", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "advance_clock_400", "cu": cu, "result": "Success", "note": life_note65}));
    assert_eq!(u64_at(&find(&accounts, &kernel_state).data, 49), 400, "clock must advance to 400");

    // reconcile_plans @400 (closed window) -> RECONCILING (7),
    // compatible true (the fitting plan set).
    let mut metas = stage_accounts(false);
    metas.push(AccountMeta::new_readonly(life_signal, false));
    metas.push(AccountMeta::new_readonly(life_plan, false));
    let ix = Instruction::new_with_bytes(program_id, &stage_ix("reconcile_plans"), metas);
    let (next, cu) = run(&mollusk, "reconcile_plans_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "reconcile_plans_life", "cu": cu, "result": "Success", "note": life_note65}));
    let rec = find(&accounts, &life_record);
    assert_eq!(rec.data[216], 7, "lifecycle record must advance to RECONCILING");
    assert_eq!(rec.data[269], 1, "the fitting plan set must reconcile compatible");

    // validate_transition @400 -> VALIDATED (8).
    let mut metas = stage_accounts(false);
    metas.push(AccountMeta::new_readonly(life_signal, false));
    metas.push(AccountMeta::new_readonly(life_plan, false));
    let ix = Instruction::new_with_bytes(program_id, &stage_ix("validate_transition"), metas);
    let (next, cu) = run(&mollusk, "validate_transition_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "validate_transition_life", "cu": cu, "result": "Success", "note": life_note65}));
    assert_eq!(find(&accounts, &life_record).data[216], 8, "lifecycle record must advance to VALIDATED");

    // declare_transition @400 -> DECLARED (9).
    let mut metas = stage_accounts(false);
    metas.push(AccountMeta::new_readonly(life_signal, false));
    metas.push(AccountMeta::new_readonly(life_plan, false));
    let ix = Instruction::new_with_bytes(program_id, &stage_ix("declare_transition"), metas);
    let (next, cu) = run(&mollusk, "declare_transition_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "declare_transition_life", "cu": cu, "result": "Success", "note": life_note65}));
    assert_eq!(find(&accounts, &life_record).data[216], 9, "lifecycle record must advance to DECLARED");

    // commit_transition @400 -> COMMITTED (10); the head moves
    // to index 1 at the declared next hash, and the ledger holds
    // the committed claims (proven by the settle replay below).
    let mut metas = stage_accounts(true);
    metas.push(AccountMeta::new_readonly(life_signal, false));
    metas.push(AccountMeta::new_readonly(life_plan, false));
    let ix = Instruction::new_with_bytes(program_id, &stage_ix("commit_transition"), metas);
    let (next, cu) = run(&mollusk, "commit_transition_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "commit_transition_life", "cu": cu, "result": "Success", "note": life_note65}));
    assert_eq!(find(&accounts, &life_record).data[216], 10, "lifecycle record must advance to COMMITTED");
    let ks = find(&accounts, &kernel_state);
    assert_eq!(u64_at(&ks.data, 9), 1, "commit must move the head to index 1");
    assert_eq!(ks.data[17..49], DECLARED_HASH, "commit must move the head to the declared hash");

    // record_settlement_evidence @400 by the launch observer at
    // the declared hash (exact settle path).
    let mut data = disc("record_settlement_evidence");
    data.extend_from_slice(&LIFE_TID);
    data.extend_from_slice(&LIFE_OBS);
    data.extend_from_slice(&DECLARED_HASH);
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(kernel_state, false),
            AccountMeta::new_readonly(life_record, false),
            AccountMeta::new_readonly(life_obs, false),
            AccountMeta::new(life_evidence, false),
            AccountMeta::new(observer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "record_settlement_evidence_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "record_settlement_evidence_life", "cu": cu, "result": "Success", "note": life_note65}));
    // SettlementEvidence layout: 8 + 32 (transition) + 32
    // (realized) + 32 (recorder) + 1 (bump).
    let ev = find(&accounts, &life_evidence);
    assert_eq!(ev.data[40..72], DECLARED_HASH, "evidence must carry the declared hash");
    assert_eq!(ev.data[72..104], observer.to_bytes(), "evidence must name the launch observer as recorder");

    // settle_transition @400 -> SETTLED (11); settle moves
    // neither the head nor the ledger. Named accounts in the
    // struct's order: kernel_state, transition_record,
    // observation_record, settlement_evidence, payer; then the
    // signal and plan priors as remaining accounts.
    let metas = vec![
        AccountMeta::new(kernel_state, false),
        AccountMeta::new(life_record, false),
        AccountMeta::new_readonly(life_obs, false),
        AccountMeta::new_readonly(life_evidence, false),
        AccountMeta::new_readonly(payer, true),
        AccountMeta::new_readonly(life_signal, false),
        AccountMeta::new_readonly(life_plan, false),
    ];
    let ix = Instruction::new_with_bytes(program_id, &stage_ix("settle_transition"), metas);
    let (next, cu) = run(&mollusk, "settle_transition_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "settle_transition_life", "cu": cu, "result": "Success", "note": life_note65}));
    assert_eq!(find(&accounts, &life_record).data[216], 11, "lifecycle record must advance to SETTLED");
    let ks = find(&accounts, &kernel_state);
    assert_eq!(u64_at(&ks.data, 9), 1, "settle must leave the head at index 1");
    assert_eq!(ks.data[17..49], DECLARED_HASH, "settle must leave the head at the declared hash");

    // init_reputation for ag(2) (a sealed-profile agent) at the
    // genesis midpoint, so the outcome tail has a record to move.
    let mut data = disc("init_reputation");
    data.extend_from_slice(signaler.as_ref());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(kernel_state, false),
            AccountMeta::new(rep_ag2, false),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(system_id, false),
        ],
    );
    let (next, cu) = run(&mollusk, "init_reputation_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "init_reputation_life", "cu": cu, "result": "Success", "note": life_note65}));
    assert_eq!(u64_at(&find(&accounts, &rep_ag2).data, 40), 50_000_000, "ag(2) reputation must start at the genesis midpoint");

    // record_outcome by RECORDER with attribution (ag(2), +10)
    // on the exact settle -> EVALUATED_OUTCOME (12); exact
    // credit is unhalved, so the reputation moves 50,000,000 ->
    // 50,000,010 (delta units are kernel raw units).
    let mut data = disc("record_outcome");
    data.extend_from_slice(&LIFE_TID);
    data.extend_from_slice(&LIFE_OBS);
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(signaler.as_ref());
    data.extend_from_slice(&10i64.to_le_bytes());
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(kernel_state, false),
            AccountMeta::new(life_record, false),
            AccountMeta::new_readonly(life_obs, false),
            AccountMeta::new_readonly(life_evidence, false),
            AccountMeta::new_readonly(recorder, true),
            AccountMeta::new_readonly(life_signal, false),
            AccountMeta::new_readonly(life_plan, false),
            AccountMeta::new(rep_ag2, false),
        ],
    );
    let (next, cu) = run(&mollusk, "record_outcome_life", &ix, &accounts);
    accounts = next;
    measured.push(serde_json::json!({"instruction": "record_outcome_life", "cu": cu, "result": "Success", "note": life_note65}));
    assert_eq!(find(&accounts, &life_record).data[216], 12, "lifecycle record must advance to EVALUATED_OUTCOME");
    assert_eq!(u64_at(&find(&accounts, &rep_ag2).data, 40), 50_000_010, "exact outcome credit must move ag(2) reputation to 50,000,010");

    let out = serde_json::json!({
        "program": "program_v04",
        "program_id": PROGRAM_ID,
        "so_sha256": so_sha256,
        "mollusk_svm": MOLLUSK_SVM_VERSION,
        "toolchain": TOOLCHAIN,
        "slice": "slice 66: slice-64 chain + full lifecycle chain (evaluate, mark_eligible, submit_signal, select, open_planning, submit_plan, reconcile, validate, declare, commit, record_settlement_evidence, settle, init_reputation, record_outcome) to EVALUATED_OUTCOME with an exact reputation tail (ag(2) 50,000,000 -> 50,000,010); program slice 66 is the custom 256 KiB heap allocator (F6 direction (c)); prior values re-freeze with the 56 CU heap-cost delta of the 256 KiB frame",
        "measured": measured,
        "refusals": refusals,
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
