//! Attestation-timing anomaly detection — Carlos's 2026-10-22 addendum,
//! a first-class detection class (W6.1.2).
//!
//! Carlos's empirical case against incumbent on-chain verification (stated
//! 2026-10-22): "blocks that were verified by an authority before the block
//! existed" and "signature accounts that post dated the thing they were
//! verifying." Attestations whose timing cannot line up with the events they
//! certify are verification as theater — a rubber stamp decoupled from the
//! thing being verified. The kernel-side answer is the design rule:
//! attestation only after the realized outcome exists (settle-after-realized,
//! enforced by the `EvidenceAttestor` in `kernel-policy` and the W4.1 design).
//! This class is the tracer-side half: given a realized event and the
//! attestation that claims to certify it, detect the timing shapes that prove
//! the claim impossible.
//!
//! ## The causal clock, on purpose
//!
//! All comparisons use `(slot, index)` causal ordering — the same clock the
//! ingester sorts by — not wall-clock timestamps. The anomalies here are
//! about *causal impossibility* (certifying something that has not happened
//! yet; a signature whose envelope contradicts its claimed time), which a
//! causal order decides exactly. Wall clocks are attestation content, and
//! content is what gets lied about.
//!
//! ## What is detected
//!
//! * [`AttestationLabel::PreCertification`] — signed before the certified
//!   event existed. Structural impossibility, the verification-theater
//!   pattern Carlos observed. Confidence 0.90.
//! * [`AttestationLabel::HashMismatch`] — the certified hash is not the
//!   realized outcome's hash: settle-after-realized violated on the
//!   evidence. Confidence 0.90.
//! * [`AttestationLabel::PrematureClaim`] — the attestation's *claimed*
//!   certification time predates the realized outcome, even when the
//!   signature itself is later (a backdated "we observed this at X").
//!   Confidence 0.80.
//! * [`AttestationLabel::PostDated`] — the signature actually issued after
//!   its own claimed time: "post dated the thing they were verifying."
//!   Confidence 0.75.
//! * [`AttestationLabel::ValidAttestation`] — timing and hash both align:
//!   signed after the realized outcome, claimed time inside the causal
//!   envelope, hash matches. Confidence 0.80.
//!
//! An attestation whose event is not on record is **not fraud** — it is
//! [`AttestationClassification::Unclassified`]: the machine cannot verify
//! timing it cannot see, and says so. The same unclassified guarantee the
//! workflow classifier enforces ([`crate::classify`]) is enforced here by
//! [`AttestationVerdict::seal`]: no public path to a labeled verdict below
//! [`crate::classify::CONFIDENCE_THRESHOLD`].
//!
//! A wrong "pre-certified" flag is a false accusation — this class ships
//! under the same W6.2 validation gate as the workflow classifiers:
//! labeled routes, including our own swarm's devnet traffic, before any
//! label is user-facing. Wiring attestation reports into `TreasuryTransition`
//! workflow verdicts (a treasury settle whose authorization-context events
//! fail timing) is a later increment.

use crate::classify::CONFIDENCE_THRESHOLD;

/// A causal instant: slot, then index within the slot. Field order is the
/// comparison order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CausalTime {
    pub slot: u64,
    pub index: u32,
}

impl CausalTime {
    pub fn at(slot: u64, index: u32) -> Self {
        CausalTime { slot, index }
    }
}

/// The realized outcome — the thing that must exist BEFORE any honest
/// attestation can certify it. This is the settle-after-realized rule,
/// stated as data.
#[derive(Debug, Clone)]
pub struct RealizedEvent {
    /// Event identifier the attestation references.
    pub id: String,
    /// Content hash of the realized outcome (what an honest attestation
    /// certifies).
    pub hash: String,
    /// Causal time the outcome was realized.
    pub realized_at: CausalTime,
    /// Transaction signature carrying the realized outcome (evidence).
    pub signature: String,
}

/// One attestation record: what is claimed, when it is claimed to have been
/// certified, and when it was actually signed. The *gap between claimed
/// time and actual signing time* is where the theater lives.
#[derive(Debug, Clone)]
pub struct AttestationRecord {
    pub attester: String,
    /// The event this attestation claims to certify.
    pub event_id: String,
    /// The hash the attestation certifies.
    pub certified_hash: String,
    /// The causal time the attestation *claims* to have certified.
    pub certified_at: CausalTime,
    /// The causal time the signature actually issued — the envelope.
    pub signed_at: CausalTime,
    /// The attestation's signature (evidence, kept verbatim).
    pub signature: String,
}

