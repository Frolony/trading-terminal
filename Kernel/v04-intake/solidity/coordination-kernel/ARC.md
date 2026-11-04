# The kernel on Arc: a kernel that transfers

Status: **built and tested on a local EVM only.** Nothing here has run on an Arc network. See "What local tests cannot show".

## What was checked in the Arc docs (docs.arc.io, read 2026-11-03)

* The docs list **Mainnet (chain 5042) and Testnet (chain 5042002)**. They do not document a devnet. Testnet RPC: `https://rpc.testnet.arc.io`; faucet: `https://faucet.circle.com`; explorer: `https://explorer.testnet.arc.io`.
* **USDC is the gas token and has two interfaces over one balance**: native (18 decimals) and an ERC-20 at `0x3600000000000000000000000000000000000000` (6 decimals). Do not mix the two reads.
* A native send to the zero address reverts, native transfers to or from a blocklisted address revert, and the ERC-20 view drops amounts below 1e-6 USDC.
* Finality is deterministic once a transaction is in a committed block; `block.prevrandao` is 0; timestamps can repeat, so index by block number and log index.
* Solidity, Foundry, Hardhat, ethers and viem work as on Ethereum. Standard Anvil does not reproduce Arc's precompiles, blocklist or EIP-7708 logs; Arc ships `arc-forge`, `arc-cast` and `arc-anvil` for that.

## What was built

A transfer is an ordinary transition type, `TRANSFER`, whose commit **is** the payment. The shape follows governance transitions, which already apply an action in the same transaction as their commit.

| piece | job |
|---|---|
| `registerTransferTransition` | stores the exact payment (token, to, amount) at registration; the payload hash binds it and the resulting-state hash must be the derived transfer hash |
| `TransferConstraint` (the mandatory C) | judges the payment before any vote: allowed token, real destination, non-zero amount, within the vault's epoch cap. No payment or no vault is UNAVAILABLE, never PASS |
| the 13-bit predicate | unchanged; a transfer must reach `commitFailMask == 0` like everything else |
| `commitTransfer` | commits through the same `_commitCore`, then asks the vault to pay, then settles. If the payment fails everything reverts and nothing commits |
| `commit()` | refuses a transfer (`UseTransferCommit`), so a transfer cannot commit unpaid |
| `TransferVault` | the only thing that touches tokens. Bound once to the kernel; pays only on the kernel's call; refuses the zero address, itself and the kernel; enforces a per-epoch cap per token; verifies the debit and the credit to the unit by `balanceOf` |
| settlement | derived, not attested: the realized hash is computed from the measured payment and equals the declared hash, so there is no drift and no settler can assert a payment that did not happen |
| governance | `setTransferVault` (constitutional), `vaultAllow` and `vaultSetEpochCap` (adjustable) change only through governance transitions with unanimous council approval |

Arc-specific choices in the vault: ERC-20 interface only (no `msg.value`, no `receive`, no `.balance`), so there is one unit system and no decimal conversion in the payment path; exact-balance checks so a blocklisted recipient or a fee-on-transfer token fails loudly.

Sizes: `CoordinationKernel` 23,877 bytes (limit 24,576), `GovernanceModule` 18,488, `TransferVault` 2,987, `TransferConstraint` 1,208.

## Tests

`node test/transfer.js` (after `node test/build.js`): 38 checks. Registration guards; the constraint rejecting a zero destination, the vault itself, a zero amount, an amount over the cap and an unlisted token; commit and payment as one step with exact balances in 6-decimal units; a blocklisted recipient and a fee-on-transfer token each reverting with the transfer left COMMITTABLE and the declared state unmoved; the cap enforced and then lowered by a governed transition.

## What local tests cannot show

Arc documents behaviour a local EVM does not reproduce: the ERC-20 interface being a precompile over the native balance, blocklist enforcement, and its transfer events. The mock USDC here models the blocklist and fee failure modes but is not Arc. **Run `transfer.js`'s scenarios against Arc Testnet (chain 5042002) with `arc-forge` or a funded testnet account before relying on any of this.** Also untested: reentrant tokens beyond the vault's lock, and the capacity side (the K organ) for treasury balances, which uses the mock engine here.

## Not decided here

* The Rust crate has no transfer. The Rust kernel moves no assets; a deployment performs the payment through `Env`. Only the predicate parity is pinned (`the_transfer_extension_goes_through_the_same_commit_core`).
* Which tokens and caps to allow at genesis, who the proposers are, and whether payments need a compatibility engine that reads the vault's balance (so over-commitment across concurrent transfers fails COMPAT before commit).
