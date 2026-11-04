// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./IEvaluators.sol";
import "./ITransferVault.sol";

interface IKernelTransfers {
    function transferOf(bytes32 id) external view returns (address token, address to, uint256 amount);
    function transferVault() external view returns (address);
}

/// The mandatory constraint (C) of a transfer transition. It judges the exact payment before any vote:
/// an unallowed token, a bad destination, a zero amount or an amount over what the vault will release this
/// epoch FAILs the predicate. No payment recorded, or no vault yet, is UNAVAILABLE, and UNAVAILABLE is never PASS.
/// The vault enforces the same limits again at payment time; this makes a bad proposal fail early and visibly.
contract TransferConstraint is IConstraint {
    IKernelTransfers public immutable kernel;
    constructor(address k) { kernel = IKernelTransfers(k); }

    function validate(bytes32, bytes32 transitionId, bytes32) external view returns (bool pass, uint8 status) {
        address v = kernel.transferVault();
        (address token, address to, uint256 amount) = kernel.transferOf(transitionId);
        if (v == address(0) || token == address(0)) return (false, 0);
        if (amount == 0 || to == address(0) || to == v || to == address(kernel)) return (false, 2);
        ITransferVault vault = ITransferVault(v);
        if (!vault.allowed(token) || amount > vault.epochRemaining(token)) return (false, 2);
        return (true, 1);
    }
}
