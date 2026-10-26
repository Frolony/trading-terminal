//! The Rust predicate must match the Solidity predicate. These tests read the Solidity
//! sources in `reference/` and fail if the bit values, their order in the predicate, the
//! classifier tables, the lifecycle numbering or the governance constants drift apart.

use coordination_kernel::*;

fn read(f: &str) -> String {
    std::fs::read_to_string(format!("{}/reference/{}", env!("CARGO_MANIFEST_DIR"), f)).unwrap()
}
fn sol_const(src: &str, name: &str) -> u64 {
    for l in src.lines() {
        let l = l.trim();
        if l.contains(&format!(" {name} ")) && l.contains("constant") && l.contains('=') {
            let rhs = l.split('=').nth(1).unwrap().split("//").next().unwrap().trim().trim_end_matches(';').trim();
            return rhs.parse().unwrap_or_else(|_| panic!("cannot parse {name}: {rhs}"));
        }
    }
    panic!("{name} not found in Solidity")
}

#[test]
fn fail_bits_have_the_same_values() {
    let s = read("KernelStorage.sol");
    for (n, v) in [
        ("F_CONSTRAINTS", FailMask::CONSTRAINTS), ("F_OBJECTIVES", FailMask::OBJECTIVES), ("F_QUORUM", FailMask::QUORUM),
        ("F_CONFIDENCE", FailMask::CONFIDENCE), ("F_COMPAT", FailMask::COMPAT), ("F_EXCEPTIONS", FailMask::EXCEPTIONS),
        ("F_FRESH", FailMask::FRESH), ("F_PLANNING_OPEN", FailMask::PLANNING_OPEN), ("F_EXPIRED", FailMask::EXPIRED),
        ("F_LINEAGE", FailMask::LINEAGE), ("F_PLANS", FailMask::PLANS), ("F_CAPABILITY", FailMask::CAPABILITY), ("F_DIALECT", FailMask::DIALECT),
    ] {
        assert_eq!(sol_const(&s, n), v as u64, "{n}");
    }
}

