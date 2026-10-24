//! The venue recognition dictionary: the S11 amalgamated-IDL surface as data.
//!
//! Source: the instruction-family names parsed from
//! `amalgamated_liquidity_stack{,_v2,_v3}.json` (33 / 14 / 56 instructions;
//! analyses at `files/IDL_ALIGNMENT_ANALYSIS.md` and
//! `files/IDL_V3_ALIGNMENT_ANALYSIS.md`). v3 is the most complete form of the
//! dictionary per Carlos's 2026-10-22 correction — a reference example, not a
//! foundation — which is exactly what a *recognition* dictionary needs: names
//! of things venues do, not their authority model.
//!
//! Matching is two-tier, and the confidence reflects it:
//!
//! * **Pinned program id + family** → 0.95. The operator pins a program id
//!   after observing a real deployment; the dictionary never ships with
//!   program ids from memory.
//! * **Family name only** → 0.70. Enough to *suggest* a venue, and exactly at
//!   [`crate::classify::CONFIDENCE_THRESHOLD`] — a family-only match can
//!   never carry a classification on its own; a second signal is required.

/// A venue family on the S11 surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Venue {
    /// Jupiter-style aggregator routing: `swap`, `twoHopSwap`, `dynamicRoute`,
    /// `routeV2`, `proxySwap`, `swapWithDestination`, `executeHybridSwapRoute`.
    AggregatorRoute,
    /// Concentrated-liquidity / AMM positions (Raydium CLMM, Meteora DLMM/DAMM
    /// in the v1 surface): pool + position + bundle instructions.
    ClmmLiquidity,
    /// Orderbook / escrow orders: open/fill/close/expire, match, rebalance.
    EscrowOrder,
    /// v3's rotational-exit machinery: exit schedules, controlled exits,
    /// rotational extraction slots.
    ControlledExit,
    /// Our own program (W4.1 design): enroll, pay_tranche, borrow,
    /// settle_default, open_window, exit, schedule_whale_exit. Recognized so
    /// the tracer can audit our own treasury flows as workflows (W4.4
    /// dogfood) — pinned program id only; family names are known at design
    /// time, the deployed address is not.
    TreasuryProgram,
    /// Fee / operator / config plumbing. Recognized so fee extraction is
    /// labeled as plumbing, never mistaken for position-taking.
    ProtocolOps,
}

/// What a single recognized hop did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HopKind {
    SwapLeg,
    LiquidityAdd,
    LiquidityRemove,
    FeeCollect,
    OrderOpen,
    OrderFill,
    OrderCancel,
    EscrowExit,
    TreasuryFlow,
    Admin,
}

/// One dictionary entry: an instruction family and what it means.
#[derive(Debug, Clone, Copy)]
pub struct VenueEntry {
    pub program_id: Option<&'static str>,
    pub family: &'static str,
    pub venue: Venue,
    pub hop: HopKind,
}

/// A successful dictionary lookup.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VenueHit {
    pub venue: Venue,
    pub hop: HopKind,
    /// 0.95 on a pinned program-id match, 0.70 on family-name only.
    pub confidence: f64,
    pub pinned: bool,
}

/// The recognition dictionary. Built once; `recognize` is a linear scan —
/// the table is small (tens of families) and this is an analytical layer,
/// not a hot path.
#[derive(Debug)]
pub struct Dictionary {
    entries: Vec<VenueEntry>,
}

impl Dictionary {
    pub fn new(entries: Vec<VenueEntry>) -> Self {
        Dictionary { entries }
    }

