// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./KernelStorage.sol";

/// Delegatecalled by the kernel: rounds of consensus, the Stance x Contribution
/// classifier that records each agent's niche, and exception signals.
contract ConsensusModule is KernelStorage {
    // ------------------------------------------------------------------------
    // The classifier tables, packed. Index = stance * 6 + contribution.
    //
    // SECTOR_TABLE: 4 bits per cell, cell 0 in the lowest nibble.
    // WEIGHT_TABLE: 8 bits per cell, in units of WAD / 20 (so 20 = 1.0, 30 = 1.5).
    //
    // The cells are the ones in the uploaded fork. That file called every cell
    // "a proposal to correct"; they stay exactly as uploaded and a test pins all
    // 48 values, so a change to any cell is a visible, reviewed change.
    //
    //                 SUBST  PROC   INFO   OBSTR  ABST   SPEC
    //  ALIGNED  sec:  COLLAB ALIGN  ALIGN  DIVERG ABSTAI SPECIA   w: 1.0 1.0 1.0 0.5 0 1.5
    //  OPPOSED  sec:  ADVERS MISALI MISALI ADVERS ABSTAI SPECIA   w: 1.0 1.0 1.0 1.0 0 1.5
    //  NEUTRAL  sec:  DIVERG UNCERT UNCERT DIVERG ABSTAI SPECIA   w: 0.5 0.1 0.1 0.5 0 1.5
    //  UNCERT   sec:  DIVERG UNCERT UNCERT DISASS ABSTAI SPECIA   w: 0.5 0.1 0.1 0.25 0 1.5
    // ------------------------------------------------------------------------
    uint256 internal constant SECTOR_TABLE = 0x576883573883574114573002;
    uint256 internal constant WEIGHT_TABLE = 0x1e000502020a1e000a02020a1e00141414141e000a141414;

    function classifyStance(uint256 support) public pure returns (Stance) {
        if (support == UNCERTAIN_SUPPORT) return Stance.UNCERTAIN;
        if (support > NEUTRAL_BAND_HIGH) return Stance.ALIGNED;
        if (support < NEUTRAL_BAND_LOW) return Stance.OPPOSED;
        return Stance.NEUTRAL;
    }

    function sectorOf(Stance s, Contribution c) public pure returns (Sector) {
        uint256 idx = uint256(s) * 6 + uint256(c);
        return Sector((SECTOR_TABLE >> (idx * 4)) & 0xf);
    }

    /// The strength a classified participation adds to the agent's niche score.
    function baseWeightOf(Stance s, Contribution c) public pure returns (uint256) {
        uint256 idx = uint256(s) * 6 + uint256(c);
        return ((WEIGHT_TABLE >> (idx * 8)) & 0xff) * WAD / 20;
    }

    /// The agent's niche: the sector where its accumulated table weight is highest
    /// (ties go to the lower sector index). `found` is false for an agent with no
    /// weight yet. Informational: it is read by relevance functions and by people,
    /// never by the consensus arithmetic.
    function nicheOf(address agent) external view returns (bool found, Sector sector, uint256 score) {
        for (uint256 i = 0; i < 9; i++) {
            uint256 v = nicheScore[agent][Sector(i)];
            if (v > score) { score = v; sector = Sector(i); found = true; }
        }
    }

    // ------------------------------------------------------------------------
    // Rounds
    // ------------------------------------------------------------------------

    /// Rounds are slices of the consensus window, derived from time: round r runs
    /// in [eligibleAt + r*slice, eligibleAt + (r+1)*slice). Nobody advances a round,
    /// so nobody can advance one early to cut others off.
    function roundOf(bytes32 id) public view returns (uint256) {
        uint256 n = transitionPolicy[id].maxConsensusRounds;
        uint256 slice = (consensusDeadline[id] - eligibleAt[id]) / n;
        uint256 r = (block.timestamp - eligibleAt[id]) / slice;
        return r >= n ? n - 1 : r;
    }

    /// One signal per agent per round; revising means signalling in a later round.
    /// The latest signal per agent is the one that counts in the tally.
    function submitRoundSignal(bytes32 id, uint256 rawWeight, uint256 support) external whenOperational(id) {
        _require(id, TransitionLifecycle.ELIGIBLE);
        _notExpired(id);
        if (!(block.timestamp < consensusDeadline[id])) revert ConsensusClosed();
        // Raw weight and support are capped at WAD; UNCERTAIN_SUPPORT is the one other legal support.
        if (!(rawWeight <= WAD && (support <= WAD || support == UNCERTAIN_SUPPORT))) revert ValueOutOfRange();
        uint256 round = roundOf(id);
        if (!(roundSignals[id][round][msg.sender].at == 0)) revert AlreadySignalledThisRound();
        (bool ok, uint256 mult) = _eligible(msg.sender, id);
        if (!ok) revert NotEligible();

        _checkMonotone(id, support);

        // "No position" cannot help a commit: an UNCERTAIN signal is recorded and
        // classified but adds no weight to quorum or to confidence.
        uint256 w = support == UNCERTAIN_SUPPORT ? 0 : _effectiveWeight(id, msg.sender, rawWeight, mult);
        _record(id, round, rawWeight, w, support);
        _classify(id, round, rawWeight, w, support);
        emit SignalRevised(id, round, msg.sender, support);
    }

    /// A revision must move toward the mean the agent last saw (the mean of the
    /// round of its previous signal), never away from it.
    function _checkMonotone(bytes32 id, uint256 support) private view {
        uint256 last = lastSignalRound1[id][msg.sender];
        if (last == 0 || support == UNCERTAIN_SUPPORT) return;
        ConsensusSignal storage prev = roundSignals[id][last - 1][msg.sender];
        if (prev.support == UNCERTAIN_SUPPORT) return;
        Tally storage rt = roundTallies[id][last - 1];
        if (rt.totalWeight == 0) return;
        uint256 mean = rt.weightedSupport * WAD / rt.totalWeight;
        uint256 prevDiff = prev.support > mean ? prev.support - mean : mean - prev.support;
        uint256 currDiff = support > mean ? support - mean : mean - support;
        if (currDiff > prevDiff) revert MonotoneRevisionViolation();
    }

    function _record(bytes32 id, uint256 round, uint256 rawWeight, uint256 w, uint256 support) private {
        Tally storage ty = tallies[id];
        uint256 last = lastSignalRound1[id][msg.sender];
        if (last != 0) {
            ConsensusSignal storage prev = roundSignals[id][last - 1][msg.sender];
            uint256 ps = prev.support == UNCERTAIN_SUPPORT ? 0 : prev.support;
            ty.totalWeight -= prev.effectiveWeight;
            ty.weightedSupport -= prev.effectiveWeight * ps / WAD;
        } else {
            ty.count += 1;
        }
        uint256 s = support == UNCERTAIN_SUPPORT ? 0 : support;
        ty.totalWeight += w;
        ty.weightedSupport += w * s / WAD;

        Tally storage rt = roundTallies[id][round];
        rt.totalWeight += w;
        rt.weightedSupport += w * s / WAD;
        rt.count += 1;

        roundSignals[id][round][msg.sender] = ConsensusSignal(msg.sender, rawWeight, w, support, uint64(block.timestamp));
        lastSignalRound1[id][msg.sender] = round + 1;
    }

    // ------------------------------------------------------------------------
    // Classification (informational; never enters the arithmetic above)
    // ------------------------------------------------------------------------

    /// Contribution is read from facts the kernel already holds. Nobody sets it:
    ///  ABSTAINING   the agent signalled with zero raw weight
    ///  OBSTRUCTIVE  the agent has an unresolved blocking exception on this transition
    ///  SPECIALIST   the agent holds a capability this transition type requires
    ///  PROCEDURAL   the agent proposed this transition
    ///  INFORMATIONAL the signal carries no effective weight (no relevance, no influence, no position)
    ///  SUBSTANTIVE  everything else
    /// Earlier lines win over later ones.
    function _contributionOf(bytes32 id, uint256 rawWeight, uint256 w) private view returns (Contribution) {
        if (rawWeight == 0) return Contribution.ABSTAINING;
        if (blockingBy[id][msg.sender] > 0) return Contribution.OBSTRUCTIVE;
        CapabilityRequirement[] storage reqs = requiredCapabilities[transitions[id].manifest.transitionType];
        for (uint256 i = 0; i < reqs.length; i++) {
            if (agentCapabilities[msg.sender][reqs[i].capabilityId]) return Contribution.SPECIALIST;
        }
        if (transitions[id].proposer == msg.sender) return Contribution.PROCEDURAL;
        if (w == 0) return Contribution.INFORMATIONAL;
        return Contribution.SUBSTANTIVE;
    }

    function _classify(bytes32 id, uint256 round, uint256 rawWeight, uint256 w, uint256 support) private {
        Stance st = classifyStance(support);
        Contribution c = _contributionOf(id, rawWeight, w);
        Sector sec = sectorOf(st, c);
        emit SectorClassified(id, msg.sender, round, st, c, sec);
        uint8 prior = lastSector1[id][msg.sender];
        if (prior != 0 && prior - 1 != uint8(sec)) emit SectorReclassified(id, msg.sender, round, Sector(prior - 1), sec);
        lastSector1[id][msg.sender] = uint8(sec) + 1;
        nicheScore[msg.sender][sec] += baseWeightOf(st, c);
    }

    // ------------------------------------------------------------------------
    // Exceptions: safety is separate from consensus
    // ------------------------------------------------------------------------

    function submitExceptionSignal(bytes32 id, uint256 severity, ExceptionDisposition disposition, address offender)
        external whenOperational(id)
    {
        TransitionLifecycle cur = transitionLifecycle[id];
        if (!(_isPreCommit(cur) && uint8(cur) >= 3)) revert ExceptionsNotAccepted();
        if (!(agents[msg.sender].active)) revert InactiveAgent();
        ExceptionSignal[] storage arr = exceptionSignals[id];
        if (!(arr.length < MAX_EXCEPTIONS)) revert ExceptionCap();

        uint256 idx = arr.length;
        arr.push(ExceptionSignal({
            id: keccak256(abi.encode(id, msg.sender, idx)),
            agent: msg.sender,
            offender: offender,
            severity: severity,
            disposition: disposition,
            resolved: false,
            resolutionKind: ResolutionKind.UNRESOLVED,
            resolutionEvidenceHash: bytes32(0)
        }));
        if (disposition == ExceptionDisposition.BLOCK || disposition == ExceptionDisposition.EMERGENCY) {
            unresolvedBlocking[id] += 1;
            blockingBy[id][msg.sender] += 1;
            if (cur == TransitionLifecycle.VALIDATED || cur == TransitionLifecycle.COMMITTABLE) {
                _set(id, TransitionLifecycle.BLOCKED);
            }
        }
        emit ExceptionSignalSubmitted(id, msg.sender, idx);
    }

    /// Council seats adjudicate facts, not rules: an exception resolves when a
    /// strict majority of ALL seats agrees on the same (kind, evidence). The raiser
    /// cannot vote on its own exception. A confirmed defection is recorded against
    /// the offender, which keeps them off the council for DEFECTION_COOLDOWN_EPOCHS.
    function resolveException(bytes32 id, uint256 index, ResolutionKind kind, bytes32 evidenceHash)
        external whenNotPaused
    {
        if (!isSeated[msg.sender]) revert NotCouncil();
        if (kind == ResolutionKind.UNRESOLVED) revert ValueOutOfRange();
        ExceptionSignal storage e = exceptionSignals[id][index];
        if (e.resolved) revert AlreadyResolved();
        if (e.agent == msg.sender) revert RaiserCannotResolveOwnBlockingException();
        bytes32 key = keccak256(abi.encode(id, index, kind, evidenceHash));
        if (resolutionVoted[key][msg.sender]) revert AlreadyVoted();
        resolutionVoted[key][msg.sender] = true;
        if (++resolutionVotes[key] * 2 <= _councilMembers.length) return;

        e.resolved = true;
        e.resolutionKind = kind;
        e.resolutionEvidenceHash = evidenceHash;
        if (e.disposition == ExceptionDisposition.BLOCK || e.disposition == ExceptionDisposition.EMERGENCY) {
            unresolvedBlocking[id] -= 1;
            blockingBy[id][e.agent] -= 1;
        }
        if (kind == ResolutionKind.DEFECTION_CONFIRMED && e.offender != address(0)) {
            lastDefectionEpoch1[e.offender] = currentEpoch() + 1;
        }
        emit ExceptionResolved(id, index, kind, evidenceHash);
    }
}