/// The attestation-timing labels, in detector priority order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttestationLabel {
    /// Signed before the certified event existed — certifying an outcome
    /// that had not happened yet. The verification-theater pattern.
    PreCertification,
    /// The certified hash is not the realized outcome's hash.
    HashMismatch,
    /// The claimed certification time predates the realized outcome
    /// (the signature itself need not be early — the claim is).
    PrematureClaim,
    /// Actually signed after its own claimed time — the post-dated
    /// signature pattern.
    PostDated,
    /// Timing and hash both align with settle-after-realized.
    ValidAttestation,
}

/// A verdict on one (event, attestation) pair: a justified label or an
/// explicit guess.
#[derive(Debug, Clone)]
pub enum AttestationClassification {
    Labeled {
        label: AttestationLabel,
        /// Always ≥ CONFIDENCE_THRESHOLD — enforced by
        /// [`AttestationVerdict::seal`].
        confidence: f64,
        /// Why the detector believes it, in one sentence.
        rationale: String,
    },
    Unclassified {
        /// What was tried and why it fell short.
        reason: String,
    },
}

/// The full attestation verdict: classification + evidence + every anomaly
/// finding, even when only one is the primary label.
#[derive(Debug, Clone)]
pub struct AttestationVerdict {
    pub event_id: String,
    pub attester: String,
    pub classification: AttestationClassification,
    /// The underlying signatures: the realized event's, then the
    /// attestation's — the audit appendix.
    pub evidence: Vec<String>,
    /// Every anomaly found, one note each. A pair can be both pre-certified
    /// AND hash-mismatched; the primary label does not hide the rest.
    pub findings: Vec<String>,
}

impl AttestationVerdict {
    /// Seal a candidate into a verdict. **This is the unclassified
    /// guarantee for attestations**, mirroring [`crate::classify::Verdict::seal`]:
    /// a candidate below [`CONFIDENCE_THRESHOLD`] cannot become a label —
    /// it is demoted to Unclassified with the shortfall stated. `None`
    /// (no realized event on record) is never fraud: it seals as
    /// Unclassified with the reason stated. Keep this the only path to a
    /// labeled verdict.
    pub fn seal(
        event_id: String,
        attester: String,
        candidate: Option<(AttestationLabel, f64, String)>,
        findings: Vec<String>,
        evidence: Vec<String>,
    ) -> Self {
        let classification = match candidate {
            Some((label, confidence, rationale))
                if confidence >= CONFIDENCE_THRESHOLD =>
            {
                AttestationClassification::Labeled { label, confidence, rationale }
            }
            Some((label, confidence, rationale)) => AttestationClassification::Unclassified {
                reason: format!(
                    "best candidate {:?} at {:.2}, below the {:.2} threshold ({})",
                    label, confidence, CONFIDENCE_THRESHOLD, rationale
                ),
            },
            None => AttestationClassification::Unclassified {
                reason: format!(
                    "no realized event on record for event id {}; cannot verify \
                     timing — this is not a fraud verdict",
                    event_id
                ),
            },
        };
        AttestationVerdict { event_id, attester, classification, evidence, findings }
    }

    /// True when the verdict is a guess, not a justified label.
    pub fn is_guess(&self) -> bool {
        matches!(
            self.classification,
            AttestationClassification::Unclassified { .. }
        )
    }

    /// The label, if this verdict carries one.
    pub fn label(&self) -> Option<AttestationLabel> {
        match &self.classification {
            AttestationClassification::Labeled { label, .. } => Some(*label),
            AttestationClassification::Unclassified { .. } => None,
        }
    }

    /// The confidence of a labeled verdict. Panics on Unclassified — a
    /// guess has no confidence worth quoting.
    pub fn confidence(&self) -> f64 {
        match &self.classification {
            AttestationClassification::Labeled { confidence, .. } => *confidence,
            AttestationClassification::Unclassified { .. } => {
                panic!("Unclassified attestation verdict has no confidence to quote")
            }
        }
    }
}

