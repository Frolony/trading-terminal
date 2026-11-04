// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./KernelStorage.sol";

/// Delegatecalled by the kernel: proposals to change what a symbol means.
///
/// A NotationChange is a PROPOSAL, not an effect. It takes effect only when a
/// transition that declared a dependency on it commits, so it rides the same
/// process as every other proposal. Pull, not push: nothing on the evaluation
/// path reads this registry; only a proposer pays to read it, by citing the
/// committed head it is changing (prevChangeId).
contract DialectModule is KernelStorage {
    struct NotationInput {
        bytes32 oldSymbol;
        bytes32 newSymbol;
        SymbolKind oldKind;
        SymbolKind newKind;
        bytes32 definitionHash;
        bytes32 prevChangeId;
        string reason;
    }

    function proposeNotationChange(NotationInput calldata p) external whenNotPaused returns (bytes32 id) {
        if (!agents[msg.sender].active) revert InactiveAgent();
        if (p.oldSymbol == bytes32(0) || p.newSymbol == bytes32(0)) revert EmptySymbol();
        bytes32 head = dialectHead[p.oldSymbol];
        if (p.prevChangeId != head) revert StalePrevChange(head, p.prevChangeId);
        _checkLegibleChange(p, head);

        id = keccak256(abi.encode(msg.sender, p.oldSymbol, p.newSymbol, p.prevChangeId, ++notationNonce));
        notationChanges[id] = NotationChange({
            oldSymbol: p.oldSymbol,
            newSymbol: p.newSymbol,
            oldKind: p.oldKind,
            newKind: p.newKind,
            definitionHash: p.definitionHash,
            prevChangeId: p.prevChangeId,
            reason: p.reason,
            proposer: msg.sender,
            proposedAt: uint64(block.timestamp),
            committedAt: 0,
            committedEpoch: 0,
            status: NotationStatus.PROPOSED
        });
        emit NotationProposed(id, p.oldSymbol, p.newSymbol, notationChanges[id]);
    }

    /// The cited prior state must be the one being changed, and the proposal must
    /// change something. A head of 0 means the symbol still has its original meaning.
    function _checkLegibleChange(NotationInput calldata p, bytes32 head) private view {
        if (head == bytes32(0)) return;
        NotationChange storage h = notationChanges[head];
        if (h.newSymbol != p.oldSymbol) return; // head retired this name; redefining it starts fresh
        if (p.oldKind != h.newKind) revert PriorKindMismatch(h.newKind, p.oldKind);
        if (p.newSymbol == p.oldSymbol && p.newKind == h.newKind && p.definitionHash == h.definitionHash) {
            revert NoOpChange();
        }
    }

    /// A transition's proposer declares that the transition depends on a dialect
    /// change; the change takes effect if and when the transition commits.
    function declareDialectDependency(bytes32 transitionId, bytes32 changeId) external whenNotPaused {
        Transition storage t = transitions[transitionId];
        if (t.proposer == address(0) || t.proposer != msg.sender) revert NotTransitionProposer();
        if (!_isPreCommit(transitionLifecycle[transitionId])) revert NotPreCommit();
        NotationChange storage c = notationChanges[changeId];
        if (c.status != NotationStatus.PROPOSED) revert UnknownNotationChange();

        bytes32[] storage deps = dialectDeps[transitionId];
        if (deps.length >= MAX_DIALECT_DEPS) revert DialectDepCap();
        // Every change moves dialectHead for both of its names, so two dependencies of
        // one transition may not share ANY symbol (old or new): same old name, two names
        // renamed to one, and chains like X->Y plus Y->Z would all pass the predicate
        // yet go stale part-way through commit().
        for (uint256 i = 0; i < deps.length; i++) {
            NotationChange storage d = notationChanges[deps[i]];
            if (
                d.oldSymbol == c.oldSymbol || d.oldSymbol == c.newSymbol ||
                d.newSymbol == c.oldSymbol || d.newSymbol == c.newSymbol
            ) revert DialectDepConflict();
        }
        deps.push(changeId);
        emit DialectDependencyDeclared(transitionId, changeId);
    }
}