    /// Recognize `(program_id, instruction_family)`. Prefers a pinned
    /// program-id match; falls back to family-name matching.
    pub fn recognize(&self, program_id: &str, instruction: &str) -> Option<VenueHit> {
        let mut family_only: Option<VenueHit> = None;
        for e in &self.entries {
            if e.family != instruction {
                continue;
            }
            if let Some(pin) = e.program_id {
                if pin == program_id {
                    return Some(VenueHit {
                        venue: e.venue,
                        hop: e.hop,
                        confidence: 0.95,
                        pinned: true,
                    });
                }
            }
            if family_only.is_none() {
                family_only = Some(VenueHit {
                    venue: e.venue,
                    hop: e.hop,
                    confidence: 0.70,
                    pinned: false,
                });
            }
        }
        family_only
    }
}

macro_rules! entry {
    ($venue:expr, $hop:expr, $family:expr) => {
        VenueEntry { program_id: None, family: $family, venue: $venue, hop: $hop }
    };
}

/// The S11-derived table. Families are copied verbatim from the parsed IDL
/// instruction names (v1 33 + v3 23 new).
pub static VENUE_DICTIONARY: &[VenueEntry] = &[
    // AggregatorRoute (Jupiter-style routing in the v1 surface).
    entry!(Venue::AggregatorRoute, HopKind::SwapLeg, "swap"),
    entry!(Venue::AggregatorRoute, HopKind::SwapLeg, "swapWithDestination"),
    entry!(Venue::AggregatorRoute, HopKind::SwapLeg, "twoHopSwap"),
    entry!(Venue::AggregatorRoute, HopKind::SwapLeg, "dynamicRoute"),
    entry!(Venue::AggregatorRoute, HopKind::SwapLeg, "routeV2"),
    entry!(Venue::AggregatorRoute, HopKind::SwapLeg, "proxySwap"),
    entry!(Venue::AggregatorRoute, HopKind::SwapLeg, "executeHybridSwapRoute"),
    // ClmmLiquidity (Raydium CLMM / Meteora DLMM/DAMM in the v1 surface).
    entry!(Venue::ClmmLiquidity, HopKind::LiquidityAdd, "initializePool"),
    entry!(Venue::ClmmLiquidity, HopKind::LiquidityAdd, "initializePoolV2"),
    entry!(Venue::ClmmLiquidity, HopKind::LiquidityAdd, "openPosition"),
    entry!(Venue::ClmmLiquidity, HopKind::LiquidityRemove, "closePosition"),
    entry!(Venue::ClmmLiquidity, HopKind::LiquidityAdd, "increaseLiquidity"),
    entry!(Venue::ClmmLiquidity, HopKind::LiquidityRemove, "decreaseLiquidity"),
    entry!(Venue::ClmmLiquidity, HopKind::FeeCollect, "collectFees"),
    entry!(Venue::ClmmLiquidity, HopKind::LiquidityAdd, "initializePositionBundle"),
    entry!(Venue::ClmmLiquidity, HopKind::LiquidityAdd, "openBundledPosition"),
    entry!(Venue::ClmmLiquidity, HopKind::LiquidityRemove, "deletePositionBundle"),
    entry!(Venue::ClmmLiquidity, HopKind::Admin, "initializeVirtualPool"),
    entry!(Venue::ClmmLiquidity, HopKind::Admin, "migratePool"),
    // EscrowOrder (orderbook surface, v1 + v3).
    entry!(Venue::EscrowOrder, HopKind::OrderOpen, "openOrder"),
    entry!(Venue::EscrowOrder, HopKind::OrderFill, "fillOrder"),
    entry!(Venue::EscrowOrder, HopKind::OrderCancel, "closeOrder"),
    entry!(Venue::EscrowOrder, HopKind::OrderCancel, "expireOrder"),
    entry!(Venue::EscrowOrder, HopKind::OrderOpen, "placeOrder"),
    entry!(Venue::EscrowOrder, HopKind::OrderCancel, "cancelOrder"),
    entry!(Venue::EscrowOrder, HopKind::OrderFill, "matchOrders"),
    entry!(Venue::EscrowOrder, HopKind::OrderFill, "matchStandardOrders"),
    entry!(Venue::EscrowOrder, HopKind::Admin, "rebalanceOrderbook"),
    entry!(Venue::EscrowOrder, HopKind::Admin, "initializeOrderbookPool"),
    // ControlledExit (v3 rotational-exit machinery).
    entry!(Venue::ControlledExit, HopKind::EscrowExit, "registerExitSchedule"),
    entry!(Venue::ControlledExit, HopKind::EscrowExit, "updateExitSchedule"),
    entry!(Venue::ControlledExit, HopKind::EscrowExit, "depositExitInventory"),
    entry!(Venue::ControlledExit, HopKind::EscrowExit, "scheduleControlledExit"),
    entry!(Venue::ControlledExit, HopKind::EscrowExit, "processRotationalExtractionSlot"),
    entry!(Venue::ControlledExit, HopKind::OrderCancel, "cancelControlledExit"),
    entry!(Venue::ControlledExit, HopKind::OrderCancel, "expireControlledExit"),
    entry!(Venue::ControlledExit, HopKind::Admin, "advanceExitPeriod"),
    entry!(Venue::ControlledExit, HopKind::OrderCancel, "cancelExitSchedule"),
    entry!(Venue::ControlledExit, HopKind::Admin, "advanceMarketPeriod"),
    // ProtocolOps (fee / operator / config plumbing).
    entry!(Venue::ProtocolOps, HopKind::FeeCollect, "transferFee"),
    entry!(Venue::ProtocolOps, HopKind::FeeCollect, "withdrawFees"),
    entry!(Venue::ProtocolOps, HopKind::FeeCollect, "collectProtocolFees"),
    entry!(Venue::ProtocolOps, HopKind::Admin, "setFeeRate"),
    entry!(Venue::ProtocolOps, HopKind::Admin, "setProtocolFeeRate"),
    entry!(Venue::ProtocolOps, HopKind::Admin, "createOperatorAccount"),
    entry!(Venue::ProtocolOps, HopKind::Admin, "closeOperatorAccount"),
    entry!(Venue::ProtocolOps, HopKind::Admin, "updateConfig"),
    entry!(Venue::ProtocolOps, HopKind::Admin, "initializeConfig"),
    entry!(Venue::ProtocolOps, HopKind::Admin, "initializeMarketPolicy"),
    entry!(Venue::ProtocolOps, HopKind::Admin, "updateMarketPolicy"),
    entry!(Venue::ProtocolOps, HopKind::Admin, "setMarketPaused"),
    entry!(Venue::ProtocolOps, HopKind::Admin, "createTokenLedger"),
    entry!(Venue::ProtocolOps, HopKind::Admin, "setTokenLedger"),
    entry!(Venue::ProtocolOps, HopKind::Admin, "wrapSol"),
    entry!(Venue::ProtocolOps, HopKind::Admin, "unwrapSol"),
    entry!(Venue::ProtocolOps, HopKind::Admin, "createReferralTokenAccountIdempotent"),
    // TreasuryProgram (our own W4.1 design — families known, address pinned later).
    entry!(Venue::TreasuryProgram, HopKind::TreasuryFlow, "enroll"),
    entry!(Venue::TreasuryProgram, HopKind::TreasuryFlow, "pay_tranche"),
    entry!(Venue::TreasuryProgram, HopKind::TreasuryFlow, "borrow"),
    entry!(Venue::TreasuryProgram, HopKind::TreasuryFlow, "settle_default"),
    entry!(Venue::TreasuryProgram, HopKind::TreasuryFlow, "open_window"),
    entry!(Venue::TreasuryProgram, HopKind::TreasuryFlow, "exit"),
    entry!(Venue::TreasuryProgram, HopKind::TreasuryFlow, "schedule_whale_exit"),
];

/// The default dictionary: every S11 entry, program ids unpinned.
pub fn default_dictionary() -> Dictionary {
    Dictionary::new(VENUE_DICTIONARY.to_vec())
}
