// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./KernelStorage.sol";

/// Delegatecalled by the kernel: every function that changes a RULE.
///
/// Authority over these functions is `onlyGovernance`: after sealGenesis() it is
/// satisfied only when the kernel calls itself from commitGovernance(), which
/// happens only after a governance transition has passed every term of the
/// commit predicate. Nothing else, and nobody, can change a rule.
///
/// Never call this contract directly: its own storage is empty and every check fails there.
contract GovernanceModule is KernelStorage {
    // ------------------------------------------------------------------------
    // Genesis
    // ------------------------------------------------------------------------

    /// Retires the bootstrap authority for good. The kernel refuses to seal until the
    /// machinery that replaces the authority exists: a full council, and a policy for
    /// each governance type (so governance transitions can actually reach quorum).
    function sealGenesis() external {
        if (msg.sender != genesisAdmin) revert NotGenesis();
        if (genesisSealed) revert GenesisIsSealed();
        if (_councilMembers.length != councilSeats || councilSeats == 0) revert GenesisNotReady("COUNCIL");
        if (!policies[T_GOV_ADJUSTABLE].exists) revert GenesisNotReady("GOV_ADJUSTABLE");
        if (!policies[T_GOV_CONSTITUTION].exists) revert GenesisNotReady("GOV_CONSTITUTION");
        if (!policies[T_GOV_COUNCIL].exists) revert GenesisNotReady("GOV_COUNCIL");
        if (!policies[T_GOV_REBOOT].exists) revert GenesisNotReady("GOV_REBOOT");
        if (largestBaseInfluence * WAD > maxInfluenceShare * totalBaseInfluence) revert GenesisNotReady("INFLUENCE_SHARE");
        genesisSealed = true;
        genesisAdmin = address(0);
        emit GenesisSealedEvent();
    }

    /// A seated council member signs off on one exact proposal (transition id + action).
    /// registerGovernanceTransition refuses to open a proposal until EVERY seat
    /// currently sitting has approved it: the council must be unanimous before a
    /// governance transition even reaches the commit process. Unanimity only opens
    /// the door; the commit predicate still decides.
    function approveGovernance(bytes32 transitionId, bytes32 actionHash) external whenNotPaused {
        if (!isSeated[msg.sender]) revert NotCouncil();
        govApproved[keccak256(abi.encode(transitionId, actionHash))][msg.sender] = true;
        emit GovernanceApproved(transitionId, actionHash, msg.sender);
    }

    // ------------------------------------------------------------------------
    // Emergency brake. Bounded and self-expiring, so a pause can never
    // deadlock the governance that would be needed to end it.
    // ------------------------------------------------------------------------

    function pause() external {
        if (!(hasRole[GUARDIAN][msg.sender] || (!genesisSealed && msg.sender == genesisAdmin))) revert NotGuardian();
        if (block.timestamp < nextPauseAllowed) revert PauseCooldown();
        pausedUntil = block.timestamp + PAUSE_MAX;
        nextPauseAllowed = pausedUntil + PAUSE_MAX; // equal cooldown: a window in which transitions can run
        emit PausedSet(pausedUntil);
    }

    function unpause() external {
        if (!(hasRole[GUARDIAN][msg.sender] || (!genesisSealed && msg.sender == genesisAdmin))) revert NotGuardian();
        pausedUntil = 0;
        emit PausedSet(0);
    }

    // ------------------------------------------------------------------------
    // Rule changes (reached only through commitGovernance after genesis)
    // ------------------------------------------------------------------------

    function grantRole(bytes32 role, address who) external onlyGovernance {
        hasRole[role][who] = true;
        emit RoleGranted(role, who);
    }

    function revokeRole(bytes32 role, address who) external onlyGovernance {
        hasRole[role][who] = false;
        emit RoleRevoked(role, who);
    }

    function setEvaluatorGasCap(uint256 cap) external onlyGovernance {
        if (!(cap >= 50_000 && cap <= 3_000_000)) revert GasCapRange();
        evaluatorGasCap = cap;
    }

    function setPolicy(bytes32 transitionType, Policy calldata p) external onlyGovernance {
        if (!(p.confidenceThreshold <= WAD && p.minObsConfidence <= WAD)) revert WadOutOfRange();
        if (!(p.maxPlans > 0 && p.maxPlans <= MAX_PLANS_HARD)) revert BadMaxPlans();
        if (!(p.settlementThreshold > 0)) revert BadSettlementThreshold();
        // At least one round, and a window long enough that every round is non-empty.
        if (!(p.maxConsensusRounds > 0 && p.consensusWindow >= p.maxConsensusRounds)) revert BadRounds();
        Policy memory q = p;
        q.exists = true;
        policies[transitionType] = q;
        emit PolicySet(transitionType);
    }

    function setRequiredSets(
        bytes32 transitionType,
        bytes32[] calldata constraintIds,
        bytes32[] calldata objectiveIds,
        bytes32[] calldata relevanceIds,
        bytes32[] calldata attributionIds
    ) external onlyGovernance {
        if (!(constraintIds.length <= MAX_SET && objectiveIds.length <= MAX_SET &&
            relevanceIds.length <= MAX_SET && attributionIds.length <= MAX_ATTR_RULES)) revert SetTooLarge();
        requiredConstraints[transitionType] = constraintIds;
        requiredObjectives[transitionType] = objectiveIds;
        requiredRelevance[transitionType] = relevanceIds;
        requiredAttribution[transitionType] = attributionIds;
    }

    function setRequiredCapabilities(bytes32 transitionType, CapabilityRequirement[] calldata reqs) external onlyGovernance {
        if (!(reqs.length <= MAX_SET)) revert SetTooLarge();
        delete requiredCapabilities[transitionType];
        for (uint256 i = 0; i < reqs.length; i++) requiredCapabilities[transitionType].push(reqs[i]);
    }

    function setAgentCapability(address agent, bytes32 capabilityId, bool held) external onlyGovernance {
        if (!(repHistory[agent].length != 0)) revert UnknownAgent();
        agentCapabilities[agent][capabilityId] = held;
    }

    /// Constitutional: the cap on any one agent's share of base influence (WAD).
    function setMaxInfluenceShare(uint256 share) external onlyGovernance {
        if (!(share > 0 && share <= WAD)) revert WadOutOfRange();
        // Never tighten below what an existing agent already holds: that would void a seated agent.
        if (largestBaseInfluence * WAD > share * totalBaseInfluence) revert InfluenceShareExceeded();
        maxInfluenceShare = share;
    }

    function setGroupMultiplier(bytes32 transitionType, bytes32 group, uint256 mult) external onlyGovernance {
        if (!(mult <= MAX_GROUP_MULT)) revert BadMultiplier();
        groupMultiplier[transitionType][group] = mult;
    }

    /// Append-only: a (kind,id,version) -> impl mapping never changes once written.
    function registerImpl(RegistryKind kind, bytes32 id, address impl) external onlyGovernance {
        Version[] storage vs = _reg(kind)[id];
        if (!(vs.length < MAX_VERSIONS)) revert TooManyVersions();
        if (!(impl.code.length > 0)) revert NotAContract();
        uint64 at = uint64(block.timestamp + registryDelay);
        vs.push(Version(impl, at, false));
        emit ImplRegistered(kind, id, vs.length - 1, impl, at);
    }

    function registerEngine(address impl) external onlyGovernance {
        if (!(_engineReg.length < MAX_VERSIONS)) revert TooManyVersions();
        if (!(impl.code.length > 0)) revert NotAContract();
        uint64 at = uint64(block.timestamp + registryDelay);
        _engineReg.push(Version(impl, at, false));
        emit EngineRegistered(_engineReg.length - 1, impl, at);
    }

    /// Retiring prevents NEW transitions from binding a version; history still resolves.
    function retireImpl(RegistryKind kind, bytes32 id, uint256 version) external onlyGovernance {
        _reg(kind)[id][version].retired = true;
    }

    function retireEngine(uint256 version) external onlyGovernance {
        _engineReg[version].retired = true;
    }

    function implOf(RegistryKind kind, bytes32 id, uint256 version) external view returns (address) {
        return _reg(kind)[id][version].impl;
    }

    function registerAgent(address agent, bytes32[] calldata groups, uint256 baseInfluence) external onlyGovernance {
        if (!(repHistory[agent].length == 0)) revert AgentExists();
        if (!(groups.length <= MAX_GROUPS && baseInfluence <= MAX_INFLUENCE)) revert OutOfBounds();
        // The whale bound, in weight form: no single agent may carry more than
        // maxInfluenceShare of all base influence, so no individual can outweigh the
        // collective. Influence never exceeds base (reputation only scales it down),
        // so bounding base bounds weight. Genesis seeds freely and is checked once at seal.
        totalBaseInfluence += baseInfluence;
        if (baseInfluence > largestBaseInfluence) largestBaseInfluence = baseInfluence;
        if (genesisSealed && baseInfluence * WAD > maxInfluenceShare * totalBaseInfluence) revert InfluenceShareExceeded();
        AgentProfile storage ap = agents[agent];
        ap.active = true;
        ap.groups = groups;
        ap.baseInfluence = baseInfluence;
        repHistory[agent].push(RepCheckpoint(uint64(block.timestamp), 0));
        emit AgentRegistered(agent);
    }

    function setAgentActive(address agent, bool active) external onlyGovernance {
        if (!(repHistory[agent].length != 0)) revert UnknownAgent();
        agents[agent].active = active;
    }

    function invalidateObservation(bytes32 id) external onlyGovernance {
        if (!(observationStatus[id] == ObservationStatus.INGESTED)) revert NotIngested();
        observationStatus[id] = ObservationStatus.INVALID;
    }

    function setTrustMerkleRoot(bytes32 newRoot) external onlyGovernance {
        trustMerkleRoot = newRoot;
        emit TrustMerkleRootUpdated(newRoot);
    }

    function verifyTrustProof(address source, address target, uint256 trustScore, bytes32[] calldata merkleProof)
        external view returns (bool)
    {
        bytes32 h = keccak256(abi.encodePacked(source, target, trustScore));
        for (uint256 i = 0; i < merkleProof.length; i++) {
            bytes32 p = merkleProof[i];
            h = h <= p ? keccak256(abi.encodePacked(h, p)) : keccak256(abi.encodePacked(p, h));
        }
        return h == trustMerkleRoot;
    }

    /// Leaves the frozen state. Reachable only by a committed T_GOV_REBOOT transition,
    /// the one type whose lifecycle is allowed to run while the substrate is frozen.
    function rebootstrapSystem() external onlyGovernance {
        if (systemState != SystemState.COLLAPSED_FROZEN) revert NotFrozen();
        systemState = SystemState.OPERATIONAL;
        emit SystemStateChanged(SystemState.COLLAPSED_FROZEN, SystemState.OPERATIONAL);
    }
}
