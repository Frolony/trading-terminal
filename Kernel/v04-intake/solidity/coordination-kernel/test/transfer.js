// The treasury transfer kernel: a transfer transition pays from the vault in the SAME transaction as its commit,
// and its settlement is derived from the measured payment. Run: node test/transfer.js (after node test/build.js).
const ganache = require('ganache'), { ethers } = require('ethers'), art = require('./art.json');
const H = s => ethers.id(s), b32 = s => ethers.encodeBytes32String(s), WAD = 10n ** 18n, DAY = 86400, U = 10n ** 6n; // USDC has 6 decimals
let pass = 0, fail = 0;
const ok = (c, m) => { if (c) { pass++; console.log('  ok  ', m); } else { fail++; console.log('  FAIL', m); } };
const sec = t => console.log('\n' + t);

(async () => {
  const prov = new ethers.BrowserProvider(ganache.provider({ logging: { quiet: true }, wallet: { totalAccounts: 12 } }));
  const S = []; for (let i = 0; i < 12; i++) S.push(await prov.getSigner(i));
  const [admin, observer, a1, a2, a3, a4, a5, a6, a7, rando, payee, stranger] = S;
  const dep = async (n, signer, ...a) => { const f = new ethers.ContractFactory(art[n].abi, art[n].bin, signer); const c = await f.deploy(...a); await c.waitForDeployment(); return c; };
  const tx = async p => (await p).wait();
  const mine = async s => { await prov.send('evm_increaseTime', [s]); await prov.send('evm_mine', []); };
  const now = async () => (await prov.getBlock('latest')).timestamp;
  const rn = async (c, fn, ...a) => { try { await c[fn].staticCall(...a); return null; } catch (e) { return e.revert?.name || (e.data ? 'raw:' + e.data : (e.shortMessage || String(e).slice(0, 90))); } };

  // ---------------------------------------------------------------- deploy and configure (genesis)
  sec('setup: kernel, vault, constraint, a 6-decimal USDC mock');
  const mods = {};
  for (const n of ['GovernanceModule', 'OutcomeModule', 'ConsensusModule', 'PlanningModule', 'DialectModule', 'CouncilModule']) mods[n] = await dep(n, admin);
  const k0 = await dep('CoordinationKernel', admin, admin.address, b32('genesis'), H('gh'), 0, DAY, 3, {
    governance: await mods.GovernanceModule.getAddress(), outcome: await mods.OutcomeModule.getAddress(), consensus: await mods.ConsensusModule.getAddress(),
    planning: await mods.PlanningModule.getAddress(), dialect: await mods.DialectModule.getAddress(), council: await mods.CouncilModule.getAddress() }, true);
  const seen = new Set(), abi = [];
  for (const n of ['CoordinationKernel', 'GovernanceModule', 'OutcomeModule', 'ConsensusModule', 'PlanningModule', 'DialectModule', 'CouncilModule', 'TransferVault'])
    for (const e of art[n].abi) { if (e.type === 'constructor') continue; const key = e.type + e.name + JSON.stringify((e.inputs || []).map(i => i.type)); if (!seen.has(key)) { seen.add(key); abi.push(e); } }
  const iface = new ethers.Interface(abi);
  const K = s => new ethers.Contract(k0.target, abi, s), kA = K(admin);
  const vault = await dep('TransferVault', admin), V = s => new ethers.Contract(vault.target, art.TransferVault.abi, s);
  const usdc = await dep('MockUSDC6', admin), T = s => new ethers.Contract(usdc.target, art.MockUSDC6.abi, s);
  const constraint = await dep('TransferConstraint', admin, k0.target);
  const size = (await prov.getCode(k0.target)).length / 2 - 1;
  ok(size <= 24576, `the kernel still fits EIP-170 (${size} bytes)`);

  const TR = H('TRANSFER'), GA = H('GOV_ADJUSTABLE'), GC = H('GOV_CONSTITUTION'), GCO = H('GOV_COUNCIL'), GR = H('GOV_REBOOT'), G = b32('G');
  const pol = (o = {}) => ({ exists: false, quorumRequired: WAD, confidenceThreshold: 6n * WAD / 10n, coverageMin: WAD, minObsConfidence: 0n,
    consensusWindow: 1000n, planningWindow: 100n, executionWindow: 100000n, maxPlans: 4n, requireObjectives: false, settlementThreshold: 2n, maxConsensusRounds: 2n, ...o });
  for (const t of [TR, GA, GC, GCO, GR]) await tx(kA.setPolicy(t, pol()));
  const agents = [a1, a2, a3, a4, a5, a6, a7];
  for (const a of agents) await tx(kA.registerAgent(a.address, [G], WAD));
  await tx(kA.grantRole(H('OBSERVER'), observer.address));
  const eng = await dep('MockEngine', admin); await tx(kA.registerEngine(eng.target));
  await tx(kA.registerImpl(0, b32('C_transfer'), constraint.target));
  await tx(kA.setRequiredSets(TR, [b32('C_transfer')], [], [], []));
  await tx(V(admin).bind(k0.target));
  ok(await rn(V(admin), 'bind', k0.target) === 'AlreadyBound', 'the vault binds to a kernel exactly once');
  ok(await rn(kA, 'setTransferVault', stranger.address) === 'BadTransferVault', 'a non-contract cannot be the vault');
  await tx(kA.setTransferVault(vault.target));
  ok((await kA.transferVault()) === vault.target, 'the kernel pays from the bound vault');
  await tx(kA.vaultAllow(usdc.target, true));
  await tx(kA.vaultSetEpochCap(usdc.target, 1000n * U));
  await tx(T(admin).mint(vault.target, 5000n * U));
  await tx(kA.installCouncil([a1.address, a2.address, a3.address]));
  await tx(kA.sealGenesis());
  const t0 = await now();
  await tx(K(observer).ingestObservation(b32('obs1'), t0, H('s'), H('d'), H('m'), WAD, t0 + 1_000_000_000));

  // ---------------------------------------------------------------- helpers
  const FAR = () => now().then(n => n + 100_000_000);
  const parent = async () => (await kA.declaredState()).id;
  const lc = async id => Number(await kA.transitionLifecycle(id));
  const bal = async a => await usdc.balanceOf(a);
  const mfT = async (id, token, to, amount, o = {}) => ({ parentStateId: await parent(), transitionType: o.type || TR, relevantGroups: [G], changedVars: [], constraints: [],
    resources: [], expectedEffects: [b32('E1')], expiresAt: await FAR(), observationId: b32('obs1'),
    resultingStateHash: o.state || await kA.transferHash(id, token, to, amount), transitionPayloadHash: o.payload || ethers.keccak256(ethers.AbiCoder.defaultAbiCoder().encode(['address', 'address', 'uint256'], [token, to, amount])) });
  const propose = async (name, to, amount, o = {}) => {
    const id = b32(name), token = o.token || usdc.target;
    await tx(K(o.proposer || a4).registerTransferTransition(id, b32(name + 'S'), await mfT(id, token, to, amount, o), token, to, amount));
    return id;
  };
  const drive = async (id, supporters = [a1, a2, a3]) => {
    await tx(K(rando).evaluateTransition(id));
    if ((await lc(id)) === 14) return;                    // REJECTED by a failing constraint
    await tx(K(rando).markEligible(id));
    for (const s of supporters) await tx(K(s).submitRoundSignal(id, WAD, WAD));
    await mine(1001);
    await tx(K(rando).selectCandidate(id)); await tx(K(rando).openPlanning(id));
    await tx(K(a1).submitPlan(id, { resourceClaims: [], effects: [b32('E1')], prerequisites: [], conflicts: [], expiresAt: await FAR(), payloadHash: H('plan') }));
    await mine(101);
    await tx(K(rando).reconcilePlans(id)); await tx(K(rando).validateTransition(id)); await tx(K(rando).declareCommit(id));
  };

  // ---------------------------------------------------------------- registration guards
  sec('a transfer cannot be registered or committed without being the exact payment');
  const idG = b32('guard'), to = payee.address, tok = usdc.target;
  const base = await mfT(idG, tok, to, 10n * U);
  ok(await rn(K(a4), 'registerTransition', idG, b32('guardS'), base) === 'TransferTypeOnly', 'the transfer type cannot enter as an ordinary transition (no payment attached)');
  ok(await rn(K(a4), 'registerTransferTransition', idG, b32('guardS'), await mfT(idG, tok, to, 10n * U, { type: H('OTHER') }), tok, to, 10n * U) === 'NotTransferTransition', 'registerTransferTransition takes only the transfer type');
  ok(await rn(K(a4), 'registerTransferTransition', idG, b32('guardS'), base, tok, to, 11n * U) === 'TransferActionMismatch', 'the payload hash binds token, destination and amount');
  ok(await rn(K(a4), 'registerTransferTransition', idG, b32('guardS'), await mfT(idG, tok, to, 10n * U, { state: H('x') }), tok, to, 10n * U) === 'TransferStateMismatch', 'the resulting state hash must be the derived transfer hash, so settlement is exact');
  ok(await rn(K(stranger), 'registerTransferTransition', idG, b32('guardS'), base, tok, to, 10n * U) === 'InactiveAgent', 'only an active agent proposes');
  ok(await rn(V(stranger), 'release', tok, to, 1n) === 'NotKernel', 'nobody but the kernel can make the vault pay');
  ok(await rn(V(admin), 'release', tok, to, 1n) === 'NotKernel', 'not even the deployer');
  ok(await rn(kA, 'vaultSetEpochCap', tok, 1n) === 'Unauthorized', 'after the seal the cap changes only through a governance transition');

  // ---------------------------------------------------------------- the constraint judges the payment before any vote
  sec('constraint: a bad payment fails the predicate before anyone votes');
  const bad = [['badZero', ethers.ZeroAddress, 10n * U, {}], ['badVault', vault.target, 10n * U, {}], ['badAmt', to, 0n, {}], ['badCap', to, 1001n * U, {}], ['badTok', to, 10n * U, { token: stranger.address }]];
  for (const [n, t, a, o] of bad) {
    const id = await propose(n, t, a, o); await tx(K(rando).evaluateTransition(id));
    ok((await lc(id)) === 14, `${n}: a zero destination, the vault itself, a zero amount, an amount over the epoch cap, or an unlisted token is REJECTED`);
  }

  // ---------------------------------------------------------------- happy path
  sec('commitTransfer: the payment and the commit are one atomic step');
  const vault0 = await bal(vault.target), payee0 = await bal(to), idx0 = (await kA.declaredState()).index;
  const p1 = await propose('pay1', to, 100n * U); await drive(p1);
  ok((await lc(p1)) === 8, 'COMMITTABLE: the predicate mask is zero and nothing has been paid yet');
  ok((await bal(to)) === payee0 && (await bal(vault.target)) === vault0, 'declaring the commit moves no funds');
  ok(await rn(K(rando), 'commit', p1) === 'UseTransferCommit', 'commit() refuses a transfer: a transfer cannot commit without paying');
  await tx(K(rando).commitTransfer(p1));
  ok((await lc(p1)) === 11, 'SETTLED in the same transaction as the commit');
  ok((await bal(to)) === payee0 + 100n * U && (await bal(vault.target)) === vault0 - 100n * U, 'the vault was debited and the payee credited by exactly 100 USDC (6-decimal units)');
  const tr = await kA.getTransition(p1);
  const real = await kA.realizedStates(p1);
  ok(real.stateHash === tr.manifest.resultingStateHash && real.stateHash === await kA.transferHash(p1, tok, to, 100n * U), 'the realized hash is derived from the payment and equals the declared one: exact, no drift');
  ok((await kA.declaredState()).index === idx0 + 1n, 'the declared state advanced');
  ok(await rn(K(rando), 'commitTransfer', p1) === 'BadLifecycle', 'a paid transfer cannot be paid again');
  ok(await rn(K(rando), 'attestSettlement', p1, b32('x'), H('x')) !== null, 'settlement is not attested by anyone: there is nothing left to assert');
  ok((await V(rando).epochRemaining(tok)) === 900n * U, 'the vault counted 100 against this epoch\'s 1000 cap');

  // ---------------------------------------------------------------- nothing commits when the payment cannot be made
  sec('atomicity: if the payment fails, nothing commits');
  const idx1 = (await kA.declaredState()).index;
  await tx(T(admin).setBlocked(to, true));
  const p2 = await propose('pay2', to, 50n * U); await drive(p2);
  ok(await rn(K(rando), 'commitTransfer', p2) === 'TransferFailed', 'a blocklisted recipient makes the payment revert');
  ok((await lc(p2)) === 8 && (await kA.declaredState()).index === idx1, 'and the transfer is still COMMITTABLE: nothing committed, the state did not move');
  await tx(T(admin).setBlocked(to, false));
  await tx(K(rando).commitTransfer(p2));
  ok((await lc(p2)) === 11, 'once the block is lifted the same transfer commits and pays');

  await tx(T(admin).setFee(100));
  const p3 = await propose('pay3', to, 50n * U); await drive(p3);
  const r = await rn(K(rando), 'commitTransfer', p3);
  ok(r === 'PaymentInexact', 'a fee-on-transfer token (short credit) reverts: settlement is never inexact');
  ok((await lc(p3)) === 8, 'and nothing committed');
  await tx(T(admin).setFee(0));
  await tx(K(rando).commitTransfer(p3));
  ok((await lc(p3)) === 11, 'exact payment commits');

  // ---------------------------------------------------------------- the hard cap, and governing it
  sec('the epoch cap is enforced by the constraint and again by the vault; the council changes it only by a governed transition');
  ok((await V(rando).epochRemaining(tok)) === 800n * U, 'three payments have used 200 of the 1000 cap');
  const pBig = await propose('payBig', to, 801n * U); await tx(K(rando).evaluateTransition(pBig));
  ok((await lc(pBig)) === 14, 'a payment over what is left is REJECTED by the constraint');
  const seats = [a1, a2, a3];
  const cap10 = iface.encodeFunctionData('vaultSetEpochCap', [tok, 10n * U]);
  const gid = b32('gcap'), ah = ethers.keccak256(cap10);
  ok(await rn(K(a1), 'registerGovernanceTransition', gid, b32('gcapS'), { ...(await mfT(gid, tok, to, 1n, { type: GA, payload: ah, state: H('g1') })) }, cap10) === 'CouncilNotUnanimous', 'lowering the cap needs every seat');
  for (const s of seats) await tx(K(s).approveGovernance(gid, ah));
  await tx(K(a1).registerGovernanceTransition(gid, b32('gcapS'), await mfT(gid, tok, to, 1n, { type: GA, payload: ah, state: H('g1') }), cap10));
  await drive(gid, [a1, a2, a3, a4]);
  await tx(K(rando).commitGovernance(gid));
  ok((await vault.epochCap(tok)) === 10n * U, 'the governed transition lowered the vault cap to 10 USDC');
  const pLow = await propose('payLow', to, 50n * U); await tx(K(rando).evaluateTransition(pLow));
  ok((await lc(pLow)) === 14, 'a payment that was fine a moment ago is now REJECTED');

  console.log(`\n${pass} passed, ${fail} failed`); process.exit(fail ? 1 : 0);
})().catch(e => { console.error(e); process.exit(2); });
