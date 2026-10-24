//! The headline: one plain-language line per workflow.
//!
//! The net-effect summary comes first, always — the label second, the raw
//! transaction links never (they live in [`crate::classify::Verdict::evidence`],
//! the audit appendix). Plumbing is visually demoted by construction: a
//! workflow whose intermediaries kept nothing says so in its first clause.

use crate::classify::{Classification, Label, Verdict};

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", n, if n == 1 { one } else { many })
}

/// Render the verdict as a headline, e.g.
/// "Aggregator split — 14 hops, 3 venues, 12 pass-through, no net position kept".
pub fn headline(v: &Verdict) -> String {
    let n_hops = v.evidence.len();
    let n_wallets = v.net.wallet_count;
    let fees = v.net.total_fees_lamports;
    let (s0, s1) = v.net.slot_span;
    let span = if s1 > s0 { format!(", {} slots", s1 - s0) } else { String::new() };

    // The net-effect lead: did anyone take a real position, and keep it?
    let mut kept: Vec<String> = Vec::new();
    for (w, m, n) in &v.net.net_per_wallet_mint {
        if *n != 0 && !v.net.passthrough_wallets.iter().any(|p| p == w) {
            kept.push(format!("{} net {:+} {}", short(w), n, short(m)));
        }
    }
    kept.sort();
    kept.dedup();
    let net_clause = if kept.is_empty() {
        "no net position kept".to_string()
    } else if kept.len() <= 3 {
        format!("kept: {}", kept.join("; "))
    } else {
        format!("{} wallets kept net positions", kept.len())
    };

    let shape = format!(
        "{} across {}",
        plural(n_hops, "hop", "hops"),
        plural(n_wallets, "wallet", "wallets")
    );

    let label_clause = match &v.classification {
        Classification::Labeled { label, confidence, .. } => {
            let name = match label {
                Label::AggregatorSplit => "Aggregator split",
                Label::LiquidityAddRemove => "Liquidity add/remove",
                Label::WashLoop => "Wash loop (manufactured movement)",
                Label::BridgeHop => "Bridge hop",
                Label::PositionOpen => "Position opened",
                Label::PositionClose => "Position closed",
                Label::TreasuryTransition => "Treasury transition",
                Label::CrossMintRoundTrip => "Cross-mint round trip (completed swap)",
                Label::MarketMaker => "Market maker (inventory cycling)",
                Label::IncentiveFarming => "Incentive farming (position + harvest)",
            };
            format!("{} (confidence {:.2})", name, confidence)
        }
        Classification::Unclassified { reason } => {
            format!("Unclassified movement — {}", reason)
        }
    };

    let pass = if v.net.passthrough_wallets.is_empty() {
        String::new()
    } else {
        format!(", {} pass-through", plural(v.net.passthrough_wallets.len(), "hop-wallet", "hop-wallets"))
    };

    format!(
        "{} — {}{}{}, {}, fees {} lamports{}",
        label_clause, shape, pass, span, net_clause, fees,
        if v.is_guess() { " — a guess, not a verdict" } else { "" }
    )
}

fn short(s: &str) -> &str {
    if s.len() > 12 {
        &s[..12]
    } else {
        s
    }
}
