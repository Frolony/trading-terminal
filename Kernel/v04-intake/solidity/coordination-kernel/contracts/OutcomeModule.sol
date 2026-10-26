// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./KernelStorage.sol";

/// Delegatecalled by the kernel: outcome recording, reputation attribution, and
/// the Layer-3 branching-health record (a measurement, like an observation).
contract OutcomeModule is KernelStorage {
    function computeBranchingHealth(uint256 capScore, uint256 trustScore, uint256 contextScore, uint256 coverageScore)
        public pure returns (BranchingHealth memory h)
    {
        h.capabilityScore = capScore;
        h.trustScore = trustScore;
        h.contextScore = contextScore;
        h.coverageScore = coverageScore;
        uint256 m = capScore < trustScore ? capScore : trustScore;
        m = m < contextScore ? m : contextScore;
        h.minAggregate = m < coverageScore ? m : coverageScore;
    }

    /// Records health and freezes the substrate if the minimum collapses to zero.
    /// OBSERVER-gated: this is a measurement. Leaving the frozen state is a
    /// governance transition (rebootstrapSystem), never a role call.
    function recordBranchingHealth(uint256 stateIndex, uint256 capScore, uint256 trustScore, uint256 contextScore, uint256 coverageScore)
        external onlyRole(OBSERVER) whenNotPaused returns (BranchingHealth memory h)
    {
        h = computeBranchingHealth(capScore, trustScore, contextScore, coverageScore);
        stateHealthHistory[stateIndex] = h;
        emit BranchingHealthUpdated(stateIndex, h.minAggregate);
        if (h.minAggregate == 0 && systemState == SystemState.OPERATIONAL) {
            systemState = SystemState.COLLAPSED_FROZEN;
            emit SystemStateChanged(SystemState.OPERATIONAL, SystemState.COLLAPSED_FROZEN);
            emit BranchingCollapseDetected(stateIndex);
        }
    }

    function recordOutcome(
        bytes32 id,
        bytes32[] calldata observedEffects,
        bytes32[] calldata violatedConstraints,
        bytes32 correctiveTransitionId
    ) external onlyRole(OUTCOME_RECORDER) whenNotPaused {
        _require(id, TransitionLifecycle.SETTLED);
        if (!(observedEffects.length <= MAX_ARR && violatedConstraints.length <= MAX_SET)) revert ArraysTooLarge();
        bytes32 outcomeId = keccak256(abi.encode(id, "OUTCOME"));
        Outcome storage o = outcomes[id];
        if (!(o.id == bytes32(0))) revert OutcomeExists();

        bytes32[] memory unexpected = _unexpected(id, observedEffects);
        _checkViolated(id, violatedConstraints);

        o.id = outcomeId;
        o.transitionId = id;
        o.resultingStateId = realizedStates[id].id;
        o.observedEffects = observedEffects;
        o.violatedConstraints = violatedConstraints;
        o.unexpectedEffects = unexpected;
        o.correctiveTransitionId = correctiveTransitionId;
        // Derived, never caller-asserted.
        o.successful = violatedConstraints.length == 0 && unexpected.length == 0 &&
            realizedStates[id].stateHash == transitions[id].manifest.resultingStateHash;

        _attribute(id, outcomeId);
        _set(id, TransitionLifecycle.EVALUATED_OUTCOME);
        emit OutcomeRecorded(id, o.successful);
    }

    /// ObservedEffects \ ExpectedEffects
    function _unexpected(bytes32 id, bytes32[] calldata observed) private view returns (bytes32[] memory out) {
        bytes32[] storage expected = transitions[id].manifest.expectedEffects;
        out = new bytes32[](observed.length);
        uint256 n = 0;
        for (uint256 i = 0; i < observed.length; i++) {
            bool found = false;
            for (uint256 j = 0; j < expected.length; j++) {
                if (expected[j] == observed[i]) { found = true; break; }
            }
            if (!found) out[n++] = observed[i];
        }
        assembly { mstore(out, n) }
    }

    /// Violated constraints must belong to this transition's frozen constraint set.
    function _checkViolated(bytes32 id, bytes32[] calldata violated) private view {
        bytes32[] storage set_ = constraintSet[id];
        for (uint256 i = 0; i < violated.length; i++) {
            bool found = false;
            for (uint256 j = 0; j < set_.length; j++) {
                if (set_[j] == violated[i]) { found = true; break; }
            }
            if (!(found)) revert ViolatedConstraintNotInSet();
        }
    }

    /// ΔR(a) = Σ_i ΔR_i(a), each clamped to ±MAX_DELTA, then Rep clamped once in _applyRep.
    function _attribute(bytes32 id, bytes32 outcomeId) private {
        bytes32[] storage aids = attributionSet[id];
        uint256[] storage vs = context[id].attributionVersions;
        address[] memory touched = new address[](MAX_ATTR_RULES * MAX_ATTR_AGENTS);
        int256[] memory deltas = new int256[](MAX_ATTR_RULES * MAX_ATTR_AGENTS);
        uint256 n = 0;

        for (uint256 i = 0; i < aids.length; i++) {
            if (vs[i] == NO_VERSION) continue;
            IAttributionRule rule = IAttributionRule(_attributionReg[aids[i]][vs[i]].impl);
            try rule.evaluate{gas: evaluatorGasCap}(id, outcomeId) returns (
                address[] memory ag, int256[] memory d, bytes32[] memory mp
            ) {
                if (ag.length != d.length || ag.length > MAX_ATTR_AGENTS) {
                    emit EvaluatorFailed(id, aids[i]);
                    continue;
                }
                for (uint256 j = 0; j < ag.length; j++) {
                    int256 dj = d[j];
                    if (dj > MAX_DELTA) dj = MAX_DELTA;
                    if (dj < -MAX_DELTA) dj = -MAX_DELTA;
                    uint256 k = 0;
                    while (k < n && touched[k] != ag[j]) k++;
                    if (k == n) { touched[n] = ag[j]; n++; }
                    deltas[k] += dj;
                }
                bytes32[] storage store = modelProposals[outcomeId];
                for (uint256 m = 0; m < mp.length && store.length < MAX_ARR; m++) {
                    store.push(mp[m]);
                    emit ModelProposalRecorded(outcomeId, mp[m]);
                }
            } catch {
                emit EvaluatorFailed(id, aids[i]);
            }
        }

        for (uint256 k = 0; k < n; k++) {
            int256 dk = deltas[k];
            if (dk > MAX_DELTA) dk = MAX_DELTA;
            if (dk < -MAX_DELTA) dk = -MAX_DELTA;
            _applyRep(touched[k], dk);
        }
    }

    function _applyRep(address agent, int256 delta) internal {
        RepCheckpoint[] storage h = repHistory[agent];
        if (h.length == 0) return; // unknown agent: ignore
        int256 next = h[h.length - 1].rep + delta;
        if (next < REP_MIN) next = REP_MIN;
        if (next > REP_MAX) next = REP_MAX;
        agents[agent].reputation = next;
        if (h[h.length - 1].ts == uint64(block.timestamp)) h[h.length - 1].rep = next;
        else h.push(RepCheckpoint(uint64(block.timestamp), next));
        emit ReputationChanged(agent, delta, next);
    }
}
