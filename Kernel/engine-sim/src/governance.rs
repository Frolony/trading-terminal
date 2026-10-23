//! Governance adapter (W3.1): engine flows execute as Coordination Kernel
//! transitions under the `kernel-policy` launch profile.
//!
//! A dividend tranche or lending batch is a proposal: it is registered with a
//! declared outcome hash, gated by the full `commit_fail_mask` predicate
//! (constraint pass, quorum, confidence, capacity compatibility, lineage,
//! freshness), and only on a clear mask does the engine move the money. After
//! execution, the adapter records the realized outcome as evidence with the
//! `EvidenceAttestor` and settles — settle-after-realized, exactly as the
//! profile specifies. A claim exceeding its authorized stock fails COMPAT at
//! the predicate: the flow is blocked, and the engine records the block.

use coordination_kernel::*;
use kernel_policy::{launch_kernel, EvidenceAttestor, LaunchKernel};
use std::collections::{BTreeMap, BTreeSet};

pub const ADMIN: AgentId = [0xA0; 32];
pub const ORACLE: AgentId = [0xA1; 32];
pub const EVALUATOR: AgentId = [0xA2; 32];
pub const PROPOSER: AgentId = [1; 32];

const CONSTRAINT: ConstraintId = [0xC1; 32];
const TT_TRANCHE: TransitionType = [0xD1; 32];
const TT_LENDING: TransitionType = [0xD2; 32];

fn amt(x: f64) -> Amount { Amount::from_f64(x).expect("valid amount") }

/// A committed authorization awaiting post-execution settlement.
pub struct Authorization {
    id: TransitionId,
    declared: Hash32,
    settle_at: Timestamp,
}

impl Authorization {
    /// A zero-amount flow never reaches the kernel (no transition exists),
    /// so it must not be settled or counted — only real commits settle.
    pub fn is_real(&self) -> bool { self.settle_at != 0 }
}

pub struct Governance {
    kernel: LaunchKernel,
    attestor: EvidenceAttestor,
    seq: u64,
    /// Policy stock cap (micro-dollars): no flow may claim more than this,
    /// regardless of the engine's available figure. `None` = uncapped.
    pub stock_cap_micro: Option<i128>,
}

impl Governance {
    pub fn new(stock_cap_micro: Option<i128>) -> Result<Governance, String> {
        let attestor = EvidenceAttestor::new();
        let genesis = SystemState { index: 0, hash: [0; 32], timestamp: 1_000 };
        let mut kernel = launch_kernel(ADMIN, genesis, attestor.clone()).map_err(|e| format!("{e:?}"))?;
        kernel.grant_role(&ADMIN, ORACLE, Role::Oracle).map_err(|e| format!("{e:?}"))?;
        kernel.grant_role(&ADMIN, EVALUATOR, Role::Evaluator).map_err(|e| format!("{e:?}"))?;
        for tt in [TT_TRANCHE, TT_LENDING] {
            kernel.set_mandatory_constraints(&ADMIN, tt, BTreeSet::from([CONSTRAINT])).map_err(|e| format!("{e:?}"))?;
        }
        kernel.register_agent(&ADMIN, AgentProfile {
            id: PROPOSER, active: true, groups: BTreeSet::from([1]),
            influence: Amount::ONE, reputation: amt(50.0),
        }).map_err(|e| format!("{e:?}"))?;
        Ok(Governance { kernel, attestor, seq: 0, stock_cap_micro })
    }