/// Render one plain-language line, e.g.
/// "Pre-certification (confidence 0.90) — attester A certified event E
/// before it existed at (slot 1000, index 0)".
pub fn attestation_headline(v: &AttestationVerdict) -> String {
    let label_clause = match &v.classification {
        AttestationClassification::Labeled { label, confidence, .. } => {
            let name = match label {
                AttestationLabel::PreCertification => "Pre-certification",
                AttestationLabel::HashMismatch => "Hash mismatch",
                AttestationLabel::PrematureClaim => "Premature claim",
                AttestationLabel::PostDated => "Post-dated attestation",
                AttestationLabel::ValidAttestation => "Valid attestation",
            };
            format!("{} (confidence {:.2})", name, confidence)
        }
        AttestationClassification::Unclassified { reason } => {
            format!("Unclassified attestation — {}", reason)
        }
    };
    let extra = if v.findings.is_empty() {
        String::new()
    } else {
        format!("; also found: {}", v.findings.join(" | "))
    };
    format!(
        "{} — attester {} on event {}{}{}",
        label_clause,
        v.attester,
        v.event_id,
        extra,
        if v.is_guess() { " — a guess, not a verdict" } else { "" }
    )
}

/// The detectors, in priority order. Each returns at most one candidate
/// (the label, its confidence, its rationale note); `attest` runs them all
/// so the findings appendix is complete, and the primary label is the
/// highest-confidence candidate (ties break to the earlier, more
/// structural, detector).
fn detect(event: &RealizedEvent, rec: &AttestationRecord) -> Vec<(AttestationLabel, f64, String)> {
    let mut out = Vec::new();
    let e = event.realized_at;

    // PreCertification: signed before the event existed. Nothing honest
    // certifies an outcome that has not happened yet.
    if rec.signed_at < e {
        out.push((
            AttestationLabel::PreCertification,
            0.90,
            format!(
                "signed at (slot {}, index {}) before the certified event existed at (slot {}, index {})",
                rec.signed_at.slot, rec.signed_at.index, e.slot, e.index
            ),
        ));
    }

    // HashMismatch: certifies a hash that is not the realized outcome's.
    if rec.certified_hash != event.hash {
        out.push((
            AttestationLabel::HashMismatch,
            0.90,
            format!(
                "certified hash {} is not the realized outcome's hash {}",
                rec.certified_hash, event.hash
            ),
        ));
    }

    // PrematureClaim: the claimed certification time predates the outcome —
    // a backdated "we observed this at X" even when the signature is later.
    if rec.certified_at < e {
        out.push((
            AttestationLabel::PrematureClaim,
            0.80,
            format!(
                "claimed certification time (slot {}, index {}) predates the realized outcome at (slot {}, index {})",
                rec.certified_at.slot, rec.certified_at.index, e.slot, e.index
            ),
        ));
    }

    // PostDated: actually signed after its own claimed time — the envelope
    // contradicts the claim.
    if rec.signed_at > rec.certified_at {
        out.push((
            AttestationLabel::PostDated,
            0.75,
            format!(
                "actually signed at (slot {}, index {}) after the claimed certification time (slot {}, index {})",
                rec.signed_at.slot, rec.signed_at.index, rec.certified_at.slot, rec.certified_at.index
            ),
        ));
    }

    out
}

/// Judge one attestation against its realized event. `event = None` (no
/// realized event on record) seals as Unclassified — never as fraud.
pub fn attest(event: Option<&RealizedEvent>, rec: &AttestationRecord) -> AttestationVerdict {
    let evidence = match event {
        Some(e) => vec![e.signature.clone(), rec.signature.clone()],
        None => vec![rec.signature.clone()],
    };
    let event_id = rec.event_id.clone();
    let attester = rec.attester.clone();

    match event {
        None => AttestationVerdict::seal(event_id, attester, None, Vec::new(), evidence),
        Some(e) => {
            let detected = detect(e, rec);
            let findings: Vec<String> =
                detected.iter().map(|(_, _, rationale)| rationale.clone()).collect();
            // Primary = highest confidence; strict `>` keeps the earlier
            // (more structural) detector on ties.
            let mut primary: Option<(AttestationLabel, f64, String)> = None;
            for (label, conf, rationale) in detected {
                match &primary {
                    Some((_, best, _)) if conf <= *best => {}
                    _ => primary = Some((label, conf, rationale)),
                }
            }
            let candidate = primary.or_else(|| {
                Some((
                    AttestationLabel::ValidAttestation,
                    0.80,
                    format!(
                        "signed at (slot {}, index {}) after the realized outcome at (slot {}, index {}), \
                         claimed time inside the causal envelope, certified hash matches",
                        rec.signed_at.slot,
                        rec.signed_at.index,
                        e.realized_at.slot,
                        e.realized_at.index
                    ),
                ))
            });
            AttestationVerdict::seal(event_id, attester, candidate, findings, evidence)
        }
    }
}
