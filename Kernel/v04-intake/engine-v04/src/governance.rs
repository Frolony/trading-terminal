//! Governance wrapper (migration step 4, increment 2): the v0.2 engine-sim
//! `Governance` interface over the step-4 `engine-adapter-v04`.
//!
//! This module is the ONLY v0.4-specific code in the engine port. The
//! aggregate engine (`lib.rs`) and the per-holder engine (`per_holder.rs`)
//! are ported verbatim from v0.2 except for the resource constants below;
//! they call exactly the interface v0.2 called, so T1-T9 test the same
//! theorems through the new lifecycle.
//!
//! Interface contract, unchanged from v0.2:
//! * `authorize(resource, amount_micro, available_micro, month, kind)`:
//!   `Some` iff the flow committed and money may move, `None` iff the
//!   predicate refused (blocked flow, no money moves), zero amounts are
//!   no-op authorizations that never reach the kernel.
//! * `settle(auth)`: settles a committed flow after the engine moved the
//!   money; a no-op for zero-amount authorizations.
//!
//! Two v0.4 differences, both deliberate and both recorded here, not
//! hidden in the engine:
//! * **Stock refresh is a governed act.** v0.2 called an admin
//!   `set_capacity` directly before each flow. v0.4 has no direct admin
//!   call, so before each non-zero flow this wrapper drives a unanimous
//!   council `SetCapacity` governance transition through
//!   `EngineAdapter::set_capacity_via_governance`, setting the stock to
//!   `min(available, stock_cap)` — exactly the stock v0.2 set. A governed
//!   stock refresh is itself a committed transition, so a 12-month gated
//!   run lands 48 history entries (24 flows + 24 refreshes), not 24.
//!   The engine's `settled_transitions` count is unchanged (24): it counts
//!   flows, not refreshes.
//! * **Settlement is inside the adapter's drive.** The step-4 adapter
//!   already runs the flow to `record_outcome` (evidence-gated settlement
//!   against the declared hash — the exact-execution assumption the v0.2
//!   adapter also carried: settle assumes execution matches the
//!   declaration exactly) before `authorize` returns. `settle()` here is
//!   therefore a no-op that preserves the engine's call order and its
//!   `settled_transitions` accounting; it never settles twice, because
//!   the adapter's transitions are already past settlement by then.
//!   A future step that separates commit from settle in the adapter must
//!   move settlement back into this function and re-prove T7/T9.
//!
//! Amount conversion: engine micro-dollars and the kernel's `Amount` share
//! the 10^6 scale, so `micro -> units` is `micro / 1_000_000` as f64 and
//! `Amount::from_f64` rounds back to the same raw value for every amount
//! in the engine's range (magnitudes here are <= 10^14 raw, far below the
//! f64 2^53 exact-integer bound; the round-trip error is < 0.02 raw).
//! No engine figure is ever rounded by this conversion: T1-T9 assert the
//! v0.2 values exactly, and a conversion drift would fail them, not be
//! absorbed.

use coordination_kernel::ResourceId;
use engine_adapter_v04::{Authorization as AdapterAuth, EngineAdapter};
use kernel_launch_v04::{LaunchKernel, RES_LENDING_BOOK as LENDING, RES_TREASURY_TRANCHE as TRANCHE};

pub const RES_TREASURY_TRANCHE: ResourceId = TRANCHE;
pub const RES_LENDING_BOOK: ResourceId = LENDING;

/// A committed authorization awaiting the engine's post-execution settle
/// step. Wraps the adapter's authorization; `is_real()` is the only
/// property the engine reads (zero-amount no-ops must not settle or count).
pub struct Authorization {
    inner: AdapterAuth,
}

impl Authorization {
    pub fn is_real(&self) -> bool {
        self.inner.is_real()
    }
    /// The adapter authorization, for tests that need the kernel id.
    pub fn adapter_id(&self) -> coordination_kernel::TransitionId {
        self.inner.id
    }
}

pub struct Governance {
    adapter: EngineAdapter,
    /// Policy stock cap (micro-dollars): no flow may claim more than this,
    /// regardless of the engine's available figure. `None` = uncapped.
    /// Mirrors the v0.2 adapter's `stock_cap_micro` exactly.
    pub stock_cap_micro: Option<i128>,
}

impl Governance {
    pub fn new(stock_cap_micro: Option<i128>) -> Result<Governance, String> {
        Ok(Governance { adapter: EngineAdapter::new(), stock_cap_micro })
    }

    /// Drive one flow through the full v0.4 lifecycle up to (and, in the
    /// step-4 adapter, including) settlement. Returns `Some` iff the
    /// commit predicate cleared; `None` if it refused.
    pub fn authorize(
        &mut self,
        resource: ResourceId,
        amount_micro: i128,
        available_micro: i128,
        _month: u32,
        _kind: u8,
    ) -> Result<Option<Authorization>, String> {
        if amount_micro <= 0 {
            // Zero-amount no-op: never reaches the kernel, never settles.
            // Matches v0.2 (early return before any set_capacity) and the
            // adapter's own zero path; settle_at == 0 marks it.
            return Ok(Some(Authorization {
                inner: AdapterAuth { id: [0; 32], resource, amount: coordination_kernel::Amount::from_raw(0), settle_at: 0 },
            }));
        }
        let err = |e: coordination_kernel::KernelError| format!("{e:?}");
        // v0.2 semantics, v0.4 form: stock = min(available, cap), set by a
        // governed capacity transition, before the flow registers.
        let stock = match self.stock_cap_micro {
            Some(cap) => available_micro.min(cap),
            None => available_micro,
        };
        let stock_units = stock.max(0) as f64 / 1_000_000.0;
        self.adapter.set_capacity_via_governance(resource, stock_units).map_err(|e| format!("{e:?}"))?;
        let amount_units = amount_micro as f64 / 1_000_000.0;
        match self.adapter.authorize(resource, amount_units).map_err(err)? {
            Some(inner) => Ok(Some(Authorization { inner })),
            None => Ok(None),
        }
    }

    /// Settle a committed flow. No-op by design in this increment: the
    /// step-4 adapter already settled the flow (evidence-gated, against
    /// the declared hash) inside `authorize`, so there is nothing left to
    /// settle and a second settle would be a double-settle, not a
    /// post-execution step. Kept so the engine's call order and its
    /// `settled_transitions` accounting stay exactly as in v0.2.
    pub fn settle(&mut self, auth: &Authorization) -> Result<(), String> {
        if !auth.is_real() {
            return Ok(()); // zero-amount no-op authorization
        }
        Ok(())
    }

    pub fn kernel(&self) -> &LaunchKernel {
        self.adapter.kernel()
    }

    pub fn adapter(&self) -> &EngineAdapter {
        &self.adapter
    }
}
