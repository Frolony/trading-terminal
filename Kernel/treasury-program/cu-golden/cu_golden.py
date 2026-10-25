#!/usr/bin/env python3
"""CU golden-test harness, slice 1: genesis/admin instruction chain.

Runs the treasury program's genesis/admin instructions against a local
test validator (started by run-cu-golden.sh) over pure JSON-RPC -- no
websockets, following the W3.3 pure-RPC lesson -- and records the
measured compute units per instruction into cu-golden.json.

Slice 1 covers only instructions that need no kernel transition:
initialize_config, register_agent, grant_role, set_capacity,
grant_capability. Value-moving and transition-bound instructions
(commit, settle, pay_tranche, ...) join in later slices, once their
setup chains (policy, observation, plan, reconcile) are scripted; the
design §2.2 >2x-miss gate applies to those estimates when they land.
Genesis/admin instructions have no §2.2 estimate, so slice 1 values
are recorded as baseline, NOT gated.

Instruction data is Anchor-shaped: 8-byte discriminator
sha256("global:<name>")[:8] + borsh args. Account order and
writable/signer flags are read from the program IDL (target/idl/),
PDA seeds from the program source (config=["config"],
chain=["chain"], agent=["agent", agent], stock=["stock", u16 LE]).

Local validator only. No devnet, no mainnet, no real funds: the
authority is the validator's own genesis faucet keypair (read from the
ledger dir), because the sandbox faucet's requestAirdrop path fails
with TransactionDataTooLarge -- see the run log. The faucet keypair is
a throwaway local-test key, never a real secret.
"""

import base64
import hashlib
import json
import os
import struct
import sys
import time
import urllib.request

from solders.instruction import AccountMeta, Instruction
from solders.keypair import Keypair
from solders.pubkey import Pubkey
from solders.transaction import Transaction

RPC = os.environ.get("CU_GOLDEN_RPC", "http://127.0.0.1:8899")
PROGRAM_ID = "FPGSeLBWEiFKdTuqDbJeKhjXeah5H4c3rqsW4Tzn3RAS"
SYSTEM_PROGRAM = "11111111111111111111111111111111"


def rpc(method, params):
    body = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
    ).encode()
    req = urllib.request.Request(
        RPC, data=body, headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req, timeout=30) as resp:
        out = json.load(resp)
    if "error" in out:
        raise RuntimeError(f"{method}: {out['error']}")
    return out["result"]


def disc(name):
    return hashlib.sha256(f"global:{name}".encode()).digest()[:8]


def u16(v):
    return struct.pack("<H", v)


def u64(v):
    return struct.pack("<Q", v)


def vec_u16(items):
    return struct.pack("<I", len(items)) + b"".join(u16(x) for x in items)


def wait_confirmed(sig, timeout=60):
    deadline = time.time() + timeout
    while time.time() < deadline:
        res = rpc("getSignatureStatuses", [[sig], {"searchTransactionHistory": False}])
        st = res["value"][0]
        if st is not None and st.get("confirmationStatus") in ("confirmed", "finalized"):
            if st.get("err") is not None:
                raise RuntimeError(f"tx {sig} failed: {st['err']}")
            return
        time.sleep(0.5)
    raise RuntimeError(f"tx {sig} not confirmed in {timeout}s")


def send_ix(program, idl_accounts, metas, data, payer_kp):
    """Send one instruction, return (signature, compute_units, fee)."""
    accounts = [
        AccountMeta(pubkey=pub, is_signer=sig, is_writable=w)
        for (pub, sig, w) in metas
    ]
    blockhash = rpc("getLatestBlockhash", [{"commitment": "confirmed"}])["value"][
        "blockhash"
    ]
    from solders.hash import Hash

    tx = Transaction.new_signed_with_payer(
        [Instruction(program, data, accounts)],
        payer_kp.pubkey(),
        [payer_kp],
        Hash.from_string(blockhash),
    )
    # Diagnostic (kept): the preflight once reported BlockhashNotFound
    # for a hash that validated standalone; print both sides of it.
    valid = rpc(
        "isBlockhashValid", [blockhash, {"commitment": "confirmed"}]
    )["value"]
    print(
        f"blockhash fetched={blockhash} valid={valid} "
        f"tx_carries={tx.message.recent_blockhash}"
    )
    # skipPreflight: a just-started local validator's preflight simulation
    # mis-reports BlockhashNotFound for a hash that isBlockhashValid
    # confirms and that the transaction itself carries (diagnosed
    # 2026-10-25). The landed meta check below is the ground truth:
    # a failed instruction still lands with meta.err set and raises.
    sig = rpc(
        "sendTransaction",
        [
            base64.b64encode(bytes(tx)).decode(),
            {"encoding": "base64", "skipPreflight": True},
        ],
    )
    wait_confirmed(sig)
    txinfo = rpc(
        "getTransaction",
        [sig, {"encoding": "json", "maxSupportedTransactionVersion": 0}],
    )
    meta = txinfo["meta"]
    if meta.get("err") is not None:
        raise RuntimeError(f"tx {sig} landed with err: {meta['err']}")
    return sig, meta["computeUnitsConsumed"], meta["fee"]


