//! W6.2 (seventh pass) — entrance/exit boundary judgment tests.
//!
//! The second half of the S13 blob-rendering rule (Carlos, 2026-10-24
//! ~02:26): a dispersal slice graduates to its own exit blob only when the
//! recipient itself crosses a boundary — "an exit is declared when money
//! leaves the story, not when it moves." Fixtures here are synthetic
//! shapes; the real-mainnet-route gate item is separate (see
//! `labeled_validation.rs` header) and is NOT claimed by these tests.

use tracer_s13::boundary::{
    entrance_declared, exit_declared, BoundaryDecision, CrossingKind, OnwardHop, OnwardKind,
    SourceInfo,
};

fn hop(to: &str, mint: &str, kind: OnwardKind, persistent: bool, in_cluster: bool) -> OnwardHop {
    OnwardHop {
        to: to.to_string(),
        mint: mint.to_string(),
        kind,
        destination_persistent: persistent,
        destination_in_cluster: in_cluster,
    }
}

fn exit_kind(d: &BoundaryDecision) -> Option<(CrossingKind, usize)> {
    match d {
        BoundaryDecision::Exit { kind, hop_index } => Some((*kind, *hop_index)),
        BoundaryDecision::StillInStory { .. } => None,
    }
}

#[test]
fn burn_of_received_mint_graduates() {
    let d = exit_declared("MINT", &[hop("", "MINT", OnwardKind::Burn, false, false)]);
    assert_eq!(exit_kind(&d), Some((CrossingKind::Burn, 0)));
}

#[test]
fn asset_change_graduates() {
    let d = exit_declared(
        "MINT",
        &[hop("OUT", "OTHER", OnwardKind::AssetChange, false, false)],
    );
    assert_eq!(exit_kind(&d), Some((CrossingKind::AssetChange, 0)));
}

#[test]
fn transfer_to_persistent_outside_destination_graduates() {
    let d = exit_declared(
        "MINT",
        &[hop("HOME", "MINT", OnwardKind::Transfer, true, false)],
    );
    assert_eq!(exit_kind(&d), Some((CrossingKind::PersistentDestination, 0)));
}

#[test]
fn transfer_to_single_use_destination_does_not_graduate() {
    // A destination seen exactly once is a relay hop, not a home: the
    // slice is still in the story.
    let d = exit_declared(
        "MINT",
        &[hop("RELAY", "MINT", OnwardKind::Transfer, false, false)],
    );
    assert!(exit_kind(&d).is_none());
}

#[test]
fn transfer_inside_cluster_does_not_graduate() {
    // Even a persistent destination inside the cluster is the cluster
    // paying itself, not an exit.
    let d = exit_declared(
        "MINT",
        &[hop("CLAN", "MINT", OnwardKind::Transfer, true, true)],
    );
    assert!(exit_kind(&d).is_none());
}

#[test]
fn move_then_burn_graduates_at_the_burn() {
    // The first hop merely moves the slice on; the crossing is the burn.
    let d = exit_declared(
        "MINT",
        &[
            hop("RELAY", "MINT", OnwardKind::Transfer, false, false),
            hop("", "MINT", OnwardKind::Burn, false, false),
        ],
    );
    assert_eq!(exit_kind(&d), Some((CrossingKind::Burn, 1)));
}

#[test]
fn no_onward_activity_is_not_an_exit() {
    let d = exit_declared("MINT", &[]);
    assert!(exit_kind(&d).is_none());
}

#[test]
fn off_cluster_deposit_graduates() {
    let d = exit_declared(
        "MINT",
        &[hop("VAULT", "MINT", OnwardKind::OffClusterDeposit, false, false)],
    );
    assert_eq!(exit_kind(&d), Some((CrossingKind::OffClusterDeposit, 0)));
}

#[test]
fn burn_of_other_mint_is_not_this_slice_exiting() {
    // Burning an unrelated mint does not end this slice's story.
    let d = exit_declared("MINT", &[hop("", "OTHER", OnwardKind::Burn, false, false)]);
    assert!(exit_kind(&d).is_none());
}

#[test]
fn entrance_requires_persistent_unrelated_source() {
    assert!(entrance_declared(&SourceInfo { persistent: true, in_cluster: false }));
    assert!(!entrance_declared(&SourceInfo { persistent: false, in_cluster: false }));
    assert!(!entrance_declared(&SourceInfo { persistent: true, in_cluster: true }));
    assert!(!entrance_declared(&SourceInfo { persistent: false, in_cluster: true }));
}
