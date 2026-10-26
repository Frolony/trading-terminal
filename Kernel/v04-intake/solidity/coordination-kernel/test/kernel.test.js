// Run: node build.js && node kernel.test.js   (needs solc, ganache, ethers)
const ganache = require('ganache'), { ethers } = require('ethers'), art = require('./art.json');
const H = s => ethers.id(s), b32 = s => ethers.encodeBytes32String(s), Z = ethers.ZeroHash, WAD = 10n ** 18n;
const UNC = (1n << 256n) - 1n;
let pass = 0, fail = 0;
const ok = (c, m) => { if (c) { pass++; console.log('  ok  ', m); } else { fail++; console.log('  FAIL', m); } };
const section = s => console.log('\n' + s);

(async () => {
  const prov = new ethers.BrowserProvider(ganache.provider({ logging: { quiet: true }, wallet: { totalAccounts: 14 } }));
  const S = []; for (let i = 0; i < 14; i++) S.push(await prov.getSigner(i));
  const [gov, observer, a1, a2, a3, a4, a5, executor, settler, recorder, guardian, rando, a6, a7] = S;
  const dep = async (n, signer, ...a) => { const f = new ethers.ContractFactory(art[n].abi, art[n].bin, signer); const c = await f.deploy(...a); await c.waitForDeployment(); return c; };
  const mine = async (s = 0) => { await prov.send('evm_increaseTime', [s]); await prov.send('evm_mine', []); };
  const now = async () => (await prov.getBlock('latest')).timestamp;

  // ---- deploy
  const mods = {};
  for (const [k, n] of [['governance', 'GovernanceModule'], ['outcome', 'OutcomeModule'], ['consensus', 'ConsensusModule'], ['planning', 'PlanningModule'], ['dialect', 'DialectModule'], ['council', 'CouncilModule']]) mods[k] = (await dep(n, gov)).target;
  const EPOCH = 100000n;
  const k0 = await dep('CoordinationKernel', gov, gov.address, b32('genesis'), H('gh'), 0, EPOCH, 3, mods);
  const seen = new Set(), abi = [];
  for (const n of Object.keys(art)) for (const e of art[n].abi) { if (e.type === 'constructor' || e.type === 'fallback') continue; const key = e.type + (e.name || '') + JSON.stringify((e.inputs || []).map(i => i.type)); if (!seen.has(key)) { seen.add(key); abi.push(e); } }
  const K = s => new ethers.Contract(k0.target, abi, s);
  const kg = K(gov);
  const rn = async (c, fn, ...a) => { try { await c[fn].staticCall(...a); return null; } catch (e) { return e.revert?.name || (e.data ? 'raw:' + e.data.slice(0, 10) : (e.shortMessage || String(e).slice(0, 90))); } };
  const tx = async p => (await p).wait();
  const lc = async id => Number(await kg.transitionLifecycle(id));

  const T = b32('TYPE_A'), G = b32('G'), G2 = b32('G2');
  const GT = { adj: H('GOV_ADJUSTABLE'), con: H('GOV_CONSTITUTION'), cou: H('GOV_COUNCIL'), reb: H('GOV_REBOOT') };
  const pol = (q, c, extra = {}) => ({ exists: false, quorumRequired: q, confidenceThreshold: c, coverageMin: WAD, minObsConfidence: 0n, consensusWindow: 1000n, planningWindow: 100n, executionWindow: 100000n, maxPlans: 4n, requireObjectives: true, settlementThreshold: 1n, maxConsensusRounds: 2n, ...extra });

  section('genesis: authority exists only before the seal');
  ok(await rn(K(rando), 'registerAgent', rando.address, [G], WAD) === 'Unauthorized', 'a stranger cannot register an agent');
  const cGood = await dep('MockConstraint', gov, true), obj = await dep('MockObjective', gov), rel = await dep('MockRelevance', gov), eng = await dep('MockEngine', gov);
  const att = await dep('MockAttribution', gov, a1.address), lock = await dep('MockLock', gov);
  const C1 = b32('C1'), J1 = b32('J1'), R1 = b32('R1'), A1 = b32('A1');
  for (const [r, w] of [['OBSERVER', observer], ['EXECUTOR', executor], ['SETTLER', settler], ['OUTCOME_RECORDER', recorder], ['GUARDIAN', guardian]]) await tx(kg.grantRole(H(r), w.address));
  await tx(kg.registerImpl(0, C1, cGood.target)); await tx(kg.registerImpl(1, J1, obj.target)); await tx(kg.registerImpl(2, R1, rel.target)); await tx(kg.registerImpl(3, A1, att.target));
  await tx(kg.registerEngine(eng.target));
  await tx(kg.setPolicy(T, pol(2n * WAD, WAD / 2n)));
  for (const t of Object.values(GT)) await tx(kg.setPolicy(t, pol(3n * WAD, 66n * WAD / 100n)));
  for (const t of [T, ...Object.values(GT)]) await tx(kg.setRequiredSets(t, [C1], [J1], [R1], [A1]));
  const agentsAll = [a1, a2, a3, a4, a5, a6, a7];
  for (const a of agentsAll) await tx(kg.registerAgent(a.address, a === a6 ? [G, G2] : [G], 2n * WAD));
  await tx(kg.setPositionSource(lock.target));
  ok(await rn(kg, 'sealGenesis') === 'GenesisNotReady', 'seal refused while the council is empty');
  await tx(kg.installCouncil([a1.address, a2.address, a3.address]));
  ok((await kg.councilMembers()).length === 3 && await kg.isSeated(a1.address), 'genesis council seated');
  ok(await rn(kg, 'installCouncil', [a1.address, a2.address]) === 'BadSeats', 'wrong seat count refused');

  // ---- helpers for driving a transition through the lifecycle
  const t0 = await now();
  let obsN = 0;
  const newObs = async () => { const id = b32('obs' + (++obsN)); await tx(K(observer).ingestObservation(id, await now(), H('s'), H('d'), H('m'), WAD, (await now()) + 1_000_000_000)); return id; };
  const manifest = async (type, obs, groups = [G], payload = H('p')) => ({ parentStateId: (await kg.declaredState()).id, transitionType: type, relevantGroups: groups, changedVars: [], constraints: [], resources: [], expectedEffects: [b32('E1')], expiresAt: (await now()) + 1_000_000_000, observationId: obs, resultingStateHash: H('S' + obs), transitionPayloadHash: payload });
  const finish = async (id, planner, commitFn) => {
    await tx(K(rando).evaluateTransition(id)); await tx(K(rando).markEligible(id));
  };
  const afterSignals = async (id, planner, gov_) => {
    await mine(1001);
    await tx(K(rando).selectCandidate(id));
    await tx(K(rando).openPlanning(id));
    await tx(K(planner).submitPlan(id, { resourceClaims: [], effects: [b32('E1')], prerequisites: [], conflicts: [], expiresAt: (await now()) + 1_000_000_000, payloadHash: H('plan') }));
    await mine(101);
    await tx(K(rando).reconcilePlans(id)); await tx(K(rando).validateTransition(id)); await tx(K(rando).declareCommit(id));
    if (gov_) await tx(K(rando).commitGovernance(id)); else await tx(K(rando).commit(id));
  };
  const commitOrdinary = async (label, proposer, signers = [a1, a2, a3], support = WAD) => {
    const id = b32(label), obs = await newObs();
    await tx(K(proposer).registerTransition(id, b32(label + 'S'), await manifest(T, obs)));
    await finish(id);
    for (const s of signers) await tx(K(s).submitRoundSignal(id, WAD, support));
    await afterSignals(id, signers[0], false);
    return id;
  };
  const approveAll = async (id, action) => { const ah = ethers.keccak256(action); for (const m of [a1, a2, a3]) if (await kg.isSeated(m.address)) await tx(K(m).approveGovernance(id, ah)); return ah; };
  const prepGov = async (label, proposer, type, action, signers = [a4, a5, a6, a7], groups = [G]) => {
    const id = b32(label), obs = await newObs();
    const ah = ethers.keccak256(action);
    for (const m of await kg.councilMembers()) await tx(K(S.find(s => s.address === m)).approveGovernance(id, ah));
    await tx(K(proposer).registerGovernanceTransition(id, b32(label + 'S'), await manifest(type, obs, groups, ah), action));
    await finish(id);
    for (const s of signers) await tx(K(s).submitRoundSignal(id, WAD, WAD));
    await mine(1001);
    await tx(K(rando).selectCandidate(id)); await tx(K(rando).openPlanning(id));
    await tx(K(signers[0]).submitPlan(id, { resourceClaims: [], effects: [b32('E1')], prerequisites: [], conflicts: [], expiresAt: (await now()) + 1_000_000_000, payloadHash: H('plan') }));
    await mine(101);
    await tx(K(rando).reconcilePlans(id)); await tx(K(rando).validateTransition(id)); await tx(K(rando).declareCommit(id));
    return id;
  };
  const commitGov = async (label, proposer, type, action, signers = [a4, a5, a6, a7]) => {
    const id = b32(label), obs = await newObs();
    // approve on behalf of whoever sits now
    const ah = ethers.keccak256(action);
    for (const m of await kg.councilMembers()) await tx(K(S.find(s => s.address === m)).approveGovernance(id, ah));
    await tx(K(proposer).registerGovernanceTransition(id, b32(label + 'S'), await manifest(type, obs, [G], ah), action));
    await finish(id);
    for (const s of signers) await tx(K(s).submitRoundSignal(id, WAD, WAD));
    await afterSignals(id, signers[0], true);
    return id;
  };

  await tx(kg.sealGenesis());
  ok(await kg.genesisSealed() && (await kg.genesisAdmin()) === ethers.ZeroAddress, 'sealed: bootstrap authority gone');
  ok(await rn(kg, 'registerAgent', rando.address, [G], WAD) === 'Unauthorized', 'former admin can no longer register an agent');
  ok(await rn(kg, 'setPolicy', T, pol(1n, 1n)) === 'Unauthorized', 'former admin can no longer change policy');
  ok(await rn(kg, 'grantRole', H('OBSERVER'), rando.address) === 'Unauthorized', 'former admin can no longer grant roles');
  ok(await rn(kg, 'sealGenesis') === 'NotGenesis', 'seal cannot be repeated');

  section('ordinary transition: rounds, classifier, selection, commit');
  const obs = await newObs();
  const tA = b32('tA');
  ok(await rn(K(rando), 'registerTransition', tA, b32('SA'), await manifest(T, obs)) === 'InactiveAgent', 'only an active agent proposes');
  ok(await rn(K(a4), 'registerTransition', tA, b32('SA'), await manifest(GT.adj, obs)) === 'GovernanceTypeOnly', 'governance type refused as an ordinary transition');
  ok(await rn(K(a4), 'registerTransition', tA, b32('SA'), await manifest(b32('NOPE'), obs)) === 'NoPolicyForType', 'unknown type refused (no proposer-supplied policy)');
  await tx(K(a4).registerTransition(tA, b32('SA'), await manifest(T, obs)));
  await finish(tA);
  ok(await lc(tA) === 3, 'ELIGIBLE');
  await tx(K(a1).submitRoundSignal(tA, WAD, WAD));
  ok(await rn(K(a1), 'submitRoundSignal', tA, WAD, WAD) === 'AlreadySignalledThisRound', 'one signal per agent per round');
  ok(await rn(K(a2), 'submitRoundSignal', tA, WAD + 1n, WAD) === 'ValueOutOfRange', 'raw weight above WAD refused (cap restored)');
  ok(await rn(K(a2), 'submitRoundSignal', tA, WAD, WAD + 1n) === 'ValueOutOfRange', 'support above WAD refused');
  ok(await rn(K(rando), 'submitRoundSignal', tA, WAD, WAD) === 'NotEligible', 'non-agent cannot signal');
  ok((await kg.tallies(tA)).totalWeight === WAD, 'effective weight = 1e18');
  await tx(K(a2).submitRoundSignal(tA, WAD, UNC));
  await tx(K(a3).submitRoundSignal(tA, WAD, WAD / 2n));
  ok((await kg.tallies(tA)).totalWeight === 2n * WAD && (await kg.tallies(tA)).count === 3n, 'UNCERTAIN signal is recorded but adds no weight');
  ok(await rn(K(rando), 'selectCandidate', tA) === 'ConsensusStillOpen', 'cannot select while the window is open');
  await mine(520); // round 1
  ok(Number(await kg.roundOf(tA)) === 1, 'time derives the round');
  ok(await rn(K(a3), 'submitRoundSignal', tA, WAD, 4n * WAD / 10n) === 'MonotoneRevisionViolation', 'a revision may not move away from the mean the agent last saw');
  await tx(K(a1).submitRoundSignal(tA, WAD, 9n * WAD / 10n));
  ok((await kg.tallies(tA)).totalWeight === 2n * WAD, 'revision replaces the earlier signal, not adds to it');
  await tx(K(a3).submitRoundSignal(tA, WAD, 7n * WAD / 10n));
  await tx(K(a2).submitRoundSignal(tA, WAD, 8n * WAD / 10n));
  ok((await kg.tallies(tA)).totalWeight === 3n * WAD, 'three agents counted once each');
  ok(await rn(K(a1), 'submitRoundSignal', tA, WAD, 9n * WAD / 10n) === 'AlreadySignalledThisRound', 'second signal in the same later round refused');
  // sibling field: B at lower confidence, same parent
  const obsB = await newObs(), tB = b32('tB');
  await tx(K(a5).registerTransition(tB, b32('SB'), await manifest(T, obsB))); await finish(tB);
  for (const s of [a1, a2, a3]) await tx(K(s).submitRoundSignal(tB, WAD, 7n * WAD / 10n));
  await mine(1001);
  ok(await rn(K(rando), 'selectCandidate', tB) === 'OutrankedBySibling', 'lower-confidence sibling cannot be selected first');
  await tx(K(rando).selectCandidate(tA));
  await tx(K(rando).openPlanning(tA));
  await tx(K(a1).submitPlan(tA, { resourceClaims: [], effects: [b32('E1')], prerequisites: [], conflicts: [], expiresAt: (await now()) + 1_000_000_000, payloadHash: H('plan') }));
  ok(await rn(K(rando), 'reconcilePlans', tA) === 'PlanningWindowOpen', 'reconcile waits for the window');
  await mine(101);
  await tx(K(rando).reconcilePlans(tA)); await tx(K(rando).validateTransition(tA)); await tx(K(rando).declareCommit(tA));
  ok(await rn(K(rando), 'commitGovernance', tA) === 'NotGovernanceTransition', 'ordinary transition cannot use the governance commit');
  await tx(K(rando).commit(tA));
  ok(await lc(tA) === 9 && (await kg.declaredState()).index === 1n, 'COMMITTED, declared state advanced');
  ok((await kg.acceptedProposals(a4.address)) === 1n, 'proposer credited with an accepted proposal');
  await tx(K(rando).markConflicted(tB));
  ok(await lc(tB) === 16, 'losing sibling becomes CONFLICTED');
  ok(await rn(K(rando), 'commit', tA) === 'BadLifecycle', 'COMMITTED cannot be committed again');

  section('classifier: all 48 cells pinned, niche accumulates, nothing enters consensus');
  const SEC = ['ALIGNED', 'MISALIGNED', 'COLLABORATIVE', 'DIVERGENT', 'ADVERSARIAL', 'SPECIALIST', 'DISASSOCIATED', 'ABSTAINING', 'UNCERTAIN'];
  const expSec = [['COLLABORATIVE', 'ALIGNED', 'ALIGNED', 'DIVERGENT', 'ABSTAINING', 'SPECIALIST'], ['ADVERSARIAL', 'MISALIGNED', 'MISALIGNED', 'ADVERSARIAL', 'ABSTAINING', 'SPECIALIST'], ['DIVERGENT', 'UNCERTAIN', 'UNCERTAIN', 'DIVERGENT', 'ABSTAINING', 'SPECIALIST'], ['DIVERGENT', 'UNCERTAIN', 'UNCERTAIN', 'DISASSOCIATED', 'ABSTAINING', 'SPECIALIST']];
  const expW = [[20, 20, 20, 10, 0, 30], [20, 20, 20, 20, 0, 30], [10, 2, 2, 10, 0, 30], [10, 2, 2, 5, 0, 30]];
  let cellsOk = true;
  for (let s = 0; s < 4; s++) for (let c = 0; c < 6; c++) {
    if (SEC[Number(await kg.sectorOf(s, c))] !== expSec[s][c]) cellsOk = false;
    if ((await kg.baseWeightOf(s, c)) !== BigInt(expW[s][c]) * WAD / 20n) cellsOk = false;
  }
  ok(cellsOk, 'sector and weight tables equal the uploaded fork cell for cell');
  ok(Number(await kg.classifyStance(UNC)) === 3 && Number(await kg.classifyStance(WAD)) === 0 && Number(await kg.classifyStance(0)) === 1 && Number(await kg.classifyStance(WAD / 2n)) === 2, 'stance bands: uncertain / aligned / opposed / neutral');
  const ev = (await kg.queryFilter(kg.filters.SectorClassified(tA))).map(e => e.args);
  ok(ev.length >= 4, 'SectorClassified emitted per signal');
  ok((await kg.queryFilter(kg.filters.SectorReclassified(tA))).length >= 1, 'SectorReclassified emitted when an agent changes sector');
  const niche = await kg.nicheOf(a1.address);
  ok(niche.found && niche.score > 0n, 'niche accumulates from the weight table');
  ok((await kg.nicheOf(rando.address)).found === false, 'an agent with no history has no niche');

  section('governance: rules change only through a committed transition');
  // first, give the future council candidates their accepted proposals
  await commitOrdinary('p5', a5); await commitOrdinary('p6', a6);
  const actMult = kg.interface.encodeFunctionData('setGroupMultiplier', [T, G2, 3n * WAD]);
  const actMultG = kg.interface.encodeFunctionData('setGroupMultiplier', [T, G, 2n * WAD]);
  const obsG = await newObs(), gid = b32('gov1');
  ok(await rn(K(a4), 'registerGovernanceTransition', gid, b32('g1S'), await manifest(GT.adj, obsG, [G], ethers.keccak256(actMult)), actMult) === 'NotCouncil', 'only a council seat may propose a rule change');
  ok(await rn(K(a1), 'registerGovernanceTransition', gid, b32('g1S'), await manifest(GT.adj, obsG, [G], ethers.keccak256(actMult)), actMult) === 'CouncilNotUnanimous', 'proposal refused without the whole council');
  await tx(K(a1).approveGovernance(gid, ethers.keccak256(actMult))); await tx(K(a2).approveGovernance(gid, ethers.keccak256(actMult)));
  ok(await rn(K(a1), 'registerGovernanceTransition', gid, b32('g1S'), await manifest(GT.adj, obsG, [G], ethers.keccak256(actMult)), actMult) === 'CouncilNotUnanimous', 'two of three seats is not unanimous');
  await tx(K(a3).approveGovernance(gid, ethers.keccak256(actMult)));
  ok(await rn(K(rando), 'approveGovernance', gid, ethers.keccak256(actMult)) === 'NotCouncil', 'only seats can approve');
  ok(await rn(K(a1), 'registerGovernanceTransition', gid, b32('g1S'), await manifest(GT.con, obsG, [G], ethers.keccak256(actMult)), actMult) === 'WrongGovCategory', 'action must travel under its own type');
  ok(await rn(K(a1), 'registerGovernanceTransition', gid, b32('g1S'), await manifest(GT.adj, obsG, [G], H('other')), actMult) === 'GovActionMismatch', 'payload hash must commit to the exact call');
  const junk = kg.interface.encodeFunctionData('sealGenesis', []);
  ok(await rn(K(a1), 'registerGovernanceTransition', gid, b32('g1S'), await manifest(GT.adj, obsG, [G], ethers.keccak256(junk)), junk) === 'UnknownGovAction', 'unlisted call cannot be carried');
  const polAct = kg.interface.encodeFunctionData('setPolicy', [GT.adj, pol(1n, 0n)]);
  ok(await rn(K(a1), 'registerGovernanceTransition', gid, b32('g1S'), await manifest(GT.adj, obsG, [G], ethers.keccak256(polAct)), polAct) === 'WrongGovCategory', 'changing a governance type\'s own policy is constitutional, not adjustable');
  await tx(K(a1).registerGovernanceTransition(gid, b32('g1S'), await manifest(GT.adj, obsG, [G], ethers.keccak256(actMult)), actMult));
  ok((await kg.governanceActionOf(gid)) === actMult, 'the exact call is stored for evaluators to read');
  await finish(gid);
  for (const s of [a4, a5, a6]) await tx(K(s).submitRoundSignal(gid, WAD, WAD));
  await mine(1001);
  await tx(K(rando).selectCandidate(gid));
  await tx(K(rando).openPlanning(gid));
  await tx(K(a4).submitPlan(gid, { resourceClaims: [], effects: [b32('E1')], prerequisites: [], conflicts: [], expiresAt: (await now()) + 1_000_000_000, payloadHash: H('plan') }));
  await mine(101);
  await tx(K(rando).reconcilePlans(gid)); await tx(K(rando).validateTransition(gid)); await tx(K(rando).declareCommit(gid));
  ok(await rn(K(rando), 'commit', gid) === 'UseGovernanceCommit', 'a governance transition cannot take the ordinary commit path');
  ok(await kg.groupMultiplier(T, G2) === 0n, 'rule unchanged before the commit');
  await tx(K(rando).commitGovernance(gid));
  ok(await kg.groupMultiplier(T, G2) === 3n * WAD && await kg.govApplied(gid), 'rule applied in the same transaction as the commit');
  ok(await rn(kg, 'setGroupMultiplier', T, G, WAD) === 'Unauthorized', 'nobody can call the rule directly, whoever they are');
  ok(await rn(K(a1), 'setGroupMultiplier', T, G, WAD) === 'Unauthorized', 'not even a council seat');


  // ----- group policy: highest matching group, not the sum
  section('group policy is the highest-matching group, never the sum');
  {
    const obsM = await newObs(), tM = b32('tM');
    await tx(K(a4).registerTransition(tM, b32('SM'), await manifest(T, obsM, [G, G2]))); await finish(tM);
    await tx(K(a6).submitRoundSignal(tM, WAD, 1n)); // low support so it never qualifies against anything
    await tx(K(a4).submitRoundSignal(tM, WAD, 1n));
    const w6 = (await kg.roundSignals(tM, 0, a6.address)).effectiveWeight, w4 = (await kg.roundSignals(tM, 0, a4.address)).effectiveWeight;
    ok(w4 === WAD, 'agent in one matching group: multiplier 1.0 (default)');
    ok(w6 === 3n * WAD, 'agent in two matching groups (1.0 and 3.0): weight uses 3.0, not 1.0 + 3.0 = 4.0');
    await tx(K(a4).withdraw(tM));
  }

  // ----- predicate failures
  section('governance cannot pass on thin evidence');
  const actGrant = kg.interface.encodeFunctionData('grantRole', [H('OBSERVER'), rando.address]);
  {
    const gq = b32('gq'), ah = ethers.keccak256(actGrant), obsQ = await newObs();
    for (const m of [a1, a2, a3]) await tx(K(m).approveGovernance(gq, ah));
    await tx(K(a1).registerGovernanceTransition(gq, b32('gqS'), await manifest(GT.adj, obsQ, [G], ah), actGrant)); await finish(gq);
    for (const s of [a4, a5]) await tx(K(s).submitRoundSignal(gq, WAD, WAD)); // 2e18 < 3e18 quorum
    await mine(1001);
    ok(await rn(K(rando), 'selectCandidate', gq) === 'QuorumNotMet', 'two signers cannot reach the governance quorum');
    await tx(K(a1).withdraw(gq));
    ok(await lc(gq) === 13, 'withdrawn by its proposer');
  }

  section('an exception blocks governance; only a council majority clears it');
  const g3 = await prepGov('g3', a1, GT.adj, actGrant);
  ok(await lc(g3) === 8, 'COMMITTABLE');
  await tx(K(a4).submitExceptionSignal(g3, 5, 2, a6.address));
  ok(await lc(g3) === 17 && await rn(K(rando), 'commitGovernance', g3) === 'BadLifecycle', 'blocking exception: BLOCKED, commit refused');
  ok(await kg.hasRole(H('OBSERVER'), rando.address) === false, 'rule unchanged');
  ok(await rn(K(a4), 'resolveException', g3, 0, 3, H('ev')) === 'NotCouncil', 'a non-seat cannot resolve');
  await tx(K(a1).resolveException(g3, 0, 3, H('ev')));
  ok(await kg.unresolvedBlocking(g3) === 1n, 'one seat of three is not a majority');
  ok(await rn(K(a1), 'resolveException', g3, 0, 3, H('ev')) === 'AlreadyVoted', 'a seat votes once');
  await tx(K(a2).resolveException(g3, 0, 3, H('ev')));
  ok(await kg.unresolvedBlocking(g3) === 0n, 'two of three seats resolve it');
  ok((await kg.lastDefectionEpoch1(a6.address)) > 0n, 'confirmed defection recorded against the offender');
  await tx(K(rando).validateTransition(g3)); await tx(K(rando).declareCommit(g3)); await tx(K(rando).commitGovernance(g3));
  ok(await kg.hasRole(H('OBSERVER'), rando.address), 'after recovery the rule applies, and only then');
  // a seat that raised an exception cannot decide it
  {
    const g4 = await prepGov('g4', a1, GT.adj, kg.interface.encodeFunctionData('revokeRole', [H('OBSERVER'), rando.address]));
    await tx(K(a1).submitExceptionSignal(g4, 1, 2, ethers.ZeroAddress));
    ok(await rn(K(a1), 'resolveException', g4, 0, 1, H('x')) === 'RaiserCannotResolveOwnBlockingException', 'the raiser cannot vote on its own exception');
    await tx(K(a2).resolveException(g4, 0, 1, H('x'))); await tx(K(a3).resolveException(g4, 0, 1, H('x')));
    await tx(K(rando).validateTransition(g4)); await tx(K(rando).declareCommit(g4)); await tx(K(rando).commitGovernance(g4));
    ok(await kg.hasRole(H('OBSERVER'), rando.address) === false, 'second change applied through the same door');
  }

  section('dialect registry rides the commit');
  {
    const sym = s => H(s);
    const prop = async (agent, o, n, ok_, nk, defh, prev, reason) => {
      const r = await tx(K(agent).proposeNotationChange({ oldSymbol: sym(o), newSymbol: sym(n), oldKind: ok_, newKind: nk, definitionHash: defh, prevChangeId: prev, reason }));
      return r.logs.map(l => { try { return kg.interface.parseLog(l); } catch { return null; } }).find(x => x && x.name === 'NotationProposed').args.id;
    };
    ok((await kg.dialectHead(sym('F_PLANNING_OPEN'))) !== Z, 'genesis notations recorded (merge renames are legible)');
    ok(await rn(K(rando), 'proposeNotationChange', { oldSymbol: sym('x'), newSymbol: sym('y'), oldKind: 0, newKind: 0, definitionHash: Z, prevChangeId: Z, reason: 'r' }) === 'InactiveAgent', 'only agents propose notation');
    const c1 = await prop(a4, 'remnant', 'remnant_pred', 1, 2, H('def1'), Z, 'function became predicate');
    const c2 = await prop(a5, 'remnant', 'remnant_fn2', 1, 1, H('def2'), Z, 'rival rename');
    const obsD = await newObs(), tD = b32('tD'), tE = b32('tE');
    await tx(K(a4).registerTransition(tD, b32('SD'), await manifest(T, obsD))); await finish(tD);
    await tx(K(a5).registerTransition(tE, b32('SE'), await manifest(T, obsD))); 
    ok(await rn(K(a5), 'declareDialectDependency', tD, c1) === 'NotTransitionProposer', 'only the proposer declares a dependency');
    await tx(K(a4).declareDialectDependency(tD, c1)); await tx(K(a5).declareDialectDependency(tE, c2));
    ok(((await kg.commitFailMask(tE)) & 4096n) === 0n, 'live proposal against the committed dialect: predicate term holds');
    // position must not weight: lock very different amounts, equal standing
    await tx(lock.set(a4.address, 1n, 2n ** 200n)); await tx(lock.set(a5.address, 10n ** 30n, 2n ** 200n));
    for (const s of [a4, a5, a1, a2]) await tx(K(s).submitRoundSignal(tD, WAD, WAD));
    const wa = (await kg.roundSignals(tD, 0, a4.address)).effectiveWeight, wb = (await kg.roundSignals(tD, 0, a5.address)).effectiveWeight;
    ok(wa === wb && wa === WAD, 'a holder of 1 and a holder of 1e30 carry identical weight: position never weights');
    await afterSignals(tD, a4, false);
    ok((await kg.dialectHead(sym('remnant_pred'))) === c1 && (await kg.notationChanges(c1)).status === 2n, 'dialect change took effect with the commit that relied on it');
    ok(((await kg.commitFailMask(tE)) & 4096n) !== 0n, 'the rival went stale: F_DIALECT');
    ok(await rn(K(a5), 'proposeNotationChange', { oldSymbol: sym('remnant'), newSymbol: sym('zzz'), oldKind: 1, newKind: 1, definitionHash: Z, prevChangeId: Z, reason: 'stale' }) === 'StalePrevChange', 'a proposal must cite the committed head');
    ok(await rn(K(a5), 'proposeNotationChange', { oldSymbol: sym('remnant_pred'), newSymbol: sym('remnant_pred'), oldKind: 2, newKind: 2, definitionHash: H('def1'), prevChangeId: c1, reason: 'noop' }) === 'NoOpChange', 'a no-op redefinition is refused');
    await tx(K(rando).markConflicted(tE));
  }

  section('council: position gate, three epochs, succession by committed transition');
  {
    for (const a of [a4, a5, a6]) await tx(lock.set(a.address, a === a4 ? 1n : 10n ** 30n, 2n ** 200n));
    ok(await kg.isCandidate(a4.address) === false, 'not a candidate with no verified position');
    const e0 = Number(await kg.currentEpoch());
    await tx(K(rando).recordPosition(a4.address));
    ok(await rn(K(rando), 'recordPosition', a4.address) === 'AlreadyRecorded', 'one record per agent per epoch');
    // propose a succession now, while the sitting council has not served its minimum
    const succ = kg.interface.encodeFunctionData('installCouncil', [[a4.address, a5.address, a6.address]]);
    const gs = await prepGov('gs', a1, GT.cou, succ);
    const r = await rn(K(rando), 'commitGovernance', gs);
    ok(r === 'GovActionFailed', 'succession too early: the action fails and the commit reverts with it');
    ok(await lc(gs) === 8 && await kg.isSeated(a1.address), 'transition still COMMITTABLE, council unchanged');
    for (let i = 0; i < 3; i++) { await mine(Number(EPOCH)); for (const a of [a4, a5, a6]) await tx(K(rando).recordPosition(a.address)); }
    ok(Number(await kg.currentEpoch()) >= e0 + 3, 'three epochs passed');
    ok(await kg.positionStreak(a4.address) >= 3n && await kg.isCandidate(a4.address), 'held three epochs + an accepted proposal: candidate');
    ok(await kg.isCandidate(a7.address) === false, 'no position history and no accepted proposal: not a candidate');
    ok(await kg.isCandidate(a6.address) === false, 'a confirmed defection inside the cooldown keeps a6 out');
    // succession needs everyone listed to be a candidate: a6 is not
    ok(await rn(K(rando), 'commitGovernance', gs) === 'GovActionFailed', 'a nominee who is not a candidate fails the action');
    await tx(K(a1).withdraw(gs));
    // a fresh proposal with an eligible set: swap one seat only (a4 in, a3 out); keep a1,a2
    const succ2 = kg.interface.encodeFunctionData('installCouncil', [[a1.address, a2.address, a4.address]]);
    // a1 and a2 never recorded a position, so they are not candidates either: rotation is for members who qualify
    await tx(K(rando).recordPosition(a1.address));
    ok(await kg.isCandidate(a1.address) === false, 'streak of 1 is not enough for a1');
    // everyone eligible: a4 and a5 (a6 is in cooldown). Seats are 3, so two candidates cannot fill them.
    ok(await rn(K(rando), 'commitGovernance', b32('none')) === 'NotGovernanceTransition', 'unknown transition cannot be committed');
    const need3 = kg.interface.encodeFunctionData('installCouncil', [[a4.address, a5.address, a6.address]]);
    ok(need3 === succ, 'the only full slate includes a6, who is barred');
    // wait out the defection cooldown (3 epochs), then rotate
    for (let i = 0; i < 3; i++) { await mine(Number(EPOCH)); for (const a of [a4, a5, a6]) await tx(K(rando).recordPosition(a.address)); }
    ok(await kg.isCandidate(a6.address), 'cooldown over: a6 is a candidate again');
    const gs2 = await commitGov('gs2', a1, GT.cou, succ, [a4, a5, a6, a7]);
    const mem = (await kg.councilMembers()).map(x => x.toLowerCase());
    ok(mem.length === 3 && mem.includes(a4.address.toLowerCase()) && mem.includes(a5.address.toLowerCase()) && mem.includes(a6.address.toLowerCase()), 'new council seated by a committed transition');
    ok(await kg.isSeated(a1.address) === false, 'old seats vacated');
    ok(await rn(K(a1), 'registerGovernanceTransition', b32('late'), b32('lateS'), await manifest(GT.adj, await newObs(), [G], ethers.keccak256(actGrant)), actGrant) === 'NotCouncil', 'a former seat can no longer propose');
    // rotation again immediately is refused (3-epoch minimum)
    const back = kg.interface.encodeFunctionData('installCouncil', [[a4.address, a5.address, a6.address]]);
    const gs3 = await prepGov('gs3', a4, GT.cou, back, [a1, a2, a3, a7]);
    ok(await rn(K(rando), 'commitGovernance', gs3) === 'GovActionFailed', 'a seated council serves its minimum before it can be replaced');
  }

  section('pause is bounded; a freeze can be lifted only by a committed reboot');
  {
    await tx(K(guardian).pause());
    ok(await kg.paused(), 'guardian pauses');
    ok(await rn(K(a4), 'registerTransition', b32('zz'), b32('zzS'), await manifest(T, b32('obs1'))) === 'IsPaused', 'lifecycle halts while paused');
    ok(await rn(K(guardian), 'pause') === 'PauseCooldown', 'no immediate re-pause');
    await mine(3 * 86400 + 5);
    ok(!(await kg.paused()), 'the pause expired by itself: it can never deadlock governance');
    ok(await rn(K(guardian), 'pause') === 'PauseCooldown', 'cooldown still running');
    await mine(3 * 86400);
    // freeze
    await tx(K(observer).recordBranchingHealth(1, 0, WAD, WAD, WAD));
    ok(Number(await kg.systemState()) === 1, 'zero health freezes the substrate');
    ok(await rn(K(a4), 'registerTransition', b32('fz'), b32('fzS'), await manifest(T, await newObs())) === 'SystemFrozen', 'ordinary proposals stop while frozen');
    const reb = kg.interface.encodeFunctionData('rebootstrapSystem', []);
    await commitGov('reboot', a4, GT.reb, reb, [a1, a2, a3, a7]);
    ok(Number(await kg.systemState()) === 0, 'only a committed reboot transition lifts the freeze');
  }

  console.log(`\n${pass} passed, ${fail} failed`); process.exit(fail ? 1 : 0);
})().catch(e => { console.error(e); process.exit(2); });