def main():
    idl_path, so_sha256, out_path, faucet_key_path = (
        sys.argv[1],
        sys.argv[2],
        sys.argv[3],
        sys.argv[4],
    )
    with open(idl_path) as f:
        idl = json.load(f)
    idl_by_name = {i["name"]: i for i in idl["instructions"]}
    program = Pubkey.from_string(PROGRAM_ID)
    system = Pubkey.from_string(SYSTEM_PROGRAM)

    with open(faucet_key_path) as f:
        authority = Keypair.from_bytes(bytes(json.load(f)))
    agent = Keypair()
    print(f"authority: {authority.pubkey()}")
    print(f"agent:     {agent.pubkey()}")
    balance = rpc("getBalance", [str(authority.pubkey())])["value"]
    assert balance > 0, "faucet keypair unfunded; validator genesis failed"
    print(f"authority balance: {balance} lamports")

    config_pda, _ = Pubkey.find_program_address([b"config"], program)
    chain_pda, _ = Pubkey.find_program_address([b"chain"], program)
    agent_pda, _ = Pubkey.find_program_address(
        [b"agent", bytes(agent.pubkey())], program
    )
    stock_pda, _ = Pubkey.find_program_address([b"stock", u16(0)], program)

    def metas_for(name, by_name):
        # Order by the IDL, never by caller dict order: Anchor validates
        # account order against the instruction struct.
        out = []
        for a in idl_by_name[name]["accounts"]:
            out.append(
                (
                    by_name[a["name"]],
                    bool(a.get("signer", False)),
                    bool(a.get("writable", False)),
                )
            )
        return out

    results = []

    def run(name, by_name, data):
        sig, cu, fee = send_ix(
            program, None, metas_for(name, by_name), data, authority
        )
        print(f"{name}: sig {sig} cu {cu} fee {fee}")
        results.append(
            {"instruction": name, "signature": sig, "compute_units": cu, "fee": fee}
        )

    run(
        "initialize_config",
        {
            "config": config_pda,
            "declared_chain": chain_pda,
            "authority": authority.pubkey(),
            "system_program": system,
        },
        disc("initialize_config") + bytes(authority.pubkey()),
    )
    # State check: Config account discriminator + stored authority.
    info = rpc(
        "getAccountInfo", [str(config_pda), {"encoding": "base64", "commitment": "confirmed"}]
    )
    raw = base64.b64decode(info["value"]["data"][0])
    assert raw[:8] == hashlib.sha256(b"account:Config").digest()[:8], "Config disc mismatch"
    assert raw[8:40] == bytes(authority.pubkey()), "Config authority mismatch"
    print("config account verified (discriminator + authority)")

    run(
        "register_agent",
        {
            "config": config_pda,
            "agent_record": agent_pda,
            "authority": authority.pubkey(),
            "system_program": system,
        },
        disc("register_agent")
        + bytes(agent.pubkey())
        + vec_u16([])
        + u64(0)
        + u64(0),
    )
    run(
        "grant_role",
        {
            "config": config_pda,
            "agent_record": agent_pda,
            "authority": authority.pubkey(),
        },
        disc("grant_role") + bytes(agent.pubkey()) + bytes([1]),
    )
    run(
        "set_capacity",
        {
            "config": config_pda,
            "stock": stock_pda,
            "authority": authority.pubkey(),
            "system_program": system,
        },
        disc("set_capacity") + u16(0) + u64(100_000_000),
    )
    run(
        "grant_capability",
        {
            "config": config_pda,
            "agent_record": agent_pda,
            "authority": authority.pubkey(),
        },
        disc("grant_capability") + bytes(agent.pubkey()) + bytes([7] * 32),
    )

    golden = {
        "program": PROGRAM_ID,
        "program_so_sha256": so_sha256,
        "solana_version": rpc("getVersion", []),
        "slice": 1,
        "slice_scope": "genesis/admin instructions (no kernel transition required)",
        "design_gate": "none: no OWN_IDL_DESIGN §2.2 estimate exists for these "
        "instructions; values are baseline, not gated",
        "instructions": results,
    }
    with open(out_path, "w") as f:
        json.dump(golden, f, indent=2)
        f.write("\n")
    print(f"golden written: {out_path}")


if __name__ == "__main__":
    main()