    /// Drive one flow through the full lifecycle up to commit.
    /// Returns `Some(Authorization)` iff the commit predicate cleared and the
    /// commit landed; `None` if the predicate refused (e.g. claim > stock).
    pub fn authorize(
        &mut self,
        resource: ResourceId,
        amount_micro: i128,
        available_micro: i128,
        month: u32,
        kind: u8,
    ) -> Result<Option<Authorization>, String> {
        if amount_micro <= 0 { return Ok(Some(Authorization { id: [0; 32], declared: [0; 32], settle_at: 0 })); }
        let err = |e: KernelError| format!("{e:?}");
        self.seq += 1;
        let base: Timestamp = 1_000 + self.seq * 100;
        let stock = match self.stock_cap_micro {
            Some(cap) => available_micro.min(cap),
            None => available_micro,
        };
        let stock_u64 = u64::try_from(stock.max(0)).map_err(|_| "stock overflow".to_string())?;
        let amount_u64 = u64::try_from(amount_micro).map_err(|_| "amount overflow".to_string())?;
        self.kernel.set_capacity(&ADMIN, resource, Amount::from_raw(stock_u64)).map_err(err)?;

        let mut id = [0u8; 32];
        id[0] = kind;
        id[1..5].copy_from_slice(&month.to_be_bytes());
        id[5..9].copy_from_slice(&(self.seq as u32).to_be_bytes());
        let mut declared = [0u8; 32];
        declared[0] = kind;
        declared[1..9].copy_from_slice(&amount_micro.to_be_bytes()[8..]);
        declared[9..13].copy_from_slice(&month.to_be_bytes());
        let mut obs_id = [0xB0; 32];
        obs_id[1..9].copy_from_slice(&self.seq.to_be_bytes());

        let parent = self.kernel.declared_state().clone();
        self.kernel.post_observation(&ORACLE, Observation {
            id: obs_id, timestamp: base, expiry: base + 9_000, confidence: amt(0.9),
        }, base).map_err(err)?;
        let tt = if kind == 1 { TT_TRANCHE } else { TT_LENDING };
        self.kernel.register_transition(&PROPOSER, Manifest {
            id, transition_type: tt, parent_index: parent.index, parent_hash: parent.hash,
            declared_next_hash: declared, observation_id: obs_id, expires_at: base + 10_000,
            relevant_groups: BTreeSet::from([1]),
        }, Policy {
            quorum_required: Amount::ONE, confidence_threshold: amt(0.5),
            min_obs_confidence: amt(0.5), require_objectives: false,
            planning_window: 10, max_rounds: 3, max_plans: 4,
        }, BTreeSet::new(), base).map_err(err)?;
        self.kernel.report_constraint(&EVALUATOR, &id, CONSTRAINT, ConstraintStatus::Pass, base + 1).map_err(err)?;
        self.kernel.submit_signal(&PROPOSER, &id, Amount::ONE, amt(0.9), base + 2).map_err(err)?;
        self.kernel.open_planning(&PROPOSER, &id, base + 3).map_err(err)?;
        self.kernel.submit_plan(&PROPOSER, Plan {
            agent: PROPOSER,
            claims: BTreeMap::from([(resource, Amount::from_raw(amount_u64))]),
            expires_at: base + 5_000,
        }, &id, base + 4).map_err(err)?;
        self.kernel.reconcile(&PROPOSER, &id, base + 50).map_err(err)?;
        let mask = self.kernel.commit_fail_mask(&id, base + 50).map_err(err)?;
        if !mask.is_clear() { return Ok(None); }
        self.kernel.commit(&id, base + 50).map_err(err)?;
        Ok(Some(Authorization { id, declared, settle_at: base + 60 }))
    }

    /// Settle a committed flow after the engine executed it: the realized
    /// outcome (== declared, for an exact execution) is recorded as evidence
    /// first, then the Oracle settles against that evidence.
    pub fn settle(&mut self, auth: &Authorization) -> Result<(), String> {
        if auth.settle_at == 0 { return Ok(()); } // zero-amount no-op authorization
        self.attestor.record_evidence(auth.id, auth.declared);
        self.kernel.settle(&ORACLE, &auth.id, auth.declared, &[], auth.settle_at)
            .map_err(|e| format!("{e:?}"))?;
        Ok(())
    }

    pub fn kernel(&self) -> &LaunchKernel { &self.kernel }
}
