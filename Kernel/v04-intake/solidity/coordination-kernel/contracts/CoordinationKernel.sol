// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./KernelStorage.sol";
import "./GovernanceModule.sol";
import "./OutcomeModule.sol";
import "./ConsensusModule.sol";
import "./PlanningModule.sol";
import "./DialectModule.sol";
import "./CouncilModule.sol";

/**
 * CoordinationKernel: the amalgam of the TransitionKernel line (hardened lifecycle,
 * per-type policy, settlement, attribution) and the CoordinationKernel line
 * (rounds, classifier, capability, dialect registry, substrate health).
 *
 * One commit predicate (commitFailMask) decides everything that can ever commit,
 * including every change to the rules themselves. After sealGenesis() there is no
 * administrator: a rule changes only when a council-proposed governance transition
 * passes all thirteen terms of the predicate and commitGovernance() applies it in
 * the same transaction.
 *
 * Shared state lives in KernelStorage. The modules are delegatecall targets.
 */
contract CoordinationKernel is KernelStorage {
    struct Modules {
        address governance;
        address outcome;
        address consensus;
        address planning;
        address dialect;
        address council;
    }

    constructor(
        address genesisAdmin_,
        bytes32 genesisStateId,
        bytes32 genesisStateHash,
        uint256 registryDelay_,
        uint256 epochLength_,
        uint256 councilSeats_,
        Modules memory m
    ) {
        if (genesisAdmin_ == address(0)) revert ZeroGovernance();
        if (epochLength_ == 0) revert BadEpochLength();
        if (!(councilSeats_ > 0 && councilSeats_ <= MAX_SEATS && councilSeats_ % 2 == 1)) revert BadSeats();
        if (m.governance.code.length == 0 || m.outcome.code.length == 0 || m.consensus.code.length == 0 ||
            m.planning.code.length == 0 || m.dialect.code.length == 0 || m.council.code.length == 0) revert NotAContract();
        _wireModules(m);
        _wireGovernanceActions();

        genesisAdmin = genesisAdmin_;
        registryDelay = registryDelay_;
        epochZero = block.timestamp;
        epochLength = epochLength_;
        councilSeats = councilSeats_;
        maxInfluenceShare = (WAD + 5) / 6; // the treasury note's whale ratio L/T = 1/6 (rounded up so six equal agents fit), reused as the weight bound
        declaredState = State(genesisStateId, 0, genesisStateHash);

        _allow(TransitionLifecycle.PROPOSED, TransitionLifecycle.EVALUATED);
        _allow(TransitionLifecycle.PROPOSED, TransitionLifecycle.REJECTED);
        _allow(TransitionLifecycle.EVALUATED, TransitionLifecycle.ELIGIBLE);
        _allow(TransitionLifecycle.ELIGIBLE, TransitionLifecycle.SELECTED);
        _allow(TransitionLifecycle.SELECTED, TransitionLifecycle.PLANNING);
        _allow(TransitionLifecycle.PLANNING, TransitionLifecycle.RECONCILING);
        _allow(TransitionLifecycle.RECONCILING, TransitionLifecycle.VALIDATED);
        _allow(TransitionLifecycle.RECONCILING, TransitionLifecycle.BLOCKED);
        _allow(TransitionLifecycle.VALIDATED, TransitionLifecycle.COMMITTABLE);
        _allow(TransitionLifecycle.VALIDATED, TransitionLifecycle.BLOCKED);
        _allow(TransitionLifecycle.COMMITTABLE, TransitionLifecycle.COMMITTED);
        _allow(TransitionLifecycle.COMMITTABLE, TransitionLifecycle.BLOCKED);
        _allow(TransitionLifecycle.COMMITTED, TransitionLifecycle.EXECUTING);
        _allow(TransitionLifecycle.EXECUTING, TransitionLifecycle.SETTLED);
        _allow(TransitionLifecycle.SETTLED, TransitionLifecycle.EVALUATED_OUTCOME);
        // Recoverable BLOCKED: re-validate, or re-open planning
        _allow(TransitionLifecycle.BLOCKED, TransitionLifecycle.VALIDATED);
        _allow(TransitionLifecycle.BLOCKED, TransitionLifecycle.PLANNING);
        // Pre-commit exits. COMMITTED and later can never regress.
        for (uint8 s = 1; s <= 8; s++) _allowExit(TransitionLifecycle(s));
        _allowExit(TransitionLifecycle.BLOCKED);

        _genesisNotations();
    }

    function _allow(TransitionLifecycle a, TransitionLifecycle b) private { _allowed[a][b] = true; }

    function _allowExit(TransitionLifecycle a) private {
        _allowed[a][TransitionLifecycle.EXPIRED] = true;
        _allowed[a][TransitionLifecycle.CONFLICTED] = true;
        _allowed[a][TransitionLifecycle.DISCARDED] = true;
    }

    /// The routing table is written once, here, and never changes.
    function _wireModules(Modules memory m) private {
        address g = m.governance;
        moduleOf[GovernanceModule.sealGenesis.selector] = g;
        moduleOf[GovernanceModule.approveGovernance.selector] = g;
        moduleOf[GovernanceModule.pause.selector] = g;
        moduleOf[GovernanceModule.unpause.selector] = g;
        moduleOf[GovernanceModule.grantRole.selector] = g;
        moduleOf[GovernanceModule.revokeRole.selector] = g;
        moduleOf[GovernanceModule.setEvaluatorGasCap.selector] = g;
        moduleOf[GovernanceModule.setPolicy.selector] = g;
        moduleOf[GovernanceModule.setRequiredSets.selector] = g;
        moduleOf[GovernanceModule.setRequiredCapabilities.selector] = g;
        moduleOf[GovernanceModule.setAgentCapability.selector] = g;
        moduleOf[GovernanceModule.setGroupMultiplier.selector] = g;
        moduleOf[GovernanceModule.setMaxInfluenceShare.selector] = g;
        moduleOf[GovernanceModule.registerImpl.selector] = g;
        moduleOf[GovernanceModule.registerEngine.selector] = g;
        moduleOf[GovernanceModule.retireImpl.selector] = g;
        moduleOf[GovernanceModule.retireEngine.selector] = g;
        moduleOf[GovernanceModule.implOf.selector] = g;
        moduleOf[GovernanceModule.registerAgent.selector] = g;
        moduleOf[GovernanceModule.setAgentActive.selector] = g;
        moduleOf[GovernanceModule.invalidateObservation.selector] = g;
        moduleOf[GovernanceModule.setTrustMerkleRoot.selector] = g;
        moduleOf[GovernanceModule.verifyTrustProof.selector] = g;
        moduleOf[GovernanceModule.rebootstrapSystem.selector] = g;

        address o = m.outcome;
        moduleOf[OutcomeModule.recordOutcome.selector] = o;
        moduleOf[OutcomeModule.recordBranchingHealth.selector] = o;
        moduleOf[OutcomeModule.computeBranchingHealth.selector] = o;

        address c = m.consensus;
        moduleOf[ConsensusModule.classifyStance.selector] = c;
        moduleOf[ConsensusModule.sectorOf.selector] = c;
        moduleOf[ConsensusModule.baseWeightOf.selector] = c;
        moduleOf[ConsensusModule.nicheOf.selector] = c;
        moduleOf[ConsensusModule.roundOf.selector] = c;
        moduleOf[ConsensusModule.submitRoundSignal.selector] = c;
        moduleOf[ConsensusModule.submitExceptionSignal.selector] = c;
        moduleOf[ConsensusModule.resolveException.selector] = c;

        address p = m.planning;
        moduleOf[PlanningModule.evaluateTransition.selector] = p;
        moduleOf[PlanningModule.markEligible.selector] = p;
        moduleOf[PlanningModule.selectCandidate.selector] = p;
        moduleOf[PlanningModule.openPlanning.selector] = p;
        moduleOf[PlanningModule.submitPlan.selector] = p;
        moduleOf[PlanningModule.reconcilePlans.selector] = p;
        moduleOf[PlanningModule.validateTransition.selector] = p;

        address d = m.dialect;
        moduleOf[DialectModule.proposeNotationChange.selector] = d;
        moduleOf[DialectModule.declareDialectDependency.selector] = d;

        address k = m.council;
        moduleOf[CouncilModule.councilMembers.selector] = k;
        moduleOf[CouncilModule.recordPosition.selector] = k;
        moduleOf[CouncilModule.isCandidate.selector] = k;
        moduleOf[CouncilModule.installCouncil.selector] = k;
        moduleOf[CouncilModule.setCouncilSeats.selector] = k;
        moduleOf[CouncilModule.setPositionSource.selector] = k;
    }

    /// Which transition type may carry which action. Nothing outside this table can
    /// be applied by commitGovernance, and the type must match exactly.
    function _wireGovernanceActions() private {
        bytes32 a = T_GOV_ADJUSTABLE;
        govCategory[GovernanceModule.grantRole.selector] = a;
        govCategory[GovernanceModule.revokeRole.selector] = a;
        govCategory[GovernanceModule.setEvaluatorGasCap.selector] = a;
        govCategory[GovernanceModule.setPolicy.selector] = a;              // -> CONSTITUTION when the target is a governance type
        govCategory[GovernanceModule.setRequiredSets.selector] = a;        // (same)
        govCategory[GovernanceModule.setGroupMultiplier.selector] = T_GOV_CONSTITUTION; // a weight lever: always constitutional
        govCategory[GovernanceModule.setMaxInfluenceShare.selector] = T_GOV_CONSTITUTION;
        govCategory[GovernanceModule.setRequiredCapabilities.selector] = a;
        govCategory[GovernanceModule.setAgentCapability.selector] = a;
        govCategory[GovernanceModule.registerImpl.selector] = a;
        govCategory[GovernanceModule.registerEngine.selector] = a;
        govCategory[GovernanceModule.retireImpl.selector] = a;
        govCategory[GovernanceModule.retireEngine.selector] = a;
        govCategory[GovernanceModule.registerAgent.selector] = a;
        govCategory[GovernanceModule.setAgentActive.selector] = a;
        govCategory[GovernanceModule.invalidateObservation.selector] = a;
        govCategory[GovernanceModule.setTrustMerkleRoot.selector] = a;
        govCategory[CouncilModule.setPositionSource.selector] = T_GOV_CONSTITUTION;
        govCategory[CouncilModule.setCouncilSeats.selector] = T_GOV_CONSTITUTION;
        govCategory[CouncilModule.installCouncil.selector] = T_GOV_COUNCIL;
        govCategory[GovernanceModule.rebootstrapSystem.selector] = T_GOV_REBOOT;
    }

    /// Changes made in this merge are notated on-chain, as the dialect rules require.
    function _genesisNotations() private {
        _genesisNotation(
            keccak256("F_DEADLINE"), keccak256("F_PLANNING_OPEN"), SymbolKind.CONSTANT, SymbolKind.CONSTANT,
            "bit tests whether the planning window is still open, not whether a deadline was missed; logic unchanged"
        );
        _genesisNotation(
            keccak256("declareNotationChange"), keccak256("proposeNotationChange"), SymbolKind.FUNCTION, SymbolKind.FUNCTION,
            "governance-only log with immediate effect became an agent proposal that takes effect only when a dependent transition commits"
        );
        _genesisNotation(
            keccak256("BadLifecycleTransition"), keccak256("IllegalMove"), SymbolKind.TYPE, SymbolKind.TYPE,
            "two names for one error: the guarded-lifecycle revert is IllegalMove everywhere"
        );
        _genesisNotation(
            keccak256("submitConsensusSignal"), keccak256("submitRoundSignal"), SymbolKind.FUNCTION, SymbolKind.FUNCTION,
            "two names for one entry point; the round form is canonical and carries the one-signal-per-round rule"
        );
        _genesisNotation(
            keccak256("AlreadySignalled"), keccak256("AlreadySignalledThisRound"), SymbolKind.TYPE, SymbolKind.TYPE,
            "the guard is per round, not per transition; the name now says so"
        );
        _genesisNotation(
            keccak256("GOVERNANCE"), keccak256("commitGovernance"), SymbolKind.ROLE, SymbolKind.FUNCTION,
            "authority over rules moved from a role to a transition: a rule changes only when a governance transition passes the full commit predicate"
        );
        _genesisNotation(
            keccak256("PROPOSER"), keccak256("ACTIVE_AGENT"), SymbolKind.ROLE, SymbolKind.PREDICATE,
            "any active agent may propose; governance transitions additionally require a council seat"
        );
        _genesisNotation(
            keccak256("selectCandidate"), keccak256("selectCandidate"), SymbolKind.FUNCTION, SymbolKind.FUNCTION,
            "same name, new meaning: permissionless after the consensus window closes; highest-confidence qualifying sibling wins, earlier eligibility breaks ties"
        );
        _genesisNotation(
            keccak256("GPolicy"), keccak256("GPolicy"), SymbolKind.FUNCTION, SymbolKind.FUNCTION,
            "same name, pinned meaning: the highest-matching group multiplier, not the sum of matching groups"
        );
    }

    // ------------------------------------------------------------------------
    // Observations, scenarios, goals
    // ------------------------------------------------------------------------

    function ingestObservation(
        bytes32 id, uint256 timestamp, bytes32 sourceSetHash, bytes32 dataHash,
        bytes32 methodologyHash, uint256 confidence, uint256 expiresAt
    ) external onlyRole(OBSERVER) whenNotPaused {
        if (!(observationStatus[id] == ObservationStatus.NONE)) revert ObsExists();
        if (!(timestamp <= block.timestamp && expiresAt > block.timestamp)) revert BadObservationWindow();
        if (!(confidence <= WAD)) revert BadConfidence();
        observations[id] = ObservationSnapshot(id, timestamp, sourceSetHash, dataHash, methodologyHash, confidence, expiresAt);
        observationStatus[id] = ObservationStatus.INGESTED;
        emit ObservationIngested(id);
    }

    function registerScenario(bytes32 id, bytes32 observationId, bytes32 stateHash, uint256 confidence)
        external whenNotPaused
    {
        if (!agents[msg.sender].active) revert InactiveAgent();
        if (!(scenarios[id].id == bytes32(0))) revert ScenarioExists();
        if (!(observationStatus[observationId] == ObservationStatus.INGESTED)) revert ObservationNotIngested();
        if (!(confidence <= WAD)) revert BadConfidence();
        scenarios[id] = Scenario(id, observationId, stateHash, confidence);
        emit ScenarioRegistered(id);
    }

    function registerGoal(bytes32 id, bytes32 stateHash, bytes32[] calldata objectiveIds, int256[] calldata objectiveVector)
        external whenNotPaused
    {
        if (!agents[msg.sender].active) revert InactiveAgent();
        if (!(goals[id].id == bytes32(0))) revert GoalExists();
        if (!(objectiveIds.length == objectiveVector.length && objectiveIds.length <= MAX_SET)) revert GoalDimensionMismatch();
        Goal storage g = goals[id];
        g.id = id;
        g.stateHash = stateHash;
        g.objectiveIds = objectiveIds;
        g.objectiveVector = objectiveVector;
        emit GoalRegistered(id);
    }

    // ------------------------------------------------------------------------
    // Registration. A transition's policy comes ONLY from policies[type], which
    // only a committed governance transition can write; unknown types revert.
    // ------------------------------------------------------------------------

    function registerTransition(bytes32 id, bytes32 proposedStateId, ManifestInput calldata m) external whenNotPaused {
        if (_isGovType(m.transitionType)) revert GovernanceTypeOnly();
        _register(id, proposedStateId, m);
    }

    /// Only a council seat may propose a change to the rules, and the payload is the
    /// exact call to make: its hash is the manifest's payload hash, and the bytes are
    /// stored so constraint evaluators can read and judge them before any vote.
    function registerGovernanceTransition(
        bytes32 id, bytes32 proposedStateId, ManifestInput calldata m, bytes calldata action
    ) external whenNotPaused {
        if (!isSeated[msg.sender]) revert NotCouncil();
        bytes32 want = _categoryOf(action);
        if (m.transitionType != want) revert WrongGovCategory(m.transitionType, want);
        bytes32 ah = keccak256(action);
        if (m.transitionPayloadHash != ah) revert GovActionMismatch();
        // The council must be unanimous before a governance transition is even proposed:
        // every seat now sitting has to have approved this exact (id, action).
        bytes32 key = keccak256(abi.encode(id, ah));
        for (uint256 i = 0; i < _councilMembers.length; i++) {
            if (!govApproved[key][_councilMembers[i]]) revert CouncilNotUnanimous(_councilMembers[i]);
        }
        _register(id, proposedStateId, m);
        govAction[id] = action;
        emit GovernanceActionProposed(id, want, action);
    }

    /// The transition type an action must travel under. Policy, mandatory sets and
    /// multipliers that target a governance type are constitutional, not adjustable.
    function _categoryOf(bytes calldata action) private view returns (bytes32 cat) {
        if (action.length < 4) revert UnknownGovAction(bytes4(0));
        bytes4 sel = bytes4(action[:4]);
        cat = govCategory[sel];
        if (cat == bytes32(0)) revert UnknownGovAction(sel);
        if (sel == GovernanceModule.setPolicy.selector || sel == GovernanceModule.setRequiredSets.selector) {
            if (action.length < 36) revert UnknownGovAction(sel);
            if (_isGovType(bytes32(action[4:36]))) cat = T_GOV_CONSTITUTION;
        }
    }

    function _register(bytes32 id, bytes32 proposedStateId, ManifestInput calldata m) private {
        if (!agents[msg.sender].active) revert InactiveAgent();
        if (systemState != SystemState.OPERATIONAL && m.transitionType != T_GOV_REBOOT) revert SystemFrozen();
        if (!(transitionLifecycle[id] == TransitionLifecycle.NONE)) revert TransitionExists();
        if (!(m.parentStateId == declaredState.id)) revert ParentNeDeclared();
        if (!(m.expiresAt > block.timestamp)) revert AlreadyExpired();
        if (!(observationStatus[m.observationId] == ObservationStatus.INGESTED)) revert ObsNotIngested();
        if (!(m.relevantGroups.length > 0 && m.relevantGroups.length <= MAX_GROUPS)) revert BadGroups();
        if (!(m.constraints.length <= MAX_SET && m.changedVars.length <= MAX_ARR &&
            m.resources.length <= MAX_ARR && m.expectedEffects.length <= MAX_ARR)) revert ManifestTooLarge();
        Policy memory p = policies[m.transitionType];
        if (!(p.exists)) revert NoPolicyForType();

        Transition storage t = transitions[id];
        t.id = id;
        t.parentStateId = m.parentStateId;
        t.proposedStateId = proposedStateId;
        t.proposer = msg.sender;
        _storeManifest(t.manifest, m);

        transitionPolicy[id] = p;
        bytes32[] memory cs = _union(requiredConstraints[m.transitionType], m.constraints);
        bytes32[] memory os = _union(requiredObjectives[m.transitionType], new bytes32[](0));
        constraintSet[id] = cs;
        objectiveSet[id] = os;
        relevanceSet[id] = requiredRelevance[m.transitionType];
        attributionSet[id] = requiredAttribution[m.transitionType];
        context[id].policyHash = keccak256(abi.encode(
            p, cs, os, requiredRelevance[m.transitionType], requiredAttribution[m.transitionType]
        ));

        transitionLifecycle[id] = TransitionLifecycle.PROPOSED;
        emit LifecycleChanged(id, TransitionLifecycle.NONE, TransitionLifecycle.PROPOSED);
        emit TransitionRegistered(id, m.parentStateId);
    }

    function _storeManifest(Manifest storage mf, ManifestInput calldata m) private {
        mf.parentStateId = m.parentStateId;
        mf.transitionType = m.transitionType;
        mf.relevantGroups = m.relevantGroups;
        mf.changedVars = m.changedVars;
        mf.constraints = m.constraints;
        mf.resources = m.resources;
        mf.expectedEffects = m.expectedEffects;
        mf.expiresAt = m.expiresAt;
        mf.observationId = m.observationId;
        mf.resultingStateHash = m.resultingStateHash;
        mf.transitionPayloadHash = m.transitionPayloadHash;
    }

    /// Deduplicated union of a mandatory (storage) set and proposer additions.
    function _union(bytes32[] storage req, bytes32[] memory extra) private view returns (bytes32[] memory out) {
        uint256 total = req.length + extra.length;
        out = new bytes32[](total);
        uint256 n = 0;
        for (uint256 i = 0; i < total; i++) {
            bytes32 x = i < req.length ? req[i] : extra[i - req.length];
            bool dup = false;
            for (uint256 j = 0; j < n; j++) {
                if (out[j] == x) { dup = true; break; }
            }
            if (!dup) out[n++] = x;
        }
        if (!(n <= MAX_SET)) revert SetGtMAXSET();
        assembly { mstore(out, n) }
    }

    // ------------------------------------------------------------------------
    // The single canonical commit predicate
    // ------------------------------------------------------------------------

    /// Commit(T) = C ∧ (J if required) ∧ Quorum ∧ Conf≥θ ∧ K ∧ ExceptionsCleared ∧ Fresh
    ///           ∧ ¬PlanningOpen ∧ ¬Expired ∧ Lineage ∧ PlansLive ∧ CapabilityCovered ∧ DialectOk
    /// Returns a bitmask of FAILED terms; 0 means the predicate holds. It is the only
    /// definition of "may commit", and it is the only gate on changing the rules.
    function commitFailMask(bytes32 id) public view returns (uint256 m) {
        Transition storage t = transitions[id];
        Policy storage pol = transitionPolicy[id];
        EvaluationResult storage er = evaluationResults[id];
        Tally storage ty = tallies[id];

        if (!er.constraintsSatisfied) m |= F_CONSTRAINTS;
        if (pol.requireObjectives && !er.objectivesSatisfied) m |= F_OBJECTIVES;
        if (ty.totalWeight == 0 || ty.totalWeight < pol.quorumRequired) m |= F_QUORUM;
        if (confidenceOf(id) < pol.confidenceThreshold) m |= F_CONFIDENCE;
        if (!reconciliationResults[id].compatible) m |= F_COMPAT;
        if (unresolvedBlocking[id] != 0) m |= F_EXCEPTIONS;
        if (!_observationFresh(t.manifest.observationId, pol.minObsConfidence)) m |= F_FRESH;
        if (block.timestamp < planningDeadline[id]) m |= F_PLANNING_OPEN;
        if (block.timestamp >= t.manifest.expiresAt) m |= F_EXPIRED;
        if (t.manifest.parentStateId != declaredState.id) m |= F_LINEAGE;
        if (minPlanExpiry[id] == 0 || block.timestamp >= minPlanExpiry[id]) m |= F_PLANS;
        if (!capabilityCovered(id)) m |= F_CAPABILITY;
        if (!dialectDepsOk(id)) m |= F_DIALECT;
    }

    /// VALIDATED -> COMMITTABLE. Permissionless: the predicate is the gate, not a person.
    function declareCommit(bytes32 id) external whenOperational(id) {
        _require(id, TransitionLifecycle.VALIDATED);
        uint256 m = commitFailMask(id);
        if (m != 0) revert CommitBlocked(m);

        Transition storage t = transitions[id];
        Policy storage pol = transitionPolicy[id];
        uint256 exp = t.manifest.expiresAt;
        uint256 pe = minPlanExpiry[id];
        if (pe < exp) exp = pe;
        uint256 oe = observations[t.manifest.observationId].expiresAt;
        if (oe < exp) exp = oe;

        commitConditions[id] = CommitConditions({
            quorumRequired: pol.quorumRequired,
            quorumAchieved: tallies[id].totalWeight,
            planningDeadline: planningDeadline[id],
            executionDeadline: block.timestamp + pol.executionWindow,
            expiresAt: exp
        });
        _set(id, TransitionLifecycle.COMMITTABLE);
        emit CommitDeclared(id);
    }

    /// Re-runs the ENTIRE predicate: an exception or stale observation that
    /// appeared after declareCommit stops the commit.
    function _commitCore(bytes32 id) private {
        _require(id, TransitionLifecycle.COMMITTABLE);
        uint256 m = commitFailMask(id);
        if (m != 0) revert CommitBlocked(m);
        if (!(block.timestamp < commitConditions[id].expiresAt)) revert CommitWindowClosed();

        Transition storage t = transitions[id];
        declaredState = State({
            id: t.proposedStateId,
            index: declaredState.index + 1,
            stateHash: t.manifest.resultingStateHash
        });
        committedIndex[id] = declaredState.index;

        // Freeze attribution rule versions at commit time.
        bytes32[] storage aids = attributionSet[id];
        EvaluationContext storage ctx = context[id];
        for (uint256 i = 0; i < aids.length; i++) {
            (, uint256 v) = _activeVersion(_attributionReg[aids[i]]);
            ctx.attributionVersions.push(v);
        }

        _set(id, TransitionLifecycle.COMMITTED);
        _commitDialectDeps(id);              // dialect changes take effect with the commit that relies on them
        acceptedProposals[t.proposer] += 1;  // counts toward council candidacy
        emit TransitionCommitted(id, declaredState.id);
    }

    function commit(bytes32 id) external whenOperational(id) {
        if (_isGovType(transitions[id].manifest.transitionType)) revert UseGovernanceCommit();
        _commitCore(id);
    }

    /// Commits a governance transition and applies its action in the same transaction.
    /// If the action fails the whole commit reverts, so a committed governance
    /// transition always means an applied one. The action runs as the kernel calling
    /// itself: the only caller `onlyGovernance` accepts after genesis.
    function commitGovernance(bytes32 id) external whenOperational(id) {
        if (!_isGovType(transitions[id].manifest.transitionType)) revert NotGovernanceTransition();
        _commitCore(id);
        bytes memory action = govAction[id];
        (bool ok, bytes memory ret) = address(this).call(action);
        if (!ok) revert GovActionFailed(ret);
        govApplied[id] = true;
        emit GovernanceApplied(id, bytes4(action));
    }

    // ------------------------------------------------------------------------
    // Lifecycle housekeeping (permissionless where safe)
    // ------------------------------------------------------------------------

    function expire(bytes32 id) external {
        if (!(_isPreCommit(transitionLifecycle[id]))) revert NotPreCommit();
        if (!(block.timestamp >= transitions[id].manifest.expiresAt)) revert NotExpired();
        _set(id, TransitionLifecycle.EXPIRED);
    }

    /// Sibling transitions lose once another transition advanced the declared state.
    function markConflicted(bytes32 id) external {
        if (!(_isPreCommit(transitionLifecycle[id]))) revert NotPreCommit();
        if (!(transitions[id].manifest.parentStateId != declaredState.id)) revert ParentStillCurrent();
        _set(id, TransitionLifecycle.CONFLICTED);
    }

    /// A proposer may withdraw its own pre-commit transition.
    function withdraw(bytes32 id) external {
        if (msg.sender != transitions[id].proposer) revert NotTransitionProposer();
        if (!(_isPreCommit(transitionLifecycle[id]))) revert NotPreCommit();
        _set(id, TransitionLifecycle.DISCARDED);
    }

    // ------------------------------------------------------------------------
    // Execution and settlement. The attestor logic is unchanged: reality is
    // asserted by `settlementThreshold` distinct SETTLERs agreeing on one (stateId, hash).
    // ------------------------------------------------------------------------

    function startExecution(bytes32 id) external onlyRole(EXECUTOR) whenOperational(id) {
        _require(id, TransitionLifecycle.COMMITTED);
        if (!(block.timestamp <= commitConditions[id].executionDeadline)) revert ExecutionWindowClosed();
        _set(id, TransitionLifecycle.EXECUTING);
        emit ExecutionStarted(id);
    }

    function attestSettlement(bytes32 id, bytes32 realizedStateId, bytes32 realizedStateHash)
        external onlyRole(SETTLER) whenOperational(id)
    {
        _require(id, TransitionLifecycle.EXECUTING);
        if (!(!settlementAttested[id][msg.sender])) revert AlreadyAttested();
        settlementAttested[id][msg.sender] = true;
        bytes32 key = keccak256(abi.encode(realizedStateId, realizedStateHash));
        uint256 votes = ++settlementVotes[id][key];
        emit SettlementAttested(id, msg.sender, key);
        if (votes >= transitionPolicy[id].settlementThreshold) {
            realizedStates[id] = State(realizedStateId, committedIndex[id], realizedStateHash);
            _set(id, TransitionLifecycle.SETTLED);
            emit ExecutionSettled(id);
        }
    }

    // ------------------------------------------------------------------------
    // Views
    // ------------------------------------------------------------------------

    function paused() external view returns (bool) { return block.timestamp < pausedUntil; }

    function getTransition(bytes32 id) external view returns (Transition memory) { return transitions[id]; }

    function contextOf(bytes32 id) external view returns (EvaluationContext memory) { return context[id]; }

    /// The exact call a governance transition will make; constraint evaluators read this.
    function governanceActionOf(bytes32 id) external view returns (bytes memory) { return govAction[id]; }

    fallback() external {
        address m = moduleOf[msg.sig];
        if (m == address(0)) revert UnknownSelector();
        assembly {
            calldatacopy(0, 0, calldatasize())
            let ok := delegatecall(gas(), m, 0, calldatasize(), 0, 0)
            returndatacopy(0, 0, returndatasize())
            switch ok
            case 0 { revert(0, returndatasize()) }
            default { return(0, returndatasize()) }
        }
    }
}
