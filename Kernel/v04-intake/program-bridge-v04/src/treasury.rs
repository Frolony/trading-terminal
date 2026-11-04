//! Migration step 5, slice 2: treasury instructions (§4.2/§4.3) and the
//! program account inventory (§3) over the slice-1 bridge.
//!
//! Each treasury instruction is a typed constructor over a slice-1
//! `RecordedTransition`, replayed through the full v0.4 lifecycle
//! (evaluate, eligible, select, reconcile, validate, declare) on a fresh
//! sealed launch kernel. The mapping is the §4 authorization rule made
//! executable on the host: **every value-moving instruction names exactly
//! one kernel stock, and a tranche that fails the predicate does not pay**
//! — the same rule slice 1 proved for the generic record, now per
//! instruction:
//!
//! * `pay_tranche` claims `RES_TREASURY_TRANCHE` (the month's cohort slot).
//! * `borrow` claims `RES_LENDING_BOOK` (60-day terms at pilot; the book
//!   cap is the math doc's T/12, priced by the engine, not by this host
//!   fixture's 500-unit stock).
//! * `exit` and `execute_whale_tranche` claim `RES_EXIT_WINDOW` — the
//!   per-period aggregate bound: the window's exit stock is debited at
//!   commit, so repeated sub-cap orders in one period cannot drain more
//!   than the stock, which is the D-10 hard bound in stock form.
//! * `reinvest`, `settle_default`, `collect_fees`, and `open_window`
//!   claim no stock: reinvest increases a position without cash,
//!   settle_default moves value position-internally (principal settles
//!   against the locked position; interest forgiven), collect_fees
//!   reconciles the FeeVault against a declared total, and open_window
//!   is the authorizing window transition itself.
//!
//! Honest limits of this host slice (the program rewrite carries the rest):
//!
//! * Host replays reuse slice 1's fixture transition type (`TT`); the
//!   on-chain `TransitionRecord` will carry `treasury.window.v1` for the
//!   window family and `treasury.exit.v1` for whale schedules (see
//!   `AccountEntry::transition_type`). The type names are documented here,
//!   not yet enforced by the host replay.
//! * The D-10 program flag (>= T/30, 20%-of-position weekly blocks capped
//!   at FLAG, 12-month lock) is program logic, not host-fixture logic:
//!   the host proves the stock bound (a claim above the exit stock
//!   blocks), not the flag arithmetic. Flag pricing lives in the v0.2
//!   program slices and the engine; it is not re-priced against the
//!   250-unit fixture stock.
//! * The extraction penalty door is engine-priced (D-4: FLAT 13% as ruled
//!   2026-10-26; the 12% loan rate is a separate parameter and is
//!   unchanged). No penalty figure is asserted by this slice's replays.
//! * `settle_default` records the D-3 pinned forfeiture base
//!   (`PreDefault`); the post-default figure stays a rejected
//!   counterfactual in the engine, not a second path here.

use crate::{RecordedTransition, ReplayOutcome, replay_transition_full};
use coordination_kernel::*;
use kernel_launch_v04::*;
use std::collections::BTreeSet;

/// Dividend-forfeiture base on default. D-3 CLOSED (Carlos, 2026-10-23):
/// PRE-DEFAULT — loans are drawn from the treasury cash sleeve, so the
/// position standing before default settlement is the full position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForfeitureBase {
    /// Pinned by D-3. The only base this slice constructs.
    PreDefault,
    /// Rejected counterfactual (engine T10 prices it at $3.13). Never
    /// constructed by a settle_default record here; kept so a reader can
    /// see the rejected branch is named, not silently dropped.
    #[allow(dead_code)]
    PostDefault,
}

/// One treasury instruction (§4.2/§4.3), as a bridge record constructor.
/// Amounts are whole units in the launch fixture's scale (`amt`).
#[derive(Debug, Clone, Copy)]
pub enum TreasuryInstruction {
    /// Pay the month's cohort its slot dividend. Claims the tranche stock.
    PayTranche { amount: f64 },
    /// Issue a loan against a locked position. Claims the lending book.
    Borrow { amount: f64 },
    /// Holder exit within allowance. Claims the exit-window stock.
    Exit { amount: f64 },
    /// Operator executes one whale-schedule tranche. Claims the
    /// exit-window stock; a tranche above the stock blocks (D-10 bound).
    ExecuteWhaleTranche { amount: f64 },
    /// Extract-or-reinvest window rule, reinvest branch. Claims no cash.
    Reinvest,
    /// Default settlement: principal against the position, interest
    /// forgiven, next-slot dividend forfeited on the pre-default base.
    SettleDefault { forfeited_base: ForfeitureBase },
    /// Reconcile the FeeVault against the window's declared collected
    /// total (73 bps single flow, D-5). Claims no kernel stock.
    CollectFees { declared_total: f64 },
    /// Open the next cohort window (D-11: next-window-only entry; there
    /// is no mid-window entry). The authorizing transition itself.
    OpenWindow { window_id: u64 },
}

impl TreasuryInstruction {
    /// The single kernel stock this instruction debits, or `None` for the
    /// instructions that move no kernel stock. Value-moving instructions
    /// must return `Some`; that is the §4 authorization rule, tested.
    pub fn stock(&self) -> Option<ResourceId> {
        match self {
            TreasuryInstruction::PayTranche { .. } => Some(RES_TREASURY_TRANCHE),
            TreasuryInstruction::Borrow { .. } => Some(RES_LENDING_BOOK),
            TreasuryInstruction::Exit { .. }
            | TreasuryInstruction::ExecuteWhaleTranche { .. } => Some(RES_EXIT_WINDOW),
            TreasuryInstruction::Reinvest
            | TreasuryInstruction::SettleDefault { .. }
            | TreasuryInstruction::CollectFees { .. }
            | TreasuryInstruction::OpenWindow { .. } => None,
        }
    }

