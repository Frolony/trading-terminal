// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

interface IConstraint {
    function validate(bytes32 stateId, bytes32 transitionId, bytes32 observationId)
        external view returns (bool pass, uint8 status); // 0=UNAVAILABLE,1=PASS,2=FAIL
}

interface IObjective {
    function evaluate(bytes32 stateId, bytes32 transitionId, bytes32 observationId)
        external view returns (int256 score, bool satisfied);
}

interface IRelevanceFunction {
    function evaluate(address agent, bytes32 transitionId, bytes32 observationId)
        external view returns (uint256 relevance);
}

interface IPlanCompatibilityEngine {
    function evaluate(bytes32 transitionId, bytes32[] calldata planIds)
        external view returns (
            bool compatible,
            bytes32[] memory conflictingPlans,
            bytes32[] memory conflictingResources,
            bytes32[] memory unresolvedDependencies,
            uint256 coverage
        );
}

interface IAttributionRule {
    function evaluate(bytes32 transitionId, bytes32 outcomeId)
        external view returns (
            address[] memory agents,
            int256[] memory deltaRep,
            bytes32[] memory modelProposalIds
        );
}

/// The locked holding defined in the treasury note ("the locked position").
/// `lockedUntil` is the earliest time the holding can leave the lock. The
/// kernel only ever asks whether a holding EXISTS and stays locked through a
/// whole epoch; it never reads or stores the amount, so position cannot become
/// a weight by accident.
interface ILockedHolding {
    function lockedHolding(address agent) external view returns (uint256 amount, uint256 lockedUntil);
}
