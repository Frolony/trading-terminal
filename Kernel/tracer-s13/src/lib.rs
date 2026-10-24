//! # tracer-s13 — the workflow legibility layer (W6.1)
//!
//! Carlos's framing (2026-10-22): the chain's number-one problem is illegibility.
//! A normal human should never have to click through thousands of wallet links
//! hunting "alpha" that turns out to be a meaningless aggregator workflow.
//! This crate is the first build increment of the S13 workflow tracer:
//!
//! 1. **Route reconstruction** ([`reconstruct`]): ingest transfer events for a
//!    wallet/token/time window and stitch hops into end-to-end workflows,
//!    using the amalgamated IDL's venue-surface map (S11, v1–v3) as the
//!    recognition dictionary ([`dictionary`]). A circular flow is collapsed
//!    into ONE workflow, not dozens of impressive-looking hops.
//! 2. **Classification with confidence** ([`classify`]): aggregator split,
//!    liquidity add/remove, wash/self-routing loop, position open/close,
//!    treasury transition, cross-mint round trip, market-maker inventory,
//!    incentive farming. Every label ships with its evidence (the underlying
//!    event signatures) and a confidence value.
//! 3. **The unclassified guarantee** ([`classify`]): anything below
//!    [`classify::CONFIDENCE_THRESHOLD`] renders as "unclassified movement",
//!    never as a verdict. The project's claims discipline applies to the
//!    machine too: an inference it cannot justify is printed as a guess, not
//!    a fact. The constructor enforces this — there is no public path to a
//!    labeled verdict below the threshold.
//! 4. **Net-effect summary first** ([`summary`]): every workflow leads with
//!    its net position change, total fees paid en route, time span, and wallet
//!    count. Plumbing (zero net position change) is demoted below real
//!    position-taking: the headline answers *did anyone take a real position,
//!    and did they keep it?*
//! 5. **Attestation-timing anomaly detection** ([`attestation`], Carlos's
//!    2026-10-22 addendum): given a realized event and the attestation that
//!    claims to certify it, detect the timing shapes that prove the claim
//!    impossible — signed before the event existed (pre-certification),
//!    claimed time before the event (premature claim), actually signed after
//!    the claimed time (post-dated), or a certified hash that is not the
//!    realized outcome's. The unclassified guarantee applies here too: no
//!    realized event on record means no verdict, not fraud.
//!
//! ## Deliberate limits of this increment
//!
//! * Stitching is a greedy single pass: an event appends to the most recent
//!   open workflow whose last hop's `to` equals the event's `from`. Fan-out
//!   splits (one wallet paying many) each open their own workflow; a full DAG
//!   assembler is a later increment.
//! * The dictionary matches instruction-family names from the S11 IDL
//!   analyses. Program ids are an *optional* pin: a match on a pinned program
//!   id scores higher than a family-name-only match. No mainnet program ids
//!   are baked in here — they must be pinned from observed deployments, not
//!   from memory.
//! * The attestation-timing detection class (Carlos's 2026-10-22 addendum)
//!   landed as a first-class class in W6.1.2 ([`attestation`]); the
//!   incentive-farming / market-maker / cross-mint-round-trip classes landed
//!   in W6.1.3 ([`classify`]). Labeled-route validation against known ground
//!   truth (incl. our own devnet swarm traffic) still gates any user-facing
//!   claim (W6.2).
//! * This is inference machinery with a published error posture, not a
//!   fact source. A wrong "wash" label is a false accusation — validation
//!   against known labeled routes precedes any user-facing claim (W6.2 gate).

pub mod dictionary;
pub mod ingest;
pub mod reconstruct;
pub mod classify;
pub mod summary;
pub mod attestation;

pub use dictionary::{default_dictionary, Dictionary, HopKind, Venue, VenueHit, VENUE_DICTIONARY};
pub use ingest::TransferEvent;
pub use reconstruct::{NetEffect, Workflow, reconstruct};
pub use classify::{Classification, Label, Verdict, CONFIDENCE_THRESHOLD};
pub use summary::headline;
pub use attestation::{
    attest, attestation_headline, AttestationClassification, AttestationLabel,
    AttestationRecord, AttestationVerdict, CausalTime, RealizedEvent,
};
