// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./KernelStorage.sol";

/// Delegatecalled by the kernel: the pre-commit procedure.
/// evaluate -> eligible -> select -> planning -> reconcile -> validate.
/// Every step is permissionless and reads only kernel-owned state; none of it
/// needs an administrator, because each step is guarded by a predicate.
contract PlanningModule is KernelStorage {
    // ------------------------------------------------------------------------
    // Evaluation: kernel-owned inputs, isolated external calls
    // ------------------------------------------------------------------------

    function _callConstraint(address impl, bytes32 s, bytes32 t, bytes32 o) private view returns (uint8) {
        (bool ok, bytes memory ret) = impl.staticcall{gas: evaluatorGasCap}(
            abi.encodeCall(IConstraint.validate, (s, t, o))
        );
        if (!ok || ret.length != 64) return ST_UNAVAILABLE;
        (uint256 pass, uint256 st) = abi.decode(ret, (uint256, uint256));
        if (pass > 1 || st > 2) return ST_UNAVAILABLE;
        if ((pass == 1) != (st == ST_PASS)) return ST_FAIL; // disagreement is FAIL
        return uint8(st);
    }

    function _callObjective(address impl, bytes32 s, bytes32 t, bytes32 o)
        private view returns (bool ok, int256 score, bool satisfied)
    {
        (bool success, bytes memory ret) = impl.staticcall{gas: evaluatorGasCap}(
            abi.encodeCall(IObjective.evaluate, (s, t, o))
        );
        if (!success || ret.length != 64) return (false, 0, false);
        (int256 sc, uint256 sat) = abi.decode(ret, (int256, uint256));
        if (sat > 1) return (false, 0, false);
        return (true, sc, sat == 1);
    }

    /// Anyone may trigger evaluation. FAIL on any constraint -> REJECTED.
    /// Only UNAVAILABLE -> stays PROPOSED (retryable). UNAVAILABLE is never PASS.
    function evaluateTransition(bytes32 id) external whenOperational(id) {
        _require(id, TransitionLifecycle.PROPOSED);
        _notExpired(id);
        Transition storage t = transitions[id];
        Policy storage pol = transitionPolicy[id];
        bytes32 obsId = t.manifest.observationId;
        if (!(_observationFresh(obsId, pol.minObsConfidence))) revert ObservationNotFresh();

        EvaluationResult storage er = evaluationResults[id];
        delete er.constraintIds;
        delete er.constraintStatus;
        delete er.objectiveIds;
        delete er.objectiveScores;
        delete er.objectiveSatisfied;
        EvaluationContext storage ctx = context[id];
        delete ctx.constraintVersions;
        delete ctx.objectiveVersions;

        (bool anyFail, bool anyUnavailable) = _evalConstraints(id, t.parentStateId, obsId, er, ctx);
        bool allObjectives = _evalObjectives(id, t.parentStateId, obsId, er, ctx);

        er.constraintsSatisfied = !anyFail && !anyUnavailable;
        er.objectivesSatisfied = allObjectives;
        er.observationId = obsId;
        er.confidence = observations[obsId].confidence;

        emit TransitionEvaluated(id, er.constraintsSatisfied, er.objectivesSatisfied);
        if (anyFail) _set(id, TransitionLifecycle.REJECTED);
        else if (!anyUnavailable) _set(id, TransitionLifecycle.EVALUATED);
    }

    function _evalConstraints(
        bytes32 id, bytes32 parentId, bytes32 obsId,
        EvaluationResult storage er, EvaluationContext storage ctx
    ) private returns (bool anyFail, bool anyUnavailable) {
        bytes32[] storage ids = constraintSet[id];
        for (uint256 i = 0; i < ids.length; i++) {
            (bool found, uint256 v) = _activeVersion(_constraintReg[ids[i]]);
            ctx.constraintVersions.push(v);
            uint8 st = found
                ? _callConstraint(_constraintReg[ids[i]][v].impl, parentId, id, obsId)
                : ST_UNAVAILABLE;
            er.constraintIds.push(ids[i]);
            er.constraintStatus.push(st);
            if (st == ST_FAIL) anyFail = true;
            else if (st != ST_PASS) { anyUnavailable = true; emit EvaluatorFailed(id, ids[i]); }
        }
    }

    function _evalObjectives(
        bytes32 id, bytes32 parentId, bytes32 obsId,
        EvaluationResult storage er, EvaluationContext storage ctx
    ) private returns (bool all) {
        bytes32[] storage ids = objectiveSet[id];
        all = true;
        for (uint256 i = 0; i < ids.length; i++) {
            (bool found, uint256 v) = _activeVersion(_objectiveReg[ids[i]]);
            ctx.objectiveVersions.push(v);
            bool ok;
            int256 score;
            bool sat;
            if (found) (ok, score, sat) = _callObjective(_objectiveReg[ids[i]][v].impl, parentId, id, obsId);
            er.objectiveIds.push(ids[i]);
            er.objectiveScores.push(ok ? score : int256(0));
            er.objectiveSatisfied.push(ok && sat);
            if (!(ok && sat)) all = false;
        }
    }

    /// EVALUATED -> ELIGIBLE. Feasibility gate: no objective score can compensate for a failed constraint.
    function markEligible(bytes32 id) external whenOperational(id) {
        _require(id, TransitionLifecycle.EVALUATED);
        _notExpired(id);
        Policy storage pol = transitionPolicy[id];
        EvaluationResult storage er = evaluationResults[id];
        if (!(er.constraintsSatisfied)) revert ConstraintsNotSatisfied();
        if (pol.requireObjectives) if (!(er.objectivesSatisfied)) revert ObjectivesNotSatisfied();
        if (!(_observationFresh(transitions[id].manifest.observationId, pol.minObsConfidence))) revert ObservationNotFresh();

        // Freeze relevance rule versions and the reputation snapshot point.
        bytes32[] storage rids = relevanceSet[id];
        EvaluationContext storage ctx = context[id];
        for (uint256 i = 0; i < rids.length; i++) {
            (, uint256 v) = _activeVersion(_relevanceReg[rids[i]]);
            ctx.relevanceVersions.push(v);
        }
        eligibleAt[id] = block.timestamp;
        consensusDeadline[id] = block.timestamp + pol.consensusWindow;

        // Join the field of siblings competing for this parent. A slot held by a
        // transition that is no longer ELIGIBLE can be reclaimed; otherwise the field is full.
        bytes32[] storage sib = eligibleSiblings[transitions[id].parentStateId];
        if (sib.length < MAX_SIBLINGS) {
            sib.push(id);
        } else {
            bool placed;
            for (uint256 i = 0; i < sib.length; i++) {
                if (transitionLifecycle[sib[i]] != TransitionLifecycle.ELIGIBLE) { sib[i] = id; placed = true; break; }
            }
            if (!placed) revert SiblingCap();
        }
        _set(id, TransitionLifecycle.ELIGIBLE);
    }

    // ------------------------------------------------------------------------
    // Selection
    // ------------------------------------------------------------------------

    function _qualifies(bytes32 id) private view returns (bool) {
        Policy storage pol = transitionPolicy[id];
        Tally storage ty = tallies[id];
        return ty.totalWeight > 0 && ty.totalWeight >= pol.quorumRequired && confidenceOf(id) >= pol.confidenceThreshold;
    }

    /// Permissionless. Selection waits for the consensus window to close so every
    /// round has been heard, then picks by one rule: among the siblings that
    /// qualify (quorum and confidence), the highest confidence wins; a tie goes to
    /// the sibling that became eligible first. A sibling whose window is still
    /// open blocks selection until it closes, so nobody can win by calling early.
    function selectCandidate(bytes32 id) external whenOperational(id) {
        _require(id, TransitionLifecycle.ELIGIBLE);
        _notExpired(id);
        if (block.timestamp < consensusDeadline[id]) revert ConsensusStillOpen();
        if (!_qualifies(id)) {
            Policy storage pol = transitionPolicy[id];
            if (!(tallies[id].totalWeight > 0 && tallies[id].totalWeight >= pol.quorumRequired)) revert QuorumNotMet();
            revert ConfidenceBelowThreshold();
        }

        bytes32 parent = transitions[id].parentStateId;
        bytes32[] storage sib = eligibleSiblings[parent];
        uint256 mine = type(uint256).max;
        for (uint256 i = 0; i < sib.length; i++) {
            if (sib[i] == id) { mine = i; break; }
        }
        uint256 myConf = confidenceOf(id);
        for (uint256 i = 0; i < sib.length; i++) {
            bytes32 s = sib[i];
            if (s == id || transitionLifecycle[s] != TransitionLifecycle.ELIGIBLE) continue;
            if (!(block.timestamp < transitions[s].manifest.expiresAt)) continue;
            if (block.timestamp < consensusDeadline[s]) revert SiblingsStillOpen(s);
            if (_qualifies(s)) {
                uint256 c = confidenceOf(s);
                if (c > myConf || (c == myConf && i < mine)) revert OutrankedBySibling(s);
            }
        }

        bytes32 prev = selectedForParent[parent];
        if (prev != bytes32(0) && prev != id) {
            TransitionLifecycle ps = transitionLifecycle[prev];
            bool live = (uint8(ps) >= 4 && uint8(ps) <= 8) || ps == TransitionLifecycle.BLOCKED || ps == TransitionLifecycle.COMMITTED;
            if (live) revert ParentAlreadyHasASelectedTransition();
        }
        selectedForParent[parent] = id;
        _set(id, TransitionLifecycle.SELECTED);
        emit TransitionSelected(id);
    }

    // ------------------------------------------------------------------------
    // Planning
    // ------------------------------------------------------------------------

    /// SELECTED -> PLANNING (anyone), or BLOCKED -> PLANNING for a re-plan
    /// (proposer or a council seat, at most MAX_REPLANS times).
    function openPlanning(bytes32 id) external whenOperational(id) {
        TransitionLifecycle cur = transitionLifecycle[id];
        if (cur == TransitionLifecycle.BLOCKED) {
            if (!(msg.sender == transitions[id].proposer || isSeated[msg.sender])) revert Unauthorized(bytes32("PROPOSER_OR_COUNCIL"));
            if (!(replans[id] < MAX_REPLANS)) revert ReplanCap();
            replans[id] += 1;
        } else if (cur != TransitionLifecycle.SELECTED) {
            revert BadLifecycle(cur);
        }
        _notExpired(id);
        (bool found, uint256 v) = _activeVersion(_engineReg);
        if (!(found)) revert NoActiveCompatibilityEngine();
        context[id].engineVersion = v;
        uint256 dl = block.timestamp + transitionPolicy[id].planningWindow;
        planningDeadline[id] = dl;
        _set(id, TransitionLifecycle.PLANNING);
        emit PlanningOpened(id, dl);
    }

    function submitPlan(bytes32 transitionId, PlanInput calldata pin)
        external whenOperational(transitionId) returns (bytes32 planId)
    {
        _require(transitionId, TransitionLifecycle.PLANNING);
        _notExpired(transitionId);
        if (!(block.timestamp < planningDeadline[transitionId])) revert PlanningClosed();
        if (!(pin.expiresAt > planningDeadline[transitionId])) revert PlanExpiresTooEarly();
        if (!(pin.resourceClaims.length <= MAX_ARR && pin.effects.length <= MAX_ARR &&
            pin.prerequisites.length <= MAX_ARR && pin.conflicts.length <= MAX_ARR)) revert PlanTooLarge();
        (bool ok, ) = _eligible(msg.sender, transitionId);
        if (!(ok)) revert NotEligible();
        uint256 count = transitionPlans[transitionId].length;
        if (!(count < transitionPolicy[transitionId].maxPlans)) revert PlanCap();

        planId = keccak256(abi.encode(transitionId, msg.sender, count)); // kernel-owned id
        _storePlan(planId, transitionId, pin);
        transitionPlans[transitionId].push(planId);

        uint256 cur = minPlanExpiry[transitionId];
        if (cur == 0 || pin.expiresAt < cur) minPlanExpiry[transitionId] = pin.expiresAt;
        emit PlanSubmitted(transitionId, planId, msg.sender);
    }

    function _storePlan(bytes32 planId, bytes32 transitionId, PlanInput calldata pin) private {
        Plan storage p = plans[planId];
        p.id = planId;
        p.agent = msg.sender;
        p.candidateTransitionId = transitionId;
        p.resourceClaims = pin.resourceClaims;
        p.effects = pin.effects;
        p.prerequisites = pin.prerequisites;
        p.conflicts = pin.conflicts;
        p.expiresAt = pin.expiresAt;
        p.payloadHash = pin.payloadHash;
    }

    /// Compatibility K includes the coverage requirement: it is NOT monotone in
    /// plans, because adding a plan can raise coverage.
    function reconcilePlans(bytes32 id) external whenOperational(id) {
        _require(id, TransitionLifecycle.PLANNING);
        _notExpired(id);
        if (!(block.timestamp >= planningDeadline[id])) revert PlanningWindowOpen();

        bytes32[] storage planIds = transitionPlans[id];
        ReconciliationResult storage rr = reconciliationResults[id];
        delete rr.conflictingPlans;
        delete rr.conflictingResources;
        delete rr.unresolvedDependencies;
        rr.compatible = false;
        rr.engineFailed = false;
        rr.coverage = 0;

        if (planIds.length == 0) {
            rr.engineFailed = true;
        } else {
            IPlanCompatibilityEngine eng = IPlanCompatibilityEngine(_engineReg[context[id].engineVersion].impl);
            try eng.evaluate{gas: evaluatorGasCap}(id, planIds) returns (
                bool compat, bytes32[] memory cp, bytes32[] memory cr, bytes32[] memory ud, uint256 cov
            ) {
                if (cp.length > MAX_ARR || cr.length > MAX_ARR || ud.length > MAX_ARR) {
                    rr.engineFailed = true;
                } else {
                    rr.conflictingPlans = cp;
                    rr.conflictingResources = cr;
                    rr.unresolvedDependencies = ud;
                    rr.coverage = cov;
                    rr.compatible = compat && cov >= transitionPolicy[id].coverageMin
                        && cp.length == 0 && cr.length == 0 && ud.length == 0;
                }
            } catch {
                rr.engineFailed = true;
                emit EvaluatorFailed(id, bytes32("ENGINE"));
            }
        }
        _set(id, TransitionLifecycle.RECONCILING);
        emit PlansReconciled(id, rr.compatible);
    }

    /// RECONCILING or BLOCKED -> VALIDATED (else RECONCILING -> BLOCKED).
    /// Structural terms only; quorum, confidence, capability, dialect and the
    /// rest are re-checked by the one canonical predicate at declare and commit.
    function validateTransition(bytes32 id) external whenOperational(id) {
        TransitionLifecycle cur = transitionLifecycle[id];
        if (cur != TransitionLifecycle.RECONCILING && cur != TransitionLifecycle.BLOCKED) revert BadLifecycle(cur);
        bool ok = evaluationResults[id].constraintsSatisfied &&
            reconciliationResults[id].compatible &&
            unresolvedBlocking[id] == 0 &&
            block.timestamp < transitions[id].manifest.expiresAt &&
            _observationFresh(transitions[id].manifest.observationId, transitionPolicy[id].minObsConfidence);
        if (ok) {
            _set(id, TransitionLifecycle.VALIDATED);
            emit TransitionValidated(id);
        } else {
            if (!(cur == TransitionLifecycle.RECONCILING)) revert StillBlocked();
            _set(id, TransitionLifecycle.BLOCKED);
        }
    }
}
