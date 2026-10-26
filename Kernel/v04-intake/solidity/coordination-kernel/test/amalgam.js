const ganache = require('ganache'), { ethers } = require('ethers'), art = require('../art.json');
const H = s => ethers.id(s), b32 = s => ethers.encodeBytes32String(s), Z = ethers.ZeroHash, WAD = 10n ** 18n;
const UNC = (1n << 256n) - 1n;
let pass = 0, fail = 0;
const ok = (c, m) => { if (c) { pass++; console.log('  ok  ', m); } else { fail++; console.log('  FAIL', m); } };
const sec = t => console.log('\n' + t);

(async () => {
  const prov = new ethers.BrowserProvider(ganache.provider({ logging: { quiet: true }, chain: { allowUnlimitedContractSize: false }, wallet: { totalAccounts: 16 } }));
  const S = []; for (let i = 0; i < 16; i++) S.push(await prov.getSigner(i));
  const [admin, observer, a1, a2, a3, a4, a5, a6, a7, executor, settler, recorder, guardian, stranger, rando, x15] = S;
  const dep = async (n, signer, ...a) => { const f = new ethers.ContractFactory(art[n].abi, art[n].bin, signer); const c = await f.deploy(...a); await c.waitForDeployment(); return c; };
  const mods = {};
  for (const n of ['GovernanceModule', 'OutcomeModule', 'ConsensusModule', 'PlanningModule', 'DialectModule', 'CouncilModule']) mods[n] = await dep(n, admin);
  const genesis = b32('genesis'), DAY = 86400;
  const k0 = await dep('CoordinationKernel', admin, admin.address, genesis, H('gh'), 0, DAY, 3, {
    governance: await mods.GovernanceModule.getAddress(), outcome: await mods.OutcomeModule.getAddress(), consensus: await mods.ConsensusModule.getAddress(),
    planning: await mods.PlanningModule.getAddress(), dialect: await mods.DialectModule.getAddress(), council: await mods.CouncilModule.getAddress() });
  const seen = new Set(), abi = [];
  for (const n of ['CoordinationKernel', 'GovernanceModule', 'OutcomeModule', 'ConsensusModule', 'PlanningModule', 'DialectModule', 'CouncilModule'])
    for (const e of art[n].abi) { const key = e.type + e.name + JSON.stringify((e.inputs || []).map(i => i.type)); if (!seen.has(key)) { seen.add(key); abi.push(e); } }
  const iface = new ethers.Interface(abi);
  const K = s => new ethers.Contract(k0.target, abi, s);
  const kA = K(admin);
  const rn = async (c, fn, ...a) => { try { await c[fn].staticCall(...a); return null; } catch (e) { return e.revert?.name || (e.data ? 'raw:' + e.data : (e.shortMessage || String(e).slice(0, 90))); } };
  const mine = async (s = 0) => { await prov.send('evm_increaseTime', [s]); await prov.send('evm_mine', []); };
  const now = async () => (await prov.getBlock('latest')).timestamp;
  const tx = async p => (await p).wait();

  // ---------------------------------------------------------------- genesis
  sec('genesis: bootstrap authority exists only until sealed');
  const G = b32('G'), G2 = b32('G2');
  const T1 = b32('ORD'), T2 = b32('ORD2'), TX = b32('ORD3');
  const GA = H('GOV_ADJUSTABLE'), GC = H('GOV_CONSTITUTION'), GCO = H('GOV_COUNCIL'), GR = H('GOV_REBOOT');
  const pol = (o = {}) => ({ exists: false, quorumRequired: 2n * WAD, confidenceThreshold: 6n * WAD / 10n, coverageMin: WAD, minObsConfidence: 0n,
    consensusWindow: 1000n, planningWindow: 100n, executionWindow: 100000n, maxPlans: 4n, requireObjectives: false, settlementThreshold: 2n, maxConsensusRounds: 2n, ...o });
  ok(await rn(K(rando), 'grantRole', H('OBSERVER'), rando.address) === 'Unauthorized', 'a stranger cannot grant roles');
  ok(await rn(K(rando), 'setPolicy', T1, pol()) === 'Unauthorized', 'a stranger cannot set policy');
  ok(await rn(kA, 'sealGenesis') === 'GenesisNotReady', 'seal refused: no council, no governance policies yet');
  await tx(kA.setPolicy(T1, pol())); await tx(kA.setPolicy(T2, pol())); await tx(kA.setPolicy(TX, pol({ maxConsensusRounds: 1n })));
  for (const t of [GA, GC, GCO, GR]) await tx(kA.setPolicy(t, pol({ quorumRequired: 3n * WAD, confidenceThreshold: 9n * WAD / 10n })));
  ok(await rn(kA, 'setPolicy', T1, pol({ maxConsensusRounds: 0n })) === 'BadRounds', 'policy needs at least one round');
  await tx(kA.setGroupMultiplier(T2, G, 2n * WAD)); await tx(kA.setGroupMultiplier(T2, G2, 3n * WAD));
  const agentsL = [a1, a2, a3, a4, a5, a6, a7];
  for (const a of agentsL) await tx(kA.registerAgent(a.address, a === a5 ? [G, G2] : [G], WAD));
  const mkRole = async (r, s) => tx(kA.grantRole(H(r), s.address));
  await mkRole('OBSERVER', observer); await mkRole('EXECUTOR', executor); await mkRole('SETTLER', settler); await mkRole('SETTLER', rando);
  await mkRole('OUTCOME_RECORDER', recorder); await mkRole('GUARDIAN', guardian);
  const cGood = await dep('MockConstraint', admin, true), cBad = await dep('MockConstraint', admin, false);
  const eng = await dep('MockEngine', admin), att = await dep('MockAttribution', admin, a4.address), pos = await dep('MockPosition', admin);
  await tx(kA.registerEngine(eng.target));
  await tx(kA.registerImpl(0, b32('C_bad'), cBad.target)); await tx(kA.registerImpl(3, b32('A1'), att.target));
  await tx(kA.setRequiredSets(TX, [b32('C_bad')], [], [], []));
  await tx(kA.setRequiredSets(T1, [], [], [], [b32('A1')]));
  await tx(kA.setPositionSource(pos.target));
  ok(await rn(kA, 'installCouncil', [a1.address, a2.address]) === 'BadSeats', 'council must fill exactly the seat count');
  await tx(kA.installCouncil([a1.address, a2.address, a3.address]));
  ok(await kA.isSeated(a1.address), 'genesis council seated');
  ok(await rn(kA, 'sealGenesis') === null, 'seal allowed once council + governance policies exist');
  await tx(kA.sealGenesis());
  ok(await kA.genesisSealed() && (await kA.genesisAdmin()) === ethers.ZeroAddress, 'sealed: bootstrap authority erased');
  ok(await rn(kA, 'setPolicy', T1, pol()) === 'Unauthorized', 'admin can no longer set policy');
  ok(await rn(kA, 'grantRole', H('OBSERVER'), rando.address) === 'Unauthorized', 'admin can no longer grant roles');
  ok(await rn(kA, 'installCouncil', [a4.address, a5.address, a6.address]) === 'Unauthorized', 'admin can no longer install a council');
  ok(await rn(kA, 'sealGenesis') === 'NotGenesis', 'seal cannot be repeated');

  // ---------------------------------------------------------------- helpers
  const KO = K(observer);
  const t0 = await now();
  await tx(KO.ingestObservation(b32('obs1'), t0, H('s'), H('d'), H('m'), WAD, t0 + 1_000_000_000));
  const FAR = () => now().then(n => n + 100_000_000);
  const mf = async (type, parent, payload = H('p'), groups = [G], cons = []) => ({ parentStateId: parent, transitionType: type, relevantGroups: groups, changedVars: [], constraints: cons,
    resources: [], expectedEffects: [b32('E1')], expiresAt: await FAR(), observationId: b32('obs1'), resultingStateHash: H('S' + Math.random()), transitionPayloadHash: payload });
  const parent = async () => (await kA.declaredState()).id;
  const B = id => id.startsWith('0x') ? id : b32(id);
  const register = async (signer, id, type, o = {}) => tx(K(signer).registerTransition(B(id), b32(id.replace('0x', '') + 'S').slice(0, 66), await mf(type, o.parent || await parent(), H('p'), o.groups || [G], o.cons || [])));
  // evaluate -> eligible
  const toEligible = async id => { await tx(K(rando).evaluateTransition(id)); await tx(K(rando).markEligible(id)); };
  const sign = (signer, id, raw, sup) => tx(K(signer).submitRoundSignal(id, raw, sup));
  // consensus closed -> select -> plan -> reconcile -> validate -> declare
  const finish = async (id, planner) => {
    await mine(1001);
    await tx(K(rando).selectCandidate(id)); await tx(K(rando).openPlanning(id));
    await tx(K(planner).submitPlan(id, { resourceClaims: [], effects: [b32('E1')], prerequisites: [], conflicts: [], expiresAt: await FAR(), payloadHash: H('plan') }));
    await mine(101);
    await tx(K(rando).reconcilePlans(id)); await tx(K(rando).validateTransition(id)); await tx(K(rando).declareCommit(id));
  };
  const full = async (signer, id, type, supporters, sup = WAD, o = {}) => {
    const bid = B(id);
    await register(signer, id, type, o); await toEligible(bid);
    for (const s of supporters) await sign(s, bid, WAD, sup);
    await finish(bid, supporters[0]);
  };
  const lc = async id => Number(await kA.transitionLifecycle(id));
  const mask = async id => BigInt(await kA.commitFailMask(id));

  // ---------------------------------------------------------------- classifier pins
  sec('classifier: all 48 cells pinned to the uploaded tables');
  const Sx = ['ALIGNED', 'MISALIGNED', 'COLLABORATIVE', 'DIVERGENT', 'ADVERSARIAL', 'SPECIALIST', 'DISASSOCIATED', 'ABSTAINING', 'UNCERTAIN'];
  const secRows = [['COLLABORATIVE', 'ALIGNED', 'ALIGNED', 'DIVERGENT', 'ABSTAINING', 'SPECIALIST'], ['ADVERSARIAL', 'MISALIGNED', 'MISALIGNED', 'ADVERSARIAL', 'ABSTAINING', 'SPECIALIST'],
    ['DIVERGENT', 'UNCERTAIN', 'UNCERTAIN', 'DIVERGENT', 'ABSTAINING', 'SPECIALIST'], ['DIVERGENT', 'UNCERTAIN', 'UNCERTAIN', 'DISASSOCIATED', 'ABSTAINING', 'SPECIALIST']];
  const wRows = [[WAD, WAD, WAD, WAD / 2n, 0n, WAD * 3n / 2n], [WAD, WAD, WAD, WAD, 0n, WAD * 3n / 2n], [WAD / 2n, WAD / 10n, WAD / 10n, WAD / 2n, 0n, WAD * 3n / 2n], [WAD / 2n, WAD / 10n, WAD / 10n, WAD / 4n, 0n, WAD * 3n / 2n]];
  let cellsOk = true;
  for (let s = 0; s < 4; s++) for (let c = 0; c < 6; c++) {
    if (Sx[Number(await kA.sectorOf(s, c))] !== secRows[s][c]) { cellsOk = false; console.log('   sector mismatch', s, c); }
    if ((await kA.baseWeightOf(s, c)) !== wRows[s][c]) { cellsOk = false; console.log('   weight mismatch', s, c); }
  }
  ok(cellsOk, 'every Stance x Contribution cell matches (sector and weight)');
  const st = async v => Number(await kA.classifyStance(v));
  ok(await st(7n * WAD / 10n) === 0 && await st(3n * WAD / 10n) === 1 && await st(WAD / 2n) === 2 && await st(UNC) === 3, 'stance bands: >0.6 aligned, <0.4 opposed, between neutral, sentinel uncertain');
  ok(await st(6n * WAD / 10n) === 2 && await st(4n * WAD / 10n) === 2, 'band edges 0.4 and 0.6 are neutral');

  // ---------------------------------------------------------------- authority + registration
  sec('registration: policy comes only from governance; strangers and unknown types refused');
  const gp = await parent();
  ok(await rn(K(stranger), 'registerTransition', b32('xs'), b32('xsS'), await mf(T1, gp)) === 'InactiveAgent', 'a non-agent cannot propose');
  ok(await rn(K(a4), 'registerTransition', b32('xu'), b32('xuS'), await mf(b32('NOPE'), gp)) === 'NoPolicyForType', 'unknown transition type rejected (no proposer-supplied policy)');
  ok(await rn(K(a4), 'registerTransition', b32('xg'), b32('xgS'), await mf(GA, gp)) === 'GovernanceTypeOnly', 'a governance type cannot enter as an ordinary transition');
  ok(await rn(K(a4), 'registerGovernanceTransition', b32('xg'), b32('xgS'), await mf(GA, gp), '0x12345678') === 'NotCouncil', 'only a council seat may propose a rule change');
  console.log('constraint bypass: proposer omits the mandatory failing constraint');
  await register(a4, 'tBad', TX); await tx(K(rando).evaluateTransition(b32('tBad')));
  ok(await lc(b32('tBad')) === 14, 'mandatory failing constraint applied anyway -> REJECTED');

  // ---------------------------------------------------------------- rounds
  sec('rounds: one signal per agent per round, revisions move toward the mean, latest signal counts');
  const R1 = b32('R1');
  await register(a4, 'R1', T1); await toEligible(R1);
  ok(await rn(K(a1), 'submitRoundSignal', R1, WAD + 1n, WAD) === 'ValueOutOfRange', 'raw weight above WAD rejected (cap restored)');
  ok(await rn(K(a1), 'submitRoundSignal', R1, WAD, WAD + 1n) === 'ValueOutOfRange', 'support above WAD rejected');
  ok(await rn(K(stranger), 'submitRoundSignal', R1, WAD, WAD) === 'NotEligible', 'non-agent cannot signal');
  await sign(a1, R1, WAD, 9n * WAD / 10n); await sign(a2, R1, WAD, WAD / 2n);
  ok(await rn(K(a1), 'submitRoundSignal', R1, WAD, WAD) === 'AlreadySignalledThisRound', 'second signal in the same round rejected');
  let ty = await kA.tallies(R1);
  ok(ty.totalWeight === WAD && ty.count === 2n, 'two agents, 0.5 WAD each = 1 WAD (influence snapshot at eligibility)');
  await sign(a3, R1, WAD, UNC);
  ty = await kA.tallies(R1);
  ok(ty.totalWeight === WAD && ty.count === 3n, 'an UNCERTAIN signal is recorded but adds no weight');
  ok(Number(await kA.roundOf(R1)) === 0, 'round 0 initially');
  await mine(501);
  ok(Number(await kA.roundOf(R1)) === 1, 'round derived from time: round 1 after half the window');
  ok(await rn(K(a1), 'submitRoundSignal', R1, WAD, 95n * WAD / 100n) === 'MonotoneRevisionViolation', 'revision away from the mean refused');
  await sign(a1, R1, WAD, 8n * WAD / 10n);
  ty = await kA.tallies(R1);
  ok(ty.totalWeight === WAD && ty.count === 3n && ty.weightedSupport === (WAD / 2n * 8n / 10n) + (WAD / 2n * 5n / 10n), 'revision replaces the earlier signal in the tally (no double count)');
  console.log('   (R1 finishes below quorum on purpose)');
  ok((await mask(R1) & 4n) === 4n, 'quorum bit set while weight is below quorum');

  // ---------------------------------------------------------------- niche
  sec('niche: the weight table builds each agent\'s specialty');
  let nz = await kA.nicheOf(a1.address);
  ok(nz.found && Sx[Number(nz.sector)] === 'COLLABORATIVE', 'a1 (aligned, substantive) is niched COLLABORATIVE');
  ok(nz.score === 2n * WAD, 'its score is the table weight 1.0 added once per classified signal (two rounds)');
  nz = await kA.nicheOf(a3.address);
  ok(nz.found && Sx[Number(nz.sector)] === 'UNCERTAIN', 'a3 signalled no-position and sits in UNCERTAIN');
  nz = await kA.nicheOf(stranger.address);
  ok(!nz.found, 'an agent with no history has no niche');

  // ---------------------------------------------------------------- first commits (accepted proposals)
  sec('ordinary commits: predicate, attribution, accepted proposals');
  ok(await rn(K(a1), 'commit', R1) === 'BadLifecycle', 'cannot commit before the lifecycle completes');
  const P1 = b32('P1');
  await full(a4, 'P1', T1, [a1, a2, a3, a5, a6]);
  ok(await lc(P1) === 8, 'P1 COMMITTABLE');
  ok(await mask(P1) === 0n, 'predicate mask is zero');
  await tx(K(rando).commit(P1));
  ok(await lc(P1) === 9 && (await kA.declaredState()).index === 1n, 'P1 COMMITTED, declared state advanced');
  ok((await kA.acceptedProposals(a4.address)) === 1n, 'a4 has one accepted proposal');
  ok(await rn(K(rando), 'commit', P1) === 'BadLifecycle', 'COMMITTED cannot be committed again');
  ok(await rn(K(a4), 'registerTransition', b32('stale'), b32('staleS'), await mf(T1, gp)) === 'ParentNeDeclared', 'stale parent rejected at registration');
  console.log('settlement and outcome (attestor logic unchanged)');
  await tx(K(executor).startExecution(P1));
  await tx(K(settler).attestSettlement(P1, b32('P1real'), H('x')));
  ok(await lc(P1) === 10, 'one attestation is below the threshold of two');
  ok(await rn(K(settler), 'attestSettlement', P1, b32('P1real'), H('x')) === 'AlreadyAttested', 'one attestation per settler');
  await tx(K(rando).attestSettlement(P1, b32('P1real'), H('x')));
  ok(await lc(P1) === 11, 'two agreeing attestations settle');
  await tx(K(recorder).recordOutcome(P1, [b32('E1')], [], Z));
  ok(await lc(P1) === 12 && (await kA.outcomes(P1)).successful === false, 'outcome recorded; hash mismatch derived, not asserted');
  ok((await kA.agents(a4.address)).reputation === 5n * WAD, 'attribution moved reputation');

  const stateSnap = async () => ({ a: await kA.declaredState() });
  // more accepted proposals for a5, a6, a7 (council candidacy needs one each), with the group-max test on ORD2
  sec('group policy: highest-matching group, never the sum');
  const P2 = b32('P2');
  await register(a5, 'P2', T2, { groups: [G, G2] }); await toEligible(P2);
  await sign(a5, P2, WAD, WAD); await sign(a6, P2, WAD, WAD);
  const w5 = (await kA.roundSignals(P2, 0, a5.address)).effectiveWeight, w6 = (await kA.roundSignals(P2, 0, a6.address)).effectiveWeight;
  ok(w6 === WAD, 'a6 is in one group (x2): 0.5 * 2 = 1.0');
  ok(w5 === 3n * WAD / 2n, 'a5 is in two groups (x2 and x3): highest-matching x3 => 1.5, not x5 => 2.5');
  await sign(a1, P2, WAD, WAD); await sign(a2, P2, WAD, WAD);
  await finish(P2, a5); await tx(K(rando).commit(P2));
  ok((await kA.acceptedProposals(a5.address)) === 1n, 'a5 accepted');
  const P3 = b32('P3'); await full(a6, 'P3', T1, [a1, a2, a3, a5, a6]); await tx(K(rando).commit(P3));
  const P4 = b32('P4'); await full(a7, 'P4', T1, [a1, a2, a3, a5, a6]); await tx(K(rando).commit(P4));
  ok((await kA.acceptedProposals(a6.address)) === 1n && (await kA.acceptedProposals(a7.address)) === 1n, 'a6 and a7 accepted');

  globalThis.__ctx = { prov, S, abi, iface, K, kA, rn, mine, now, tx, mf, parent, register, toEligible, sign, finish, full, lc, mask, ok, sec, H, b32, Z, WAD, UNC, DAY, G, G2, T1, GA, GC, GCO, GR, pol, pos, agentsL, KO, att, eng, dep, admin, a1, a2, a3, a4, a5, a6, a7, rando, stranger, guardian, executor, settler, recorder, observer, Sx };
  await require('./part2.js')(globalThis.__ctx);
  console.log(`\n${pass} passed, ${fail} failed`); process.exit(fail ? 1 : 0);
})().catch(e => { console.error(e); process.exit(2); });
module.exports.count = () => ({ pass, fail });