/// The terms are tested in the same order, and no term exists on one side only.
#[test]
fn the_predicate_has_the_same_terms_in_the_same_order() {
    let sol = read("CoordinationKernel.sol");
    let body = sol.split("function commitFailMask").nth(1).unwrap().split("/// VALIDATED").next().unwrap();
    let sol_terms: Vec<String> = body.lines().filter_map(|l| l.split("m |= F_").nth(1).map(|r| r.trim_end_matches(';').trim().to_string())).collect();
    let rs = std::fs::read_to_string(format!("{}/src/kernel.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let rbody = rs.split("pub fn commit_fail_mask").nth(1).unwrap().split("/// Validated -> Committable").next().unwrap();
    let rs_terms: Vec<String> = rbody.lines().filter_map(|l| l.split("m |= FailMask::").nth(1).map(|r| r.split(';').next().unwrap().trim().to_string())).collect();
    assert_eq!(sol_terms.len(), 13);
    assert_eq!(sol_terms, rs_terms);
}

#[test]
fn classifier_tables_match_all_48_cells() {
    let s = read("ConsensusModule.sol");
    let hex = |name: &str| -> u128 {
        let l = s.lines().find(|l| l.contains(name) && l.contains("0x")).unwrap();
        u128::from_str_radix(l.split("0x").nth(1).unwrap().trim_end_matches(';').trim(), 16).unwrap()
    };
    // the sector table is 24 nibbles and the weight table 24 bytes: both fit in u128/u256 halves
    let sector = hex("SECTOR_TABLE");
    let w_hex = s.lines().find(|l| l.contains("WEIGHT_TABLE") && l.contains("0x")).unwrap().split("0x").nth(1).unwrap().trim_end_matches(';').trim().to_string();
    assert_eq!(w_hex.len(), 48);
    let stances = [Stance::Aligned, Stance::Opposed, Stance::Neutral, Stance::Uncertain];
    let contribs = [Contribution::Substantive, Contribution::Procedural, Contribution::Informational, Contribution::Obstructive, Contribution::Abstaining, Contribution::Specialist];
    for (si, st) in stances.iter().enumerate() {
        for (ci, c) in contribs.iter().enumerate() {
            let idx = si * 6 + ci;
            assert_eq!(((sector >> (idx * 4)) & 0xf) as u8, sector_of(*st, *c) as u8, "sector cell {idx}");
            // 8 bits per cell, cell 0 in the lowest byte, i.e. the right-most two hex digits
            let pos = 48 - (idx + 1) * 2;
            let twentieths = u64::from_str_radix(&w_hex[pos..pos + 2], 16).unwrap();
            assert_eq!(Amount::from_raw(twentieths * 1_000_000 / 20), base_weight_of(*st, *c), "weight cell {idx}");
        }
    }
}

#[test]
fn lifecycle_numbering_matches() {
    let s = read("KernelStorage.sol");
    let body = s.split("enum TransitionLifecycle {").nth(1).unwrap().split('}').next().unwrap();
    let names: Vec<String> = body.lines().filter_map(|l| {
        let l = l.trim();
        let name = l.split([',', ' ']).next()?;
        if name.is_empty() || name.starts_with("//") { None } else { Some(name.to_string()) }
    }).collect();
    let n = |x: &str| names.iter().position(|y| y == x).unwrap() as u8;
    assert_eq!(n("PROPOSED"), Life::Proposed as u8);
    assert_eq!(n("EVALUATED"), Life::Evaluated as u8);
    assert_eq!(n("ELIGIBLE"), Life::Eligible as u8);
    assert_eq!(n("SELECTED"), Life::Selected as u8);
    assert_eq!(n("PLANNING"), Life::Planning as u8);
    assert_eq!(n("RECONCILING"), Life::Reconciling as u8);
    assert_eq!(n("VALIDATED"), Life::Validated as u8);
    assert_eq!(n("COMMITTABLE"), Life::Committable as u8);
    assert_eq!(n("COMMITTED"), Life::Committed as u8);
    assert_eq!(n("EXECUTING"), Life::Executing as u8);
    assert_eq!(n("SETTLED"), Life::Settled as u8);
    assert_eq!(n("EVALUATED_OUTCOME"), Life::EvaluatedOutcome as u8);
    assert_eq!(n("REJECTED"), Life::Rejected as u8);
    assert_eq!(n("EXPIRED"), Life::Expired as u8);
    assert_eq!(n("CONFLICTED"), Life::Conflicted as u8);
    assert_eq!(n("DISCARDED"), Life::Discarded as u8);
    assert_eq!(n("BLOCKED"), Life::Blocked as u8);
}

#[test]
fn governance_constants_match() {
    let s = read("KernelStorage.sol");
    assert_eq!(sol_const(&s, "MIN_SITTING_EPOCHS"), MIN_SITTING_EPOCHS);
    assert_eq!(sol_const(&s, "POSITION_EPOCHS"), POSITION_EPOCHS);
    assert_eq!(sol_const(&s, "DEFECTION_COOLDOWN_EPOCHS"), DEFECTION_COOLDOWN_EPOCHS);
    assert_eq!(sol_const(&s, "MAX_SIBLINGS"), MAX_SIBLINGS as u64);
    assert_eq!(sol_const(&s, "MAX_REPLANS"), MAX_REPLANS as u64);
    assert_eq!(sol_const(&s, "MAX_SEATS"), MAX_SEATS as u64);
    assert_eq!(sol_const(&s, "MAX_DIALECT_DEPS"), MAX_DIALECT_DEPS as u64);
    assert_eq!(sol_const(&s, "MAX_EXCEPTIONS"), MAX_EXCEPTIONS as u64);
    assert_eq!(sol_const(&s, "MAX_PLANS_HARD"), MAX_PLANS_HARD as u64);
    assert_eq!(sol_const(&s, "MAX_GROUPS"), MAX_GROUPS as u64);
    assert!(s.contains("PAUSE_MAX = 3 days") && PAUSE_MAX == 3 * 86_400);
    for n in ["GOV_ADJUSTABLE", "GOV_CONSTITUTION", "GOV_COUNCIL", "GOV_REBOOT"] {
        assert!(s.contains(&format!("keccak256(\"{n}\")")), "{n}");
    }
}

/// The nine notations recorded at genesis are the same nine, in the same order.
#[test]
fn genesis_notations_match() {
    let sol = read("CoordinationKernel.sol");
    let body = sol.split("function _genesisNotations").nth(1).unwrap().split("// ------").next().unwrap();
    let pairs: Vec<(String, String)> = body.split("_genesisNotation(").skip(1).map(|c| {
        let sym = |l: &str| l.split("keccak256(\"").nth(1).unwrap().split('"').next().unwrap().to_string();
        let mut it = c.lines().filter(|l| l.contains("keccak256(\""));
        let first = it.next().unwrap();
        let o = sym(first);
        let n = first.split("keccak256(\"").nth(2).map(|x| x.split('"').next().unwrap().to_string()).unwrap();
        (o, n)
    }).collect();
    assert_eq!(pairs.len(), 9);
    let k = {
        use std::collections::BTreeSet;
        let cfg = Config { rep_min: Amount::ZERO, rep_max: Amount::units(2000).unwrap(), max_attribution: Amount::ONE, epoch_length: 100, council_seats: 3, reputation_weighs: true };
        let _ = BTreeSet::<u8>::new();
        Kernel::new([1; 32], SystemState { index: 0, hash: [0; 32], timestamp: 0 }, cfg, NoEnv).unwrap()
    };
    assert_eq!(k.notation_count(), 9);
    for (i, (o, n)) in pairs.iter().enumerate() {
        let c = k.notation(i as u64).unwrap();
        assert_eq!(c.old_symbol, tag(o.as_bytes()), "old {i}");
        assert_eq!(c.new_symbol, tag(n.as_bytes()), "new {i}");
        assert_eq!(c.status, NotationStatus::Committed);
    }
}

/// Discarding a blocked transition: Blocked-only, strict majority of all seats, votes per blocking episode.
#[test]
fn discard_rule_matches() {
    let sol = read("ConsensusModule.sol");
    let body = sol.split("function discardBlocked").nth(1).expect("discardBlocked exists in Solidity");
    assert!(body.contains("TransitionLifecycle.BLOCKED"), "Blocked only");
    assert!(body.contains("++discardVotes[key] * 2 <= _councilMembers.length"), "strict majority of all seats");
    assert!(body.contains("blockEpisode[id]"), "votes are per blocking episode");
    assert!(body.contains("TransitionLifecycle.DISCARDED"));
    let rs = std::fs::read_to_string(format!("{}/src/kernel.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let rb = rs.split("pub fn discard_blocked").nth(1).unwrap();
    assert!(rb.contains("Life::Blocked") && rb.contains("voters.len() * 2 <= self.council.len()") && rb.contains("block_episode") && rb.contains("Life::Discarded"));
}
