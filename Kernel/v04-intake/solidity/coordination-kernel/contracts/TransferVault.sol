// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./ITransferVault.sol";

interface IERC20Min {
    function balanceOf(address a) external view returns (uint256);
    function transfer(address to, uint256 amount) external returns (bool);
}

interface IKernelEpoch {
    function currentEpoch() external view returns (uint256);
}

/// Holds the treasury's tokens and pays only when the kernel says a transfer transition committed.
///
/// Built for Arc, where USDC is both the gas token (18 decimals, native balance) and an ERC-20 at
/// 0x3600000000000000000000000000000000000000 (6 decimals) over the SAME balance. The vault therefore:
///   * uses only the ERC-20 interface (never `msg.value`, never `receive`, never `.balance`), so there is one
///     unit system and no decimal conversion anywhere in the payment path;
///   * refuses the zero address and itself as a destination (a native send to 0x0 reverts on Arc);
///   * verifies the debit and the credit by `balanceOf` to the unit, so a blocklisted recipient (the transfer
///     reverts), a fee-on-transfer token, or any short payment reverts the whole commit instead of settling
///     inexactly;
///   * holds a hard per-epoch cap per token that is independent of the kernel's constraint evaluators.
contract TransferVault {
    address public immutable deployer;
    address public kernel;

    mapping(address => bool) public allowed;
    mapping(address => uint256) public epochCap;                         // 0 = nothing may leave
    mapping(address => mapping(uint256 => uint256)) public spent;        // token => epoch => amount

    bool private _lock;

    error NotKernel();
    error AlreadyBound();
    error NotDeployer();
    error TokenNotAllowed(address token);
    error BadDestination(address to);
    error ZeroAmount();
    error OverEpochCap(uint256 wanted, uint256 remaining);
    error PaymentInexact(uint256 debited, uint256 credited, uint256 wanted);
    error TransferFailed();
    error Reentered();

    event Bound(address kernel);
    event Released(address indexed token, address indexed to, uint256 amount, uint256 epoch);
    event Allowed(address indexed token, bool ok);
    event EpochCapSet(address indexed token, uint256 cap);

    constructor() { deployer = msg.sender; }

    /// Once: ties the vault to the kernel it serves. The deployer cannot pay anyone, change a cap or rebind.
    function bind(address k) external {
        if (msg.sender != deployer) revert NotDeployer();
        if (kernel != address(0)) revert AlreadyBound();
        kernel = k;
        emit Bound(k);
    }

    modifier onlyKernel() {
        if (msg.sender != kernel) revert NotKernel();
        _;
    }

    function allow(address token, bool ok) external onlyKernel { allowed[token] = ok; emit Allowed(token, ok); }

    function setEpochCap(address token, uint256 cap) external onlyKernel { epochCap[token] = cap; emit EpochCapSet(token, cap); }

    function epochRemaining(address token) public view returns (uint256) {
        uint256 cap = epochCap[token];
        uint256 used = spent[token][IKernelEpoch(kernel).currentEpoch()];
        return used >= cap ? 0 : cap - used;
    }

    function release(address token, address to, uint256 amount) external onlyKernel {
        if (_lock) revert Reentered();
        _lock = true;
        if (!allowed[token]) revert TokenNotAllowed(token);
        if (to == address(0) || to == address(this) || to == kernel) revert BadDestination(to);
        if (amount == 0) revert ZeroAmount();
        uint256 e = IKernelEpoch(kernel).currentEpoch();
        uint256 left = epochRemaining(token);
        if (amount > left) revert OverEpochCap(amount, left);
        spent[token][e] += amount;

        uint256 vaultBefore = IERC20Min(token).balanceOf(address(this));
        uint256 toBefore = IERC20Min(token).balanceOf(to);
        (bool ok, bytes memory ret) = token.call(abi.encodeCall(IERC20Min.transfer, (to, amount)));
        if (!ok || (ret.length != 0 && !abi.decode(ret, (bool)))) revert TransferFailed();
        uint256 debited = vaultBefore - IERC20Min(token).balanceOf(address(this));
        uint256 credited = IERC20Min(token).balanceOf(to) - toBefore;
        if (debited != amount || credited != amount) revert PaymentInexact(debited, credited, amount);

        emit Released(token, to, amount, e);
        _lock = false;
    }
}
