// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./KernelStorage.sol";

/// Delegatecalled by the kernel: epochs, the position gate, and the council.
///
/// Position is a GATE on candidacy and nothing else. The kernel verifies that a
/// locked holding exists and stays locked through an epoch, then records only
/// a streak counter. The amount is never read into storage, so position cannot
/// weight a signal, a quorum, a confidence, a vote on governance, or a commit.
contract CouncilModule is KernelStorage {
    function councilMembers() external view returns (address[] memory) {
        return _councilMembers;
    }

    /// Epoch e counts as "held" for an agent only if, when recorded during e, the
    /// holding is locked past the END of e. Anyone may record for anyone, once per
    /// epoch; a missed epoch resets the streak because nothing was verified.
    function recordPosition(address agent) external whenNotPaused {
        if (positionSource == address(0)) revert NoPositionSource();
        if (!agents[agent].active) revert InactiveAgent();
        uint256 e = currentEpoch();
        uint256 last = lastPositionEpoch1[agent]; // epoch + 1
        if (last == e + 1) revert AlreadyRecorded();

        bool held;
        (bool ok, bytes memory ret) = positionSource.staticcall{gas: evaluatorGasCap}(
            abi.encodeCall(ILockedHolding.lockedHolding, (agent))
        );
        if (ok && ret.length == 64) {
            (uint256 amount, uint256 lockedUntil) = abi.decode(ret, (uint256, uint256));
            held = amount > 0 && lockedUntil >= epochZero + (e + 1) * epochLength;
        }
        uint256 s = 0;
        if (held) s = (last != 0 && last == e) ? positionStreak[agent] + 1 : 1; // consecutive iff recorded in e-1
        positionStreak[agent] = s;
        lastPositionEpoch1[agent] = e + 1;
        emit PositionRecorded(agent, e, s);
    }

    /// The selection rule, exactly as the charter states it:
    ///   - holds a position (verified, recorded in this or the previous epoch),
    ///   - has held it for POSITION_EPOCHS epochs,
    ///   - has at least one accepted (committed) proposal,
    /// plus two conditions that follow from the council's role: the agent is an
    /// active agent, and has no confirmed defection inside the cooldown.
    function isCandidate(address agent) public view returns (bool) {
        if (!agents[agent].active) return false;
        if (acceptedProposals[agent] == 0) return false;
        uint256 e = currentEpoch();
        if (lastPositionEpoch1[agent] < e) return false;            // not verified in e-1 or e
        if (positionStreak[agent] < POSITION_EPOCHS) return false;
        uint256 d = lastDefectionEpoch1[agent];
        if (d != 0 && e < (d - 1) + DEFECTION_COOLDOWN_EPOCHS) return false;
        return true;
    }

    /// Installs a council. Reached by a committed T_GOV_COUNCIL transition (or by the
    /// genesis authority before seal, who is exempt only because no agent has any
    /// history yet). The consensus process chooses among eligible candidates: the
    /// kernel checks that every nominee is eligible, that seats are distinct, and
    /// that the sitting council has served its minimum. It never ranks candidates,
    /// so there is no tie-break table to maintain and position never ranks anyone.
    function installCouncil(address[] calldata seats) external onlyGovernance {
        if (!(seats.length == councilSeats && seats.length > 0)) revert BadSeats();
        bool viaGenesis = msg.sender != address(this);
        uint256 e = currentEpoch();
        if (!viaGenesis && _councilMembers.length != 0 && e < councilSince + MIN_SITTING_EPOCHS) {
            revert SittingTooShort(councilSince, e);
        }
        for (uint256 i = 0; i < _councilMembers.length; i++) isSeated[_councilMembers[i]] = false;
        delete _councilMembers;
        for (uint256 i = 0; i < seats.length; i++) {
            address a = seats[i];
            if (isSeated[a]) revert BadSeats(); // duplicate
            if (!agents[a].active) revert InactiveAgent();
            if (!viaGenesis && !isCandidate(a)) revert NotCandidate(a);
            isSeated[a] = true;
            _councilMembers.push(a);
        }
        councilSince = e;
        emit CouncilInstalled(e, seats);
    }

    /// Seat count: odd, so a strict majority always exists. Takes effect at the next install.
    function setCouncilSeats(uint256 seats) external onlyGovernance {
        if (!(seats > 0 && seats <= MAX_SEATS && seats % 2 == 1)) revert BadSeats();
        councilSeats = seats;
    }

    function setPositionSource(address source) external onlyGovernance {
        if (source != address(0) && source.code.length == 0) revert NotAContract();
        positionSource = source;
    }
}