    /// True iff this instruction moves value out of a kernel stock.
    pub fn is_value_moving(&self) -> bool {
        self.stock().is_some()
    }

    /// Build the slice-1 record this instruction replays. Claiming
    /// instructions use the ordinary constructor; non-claiming ones use
    /// the same record with its plan claims cleared (an empty-claims plan,
    /// the form governance replays already proved commit-capable).
    pub fn to_record(&self, id: TransitionId) -> RecordedTransition {
        match self {
            TreasuryInstruction::PayTranche { amount }
            | TreasuryInstruction::Borrow { amount }
            | TreasuryInstruction::Exit { amount }
            | TreasuryInstruction::ExecuteWhaleTranche { amount } => {
                let stock = self.stock().expect("claiming instruction names a stock");
                RecordedTransition::ordinary(id, stock, amt(*amount))
            }
            _ => {
                let mut record =
                    RecordedTransition::ordinary(id, RES_TREASURY_TRANCHE, amt(0.0));
                record.plan_claims.clear();
                record
            }
        }
    }

    /// Replay this instruction from its record alone, keeping the rebuilt
    /// kernel so callers can assert post-replay stocks and head.
    pub fn replay(
        &self,
        id: TransitionId,
    ) -> Result<(ReplayOutcome, LaunchKernel), KernelError> {
        replay_transition_full(&self.to_record(id))
    }
}

/// One §3 program account, mapped to its kernel stock (if any) and its
/// authorizing transition type. This is the host analogue of the Anchor
/// account inventory: it names what the program rewrite must carry, and
/// lets tests prove the inventory covers every treasury instruction's
/// stock. Accounts that hold no kernel stock (records, vaults, state)
/// carry `stock: None` honestly rather than borrowing a stock id.
#[derive(Debug, Clone, Copy)]
pub struct AccountEntry {
    pub name: &'static str,
    pub design_section: &'static str,
    pub stock: Option<ResourceId>,
    pub transition_type: &'static str,
}

/// The §3 inventory, in design order. `transition_type` is the on-chain
/// record type the program rewrite will carry; host replays use the
/// slice-1 fixture type (see the module docs).
pub const ACCOUNT_INVENTORY: &[AccountEntry] = &[
    AccountEntry {
        name: "TreasuryState",
        design_section: "§3",
        stock: None,
        transition_type: "treasury.launch.v1",
    },
    AccountEntry {
        name: "WindowState",
        design_section: "§3",
        stock: None,
        transition_type: "treasury.window.v1",
    },
    AccountEntry {
        name: "HolderRecord",
        design_section: "§3",
        stock: None,
        transition_type: "treasury.window.v1",
    },
    AccountEntry {
        name: "LendingBook",
        design_section: "§3",
        stock: Some(RES_LENDING_BOOK),
        transition_type: "treasury.window.v1",
    },
    AccountEntry {
        name: "ExitSchedule",
        design_section: "§3",
        stock: Some(RES_EXIT_WINDOW),
        transition_type: "treasury.exit.v1",
    },
    AccountEntry {
        name: "FeeVault",
        design_section: "§3",
        stock: None,
        transition_type: "treasury.window.v1",
    },
    AccountEntry {
        name: "TransitionRecord",
        design_section: "§3/§4.1",
        stock: None,
        transition_type: "treasury.window.v1",
    },
    AccountEntry {
        name: "AgentRecord",
        design_section: "§3/§4.1",
        stock: None,
        transition_type: "governance",
    },
    AccountEntry {
        name: "PolicyRecord",
        design_section: "§3/§4.1",
        stock: None,
        transition_type: "governance",
    },
    AccountEntry {
        name: "EvidenceRecord",
        design_section: "§3/§4.1",
        stock: None,
        transition_type: "governance",
    },
    // The tranche stock has no single owning account in §3 — it is the
    // window flow's debit against TreasuryState — so it is listed as its
    // own inventory row rather than attached to an unrelated account.
    AccountEntry {
        name: "TrancheStock",
        design_section: "§3/§4.2",
        stock: Some(RES_TREASURY_TRANCHE),
        transition_type: "treasury.window.v1",
    },
];

/// True iff every stock named by the inventory's stock rows is distinct
/// (no two accounts claim the same kernel stock).
pub fn inventory_stocks_unique() -> bool {
    let mut seen = BTreeSet::new();
    ACCOUNT_INVENTORY.iter().filter_map(|e| e.stock).all(|s| seen.insert(s))
}

/// The pinned §4.2 window protocol order (design §4.2): the window opens,
/// the cohort is paid first (beginning of month, same moment new loan
/// windows open), maturing settlements follow, then new borrow / exit /
/// reinvest, with fee reconciliation closing the window. This is protocol
/// order — the program sequences it; the host kernel does not enforce
/// it, and this constant documents the order rather than claiming the
/// kernel does.
pub const WINDOW_PROTOCOL_ORDER: [&str; 7] = [
    "open_window",
    "pay_tranche",
    "settle_default",
    "borrow",
    "exit",
    "reinvest",
    "collect_fees",
];

/// Position of a step name in the window protocol order, if present.
pub fn protocol_position(step: &str) -> Option<usize> {
    WINDOW_PROTOCOL_ORDER.iter().position(|s| *s == step)
}
