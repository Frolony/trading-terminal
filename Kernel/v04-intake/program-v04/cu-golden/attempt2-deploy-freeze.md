# Attempt-2 DEPLOY freeze — id binding (2026-11-03, Phase 3a)

Status: current — deploy binary frozen for the attempt-2 NEW-program devnet deploy. The attempt-2 candidate freeze (70f8fdb1…) remains the golden baseline; this record freezes the deploy variant that differs from it ONLY by the embedded declared program id.

## New program id

**`CesWhN1MyhEqYT2wu6m7NgPz4SigBqz7oz2BGrKRHNdj`** — fresh keypair `deploy-keys/attempt2/program-v04-attempt2-keypair.json` (0600, dir 0700), generated 2026-11-03 ~18:14 UTC; confirmed undeployed on devnet before binding. Attempt 1 (`2CQ58pLCb5AsaKxLudcbuiPHGphtyv9cKtDKK5mdxBvr`) is the frozen monument and is NOT this program.

## Deploy binary

- `target/deploy/program_v04.so`: **1,924,664 bytes** (identical size to the candidate), sha256 **`f501467d06f15885d395e7b9f7daf2982a9d48b0c1f337e619eec0812ac2666d`**.
- Relation to the frozen candidate (`70f8fdb1bcfd52e8cd9edd0e204683f8c06cdc1db2502e3ff27e1fee9cc527b7`, same size): **identical except the embedded declared program id** — `declare_id!` in `programs/program-v04/src/lib.rs` changed from attempt-1's id to the new id above, and `Anchor.toml [programs.devnet]` updated to match (staging-local documentation). No other source change.

## Id-binding proof (Phase 3a step 3)

- (a) Host tests: **465 passed, 0 failed** on the deploy tree — identical to the candidate.
- (b) IDL: **45 instructions / 10 accounts / 51 errors / 27 events / 40 types** — counts unchanged; only the `address` field changed to the new program id.
- (c) Mollusk golden against the deploy binary: with the harness's `PROGRAM_ID` const at the OLD id the binary refuses every instruction with `Custom(4100)` (DeclaredProgramIdMismatch) — the binding is enforced in-program. With the harness const set to the NEW id (local verification adaptation only, not part of this push), **all 63 measured verdicts and both refusals reproduce exactly** (63 Success + init_reputation_again Custom(0) + close_gov_action_finalized 6034; blocked []). Literal byte-identical reproduction of `cu-golden-mollusk-attempt2.json` (aa8820bb…) is impossible for ANY id change by construction: the JSON embeds the tested binary's sha256, and PDA bump-search CU depends on the program id (the 2026-10-30 re-freeze precedent). Measured CU: 45 of 63 steps identical; the 18 moved steps each move by an exact multiple of 1,500 CU (one PDA bump-search iteration) — the pure program-id derivation effect, no semantic delta.
- (d) Enroll scenario against the deploy binary (harness const at the new id): **all verdicts reproduce** — non-authority enroll refused 6047, enroll OBSERVER 6,514 / ag(1) 6,505 Success, occupied-slot re-enroll refused 6049, unenrolled ingest refused 6009, enrolled OBSERVER ingest Success, enrolled ag(1) register + replay Success.

## Deploy

Follows the attempt-1 chunked-writer procedure (fresh buffer, paced census 2,048 B slices + 0.4 s, standalone completeness gate before finalize, fresh deploy with the attempt-2 program keypair). Full record: `hidden_files/w44-enrollment-slice-2026-11-03.md`, Phase 3/4 section.
