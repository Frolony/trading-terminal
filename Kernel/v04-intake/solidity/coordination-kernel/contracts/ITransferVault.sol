// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// What the kernel needs from the treasury vault. The vault, not the kernel, touches tokens.
interface ITransferVault {
    function kernel() external view returns (address);
    /// Pays exactly `amount` of `token` to `to`, or reverts. The caller is the bound kernel and nobody else.
    function release(address token, address to, uint256 amount) external;
    function allow(address token, bool ok) external;
    function setEpochCap(address token, uint256 cap) external;
    function allowed(address token) external view returns (bool);
    function epochRemaining(address token) external view returns (uint256);
}
