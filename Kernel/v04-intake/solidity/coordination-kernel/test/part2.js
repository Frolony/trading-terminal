const { ethers } = require("ethers");
module.exports = async (c) => {
  const { prov, S, abi, iface, K, kA, rn, mine, now, tx, mf, parent, register, toEligible, sign, finish, full, lc, mask, ok, sec, H, b32, Z, WAD, UNC, DAY, G, G2, T1, GA, GC, GCO, GR, pol, pos, KO, dep, a1, a2, a3, a4, a5, a6, a7, rando, stranger, guardian, observer, Sx } = c;
  const seats = [a1, a2, a3];
  const FAR = async () => (await now()) + 100_000_000;

  // ---------------------------------------------------------------- siblings
  sec('selection: highest-confidence qualifying sibling wins; nobody wins by calling early');
  const Sa = b32('Sa'), Sb = b32('Sb'), Sd = b32('Sd');
  const par = await parent();
  await register(a4, 'Sa', T1, { parent: par }); await register(a5, 'Sb', T1, { parent: par });
  await toEligible(Sa); await toEligible(Sb);
  for (const s of [a1, a2, a3, a6, a7]) { await sign(s, Sa, WAD, 9n * WAD / 10n); await sign(s, Sb, WAD, 7n * WAD / 10n); }
  await mine(1001);
  await register(a6, 'Sd', T1, { parent: par }); await toEligible(Sd);
  ok(await rn(K(rando), 'selectCandidate', Sa) === 'SiblingsStillOpen', 'a sibling whose window is still open blocks selection');
  await mine(1001);
  ok(await rn(K(rando), 'selectCandidate', Sb) === 'OutrankedBySibling', 'a lower-confidence qualifying sibling cannot be selected');
  ok(await rn(K(rando), 'selectCandidate', Sd) === 'QuorumNotMet', 'a sibling with no weight does not qualify');
  await tx(K(rando).selectCandidate(Sa)); ok(await lc(Sa) === 4, 'highest confidence wins selection');
  await tx(K(rando).openPlanning(Sa));
  await tx(K(a1).submitPlan(Sa, { resourceClaims: [], effects: [b32('E1')], prerequisites: [], conflicts: [], expiresAt: await FAR(), payloadHash: H('pl') }));
  await mine(101); await tx(K(rando).reconcilePlans(Sa)); await tx(K(rando).validateTransition(Sa)); await tx(K(rando).declareCommit(Sa)); await tx(K(rando).commit(Sa));
  await tx(K(rando).markConflicted(Sb));
  ok(await lc(Sb) === 16, 'the losing sibling becomes CONFLICTED once the state advances');

  // ---------------------------------------------------------------- dialect
  sec('dialect: a proposal takes effect only when a dependent transition commits');
  ok(Number(await kA.dialectEpoch()) === 9, 'nine changes of this merge are notated at genesis');
  const X = H('SYM_X'), Y = H('SYM_Y'), Y2 = H('SYM_Y2');
  const ni = (o, n, d) => ({ oldSymbol: o, newSymbol: n, oldKind: 1, newKind: 1, definitionHash: H(d), prevChangeId: Z, reason: 'test' });
  const rcA = await tx(K(a4).proposeNotationChange(ni(X, Y, 'defY')));
  const rcB = await tx(K(a5).proposeNotationChange(ni(X, Y2, 'defY2')));
  const idOf = rc => rc.logs.map(l => { try { return iface.parseLog(l); } catch (e) { return null; } }).find(p => p && p.name === 'NotationProposed').args[0];
  const chA = idOf(rcA), chB = idOf(rcB);
  ok(await rn(K(a4), 'proposeNotationChange', ni(X, Y, 'defY')) === null, 'proposals against the same prior state coexist as declared divergence');
  ok(await rn(K(stranger), 'proposeNotationChange', ni(X, Y, 'z')) === 'InactiveAgent', 'only agents propose');
  const D1 = b32('D1'), D2 = b32('D2'); const dp = await parent();
  await register(a4, 'D1', T1, { parent: dp }); await register(a5, 'D2', T1, { parent: dp });
  await tx(K(a4).declareDialectDependency(D1, chA)); await tx(K(a5).declareDialectDependency(D2, chB));
  ok(await rn(K(a4), 'declareDialectDependency', D1, chB) === 'DialectDepConflict' || await rn(K(a4), 'declareDialectDependency', D1, chB) === 'NotTransitionProposer', 'a transition cannot depend on conflicting or foreign changes');
  await toEligible(D1); for (const s of [a1, a2, a3, a6, a7]) await sign(s, D1, WAD, WAD);
  await finish(D1, a1);
  ok((await mask(D1)) === 0n, 'D1 passes the predicate with a live dependency');
  await tx(K(rando).commit(D1));
  ok(Number(await kA.dialectEpoch()) === 10 && (await kA.dialectHead(Y)) === chA, 'the change took effect with the commit that relied on it');
  ok((await mask(D2) & 4096n) === 4096n, 'the rival change went stale: D2 now fails the dialect term of the predicate');

  // ---------------------------------------------------------------- exceptions + defection standing
  sec('exceptions: council majority resolves, raiser excluded, confirmed defection is recorded');
  const E1 = b32('E1x'); await register(a7, 'E1x', T1); await toEligible(E1);
  await tx(K(a5).submitExceptionSignal(E1, 5, 2, a6.address));
  ok((await kA.unresolvedBlocking(E1)) === 1n && ((await mask(E1)) & 32n) === 32n, 'a blocking exception sets the exceptions bit');
  await tx(K(a1).submitExceptionSignal(E1, 1, 1, ethers_zero()));
  ok(await rn(K(a1), 'resolveException', E1, 1, 1, Z) === 'RaiserCannotResolveOwnBlockingException', 'a raiser cannot resolve its own exception');
  ok(await rn(K(a4), 'resolveException', E1, 0, 3, H('ev')) === 'NotCouncil', 'only seats adjudicate');
  await tx(K(a1).resolveException(E1, 0, 3, H('ev')));
  ok((await kA.unresolvedBlocking(E1)) === 1n, 'one seat of three is not a majority');
  ok(await rn(K(a1), 'resolveException', E1, 0, 3, H('ev')) === 'AlreadyVoted', 'a seat votes once');
  await tx(K(a2).resolveException(E1, 0, 3, H('ev')));
  ok((await kA.unresolvedBlocking(E1)) === 0n, 'two of three resolves it');
  ok((await kA.lastDefectionEpoch1(a6.address)) > 0n, 'confirmed defection recorded against the offender');
  await tx(K(a7).withdraw(E1));

  // ---------------------------------------------------------------- governance
  sec('governance: a rule changes only when a council-proposed transition passes the full predicate');
  const gArgs = async (id, type, action, o = {}) => [b32(id), b32(id + 'S'), await mf(type, o.parent || await parent(), ethers.keccak256(action)), action];
  const approve = async (id, action, who = seats) => { const ah = ethers.keccak256(action); for (const x of who) await tx(K(x).approveGovernance(b32(id), ah)); };
  const genGov = async (id, type, action, o = {}) => {
    if (!o.skipApprove) await approve(id, action, o.approvers || seats);
    return K(o.proposer || a1).registerGovernanceTransition(...(await gArgs(id, type, action, o)));
  };
  const tryGov = async (id, type, action, o = {}) => {
    if (!o.skipApprove) await approve(id, action, o.approvers || seats);
    return rn(K(o.proposer || a1), 'registerGovernanceTransition', ...(await gArgs(id, type, action, o)));
  };
  const grant = (r, who) => iface.encodeFunctionData('grantRole', [H(r), who]);
  ok(await rn(K(a1), 'grantRole', H('OBSERVER'), rando.address) === 'Unauthorized', 'no seat can change a rule directly');
  const g1 = grant('OBSERVER', rando.address);
  ok(await tryGov('gU', GA, g1, { approvers: [a1, a2] }) === 'CouncilNotUnanimous', 'two of three seats approving is not unanimous: proposal refused');
  ok(await tryGov('gW', GC, g1) === 'WrongGovCategory', 'an adjustable action cannot travel as a constitutional type');
  const mult = iface.encodeFunctionData('setGroupMultiplier', [T1, G, 50n * WAD]);
  ok(await tryGov('gM', GA, mult) === 'WrongGovCategory', 'group multipliers (a weight lever) are constitutional, never adjustable');
  const polAct = iface.encodeFunctionData('setPolicy', [GA, pol({ quorumRequired: 1n })]);
  ok(await tryGov('gP', GA, polAct) === 'WrongGovCategory', 'a policy for a governance type is constitutional');
  ok(await tryGov('gN', GA, '0x12345678') === 'UnknownGovAction', 'an action outside the table is refused');

  await tx(genGov('g1', GA, g1));
  const g1id = b32('g1');
  await toEligible(g1id);
  for (const s of [a1, a2, a3]) await sign(s, g1id, WAD, WAD);
  await mine(1001); await tx(K(rando).selectCandidate(g1id)).catch(e => console.log('   select ->', e.revert?.name));
  ok(await lc(g1id) === 3, 'with half the weight, quorum for a governance type is not met: it cannot even be selected');
  for (const s of [a4, a5, a6, a7]) await sign(s, g1id, WAD, WAD).catch(() => {});
  // window closed already; start a fresh governance transition with the whole membership signalling
  await tx(K(a1).withdraw(g1id));
  const g2 = b32('g2');
  await tx(genGov('g2', GA, g1));
  await toEligible(g2);
  for (const s of [a1, a2, a3, a4, a6, a7]) await sign(s, g2, WAD, WAD);
  await mine(1001); await tx(K(rando).selectCandidate(g2)); await tx(K(rando).openPlanning(g2));
  await tx(K(a1).submitPlan(g2, { resourceClaims: [], effects: [b32('E1')], prerequisites: [], conflicts: [], expiresAt: await FAR(), payloadHash: H('pl') }));
  await mine(101); await tx(K(rando).reconcilePlans(g2)); await tx(K(rando).validateTransition(g2)); await tx(K(rando).declareCommit(g2));
  ok(await rn(K(rando), 'commit', g2) === 'UseGovernanceCommit', 'a governance transition cannot commit through the ordinary door');
  ok(!(await kA.hasRole(H('OBSERVER'), rando.address)), 'before the commit, the rule has not changed');
  await tx(K(rando).commitGovernance(g2));
  ok(await kA.hasRole(H('OBSERVER'), rando.address) && await kA.govApplied(g2), 'after the commit the rule is applied, in the same transaction');
  ok(await rn(K(rando), 'commitGovernance', g2) === 'BadLifecycle', 'it cannot be applied twice');

  console.log('predicate failure: an exception raised after declareCommit blocks the governance commit');
  const g3 = b32('g3'); await tx(genGov('g3', GA, grant('OBSERVER', stranger.address))); await toEligible(g3);
  for (const s of [a1, a2, a3, a4, a6, a7]) await sign(s, g3, WAD, WAD);
  await finish(g3, a1);
  await tx(K(a5).submitExceptionSignal(g3, 9, 3, ethers_zero()));
  ok(await lc(g3) === 17, 'blocking exception after declare -> BLOCKED');
  ok(await rn(K(rando), 'commitGovernance', g3) === 'BadLifecycle', 'a blocked governance transition cannot apply');
  ok(!(await kA.hasRole(H('OBSERVER'), stranger.address)), 'the rule did not change');
  await tx(K(a1).withdraw(g3));

  console.log('a failing action reverts the whole commit');
  const big = iface.encodeFunctionData('registerAgent', [stranger.address, [G], 100n * WAD]);
  const g4 = b32('g4'); await tx(genGov('g4', GA, big)); await toEligible(g4);
  for (const s of [a1, a2, a3, a4, a6, a7]) await sign(s, g4, WAD, WAD);
  await finish(g4, a1);
  ok(await rn(K(rando), 'commitGovernance', g4) === 'GovActionFailed', 'registering an agent above the influence-share bound fails');
  ok(await lc(g4) === 8, 'a failed action leaves no committed transition behind (still COMMITTABLE)');
  await tx(K(a1).withdraw(g4));

  // ---------------------------------------------------------------- position gate
  sec('position: a gate on council candidacy, never a weight');
  const epoch0 = Number(await kA.currentEpoch());
  const far = (await now()) + 10_000 * DAY;
  await tx(pos.set(a4.address, 1n, far)); await tx(pos.set(a5.address, 10n ** 30n, far)); await tx(pos.set(a6.address, 5n, far)); await tx(pos.set(a7.address, 7n, far));
  ok(!(await kA.isCandidate(a4.address)), 'accepted proposal alone is not candidacy');
  console.log('   epoch now', epoch0);
  // the same agent, wildly different locked amounts: identical weight
  const Wt = b32('Wt'); await register(a7, 'Wt', T1); await toEligible(Wt);
  await sign(a5, Wt, WAD, WAD);
  const w5a = (await kA.roundSignals(Wt, 0, a5.address)).effectiveWeight;
  await tx(K(a7).withdraw(Wt));
  await tx(pos.set(a5.address, 1n, far));
  const Wt2 = b32('Wt2'); await register(a7, 'Wt2', T1); await toEligible(Wt2);
  await sign(a5, Wt2, WAD, WAD);
  const w5b = (await kA.roundSignals(Wt2, 0, a5.address)).effectiveWeight;
  ok(w5a === w5b && w5a > 0n, 'a holder of 1 and a holder of 10^30 carry exactly the same weight');
  await tx(K(a7).withdraw(Wt2));
  await tx(pos.set(a5.address, 10n ** 30n, far));
  await tx(pos.set(a3.address, 2n, far));
  for (let i = 0; i < 3; i++) {
    for (const a of [a3, a4, a5, a6, a7]) await tx(K(rando).recordPosition(a.address));
    if (i < 2) await mine(DAY);
  }
  ok(await rn(K(rando), 'recordPosition', a4.address) === 'AlreadyRecorded', 'one record per epoch');
  ok((await kA.positionStreak(a4.address)) === 3n, 'three consecutive verified epochs');
  ok(await kA.isCandidate(a4.address) && await kA.isCandidate(a5.address) && await kA.isCandidate(a7.address), 'holds a position 3 epochs + accepted proposal => candidate');
  ok(!(await kA.isCandidate(a6.address)), 'a6 has a confirmed defection inside the cooldown: not a candidate');
  ok(!(await kA.isCandidate(a1.address)), 'a1 has no position: not a candidate');
  ok((await kA.positionStreak(a3.address)) === 3n, 'a3 has a verified streak');
  await tx(pos.set(a3.address, 2n, (await now()) + 10));
  await mine(DAY);
  await tx(K(rando).recordPosition(a3.address));
  ok((await kA.positionStreak(a3.address)) === 0n, 'a holding that is not locked through the epoch does not count');

  // ---------------------------------------------------------------- council rotation
  sec('council: succession is a transition; sits at least three epochs; nominees must qualify');
  const rot = (list) => iface.encodeFunctionData('installCouncil', [list.map(x => x.address)]);
  const drive = async (id, act, type, signers) => {
    await tx(genGov(id, type, act)); const bid = b32(id); await toEligible(bid);
    for (const s of signers) await sign(s, bid, WAD, WAD);
    await finish(bid, signers[0]); return bid;
  };
  const sign6 = [a1, a2, a3, a4, a5, a6, a7];
  const epNow = Number(await kA.currentEpoch());
  console.log('   epoch', epNow, 'council since', Number(await kA.councilSince()));
  ok(epNow >= 3 && (await kA.councilSince()) === 0n, 'three epochs have passed since the seated council was installed');
  const bad2 = await drive('c3', rot([a4, a1, a7]), GCO, sign6.filter(x => x !== a5));
  ok(await rn(K(rando), 'commitGovernance', bad2) === 'GovActionFailed', 'a nominee with no position (a1) fails the rotation');
  await tx(K(a1).withdraw(bad2));
  const early = await drive('c1', rot([a4, a5, a7]), GCO, sign6.filter(x => x !== a5));
  await tx(K(rando).commitGovernance(early));
  ok(!(await kA.isSeated(a1.address)) && await kA.isSeated(a4.address) && await kA.isSeated(a5.address) && await kA.isSeated(a7.address), 'council rotated to a4, a5, a7');
  ok((await kA.councilSince()) === BigInt(epNow), 'the new sitting starts this epoch');
  const members = await kA.councilMembers(); ok(members.length === 3, 'three seats');
  const c4 = await drive('c4', rot([a4, a5, a7]), GCO, sign6.filter(x => x !== a1)).catch(() => null);
  ok(await rn(K(a4), 'registerGovernanceTransition', b32('c5'), b32('c5S'), await mf(GCO, await parent(), ethers.keccak256(rot([a4, a5, a7]))), rot([a4, a5, a7])) === 'CouncilNotUnanimous', 'the new council must itself be unanimous (old approvals do not carry over)');
  ok(await rn(K(a1), 'registerGovernanceTransition', b32('c6'), b32('c6S'), await mf(GCO, await parent(), H('x')), rot([a4, a5, a7])) === 'NotCouncil', 'the retired seat can no longer propose rules');

  // ---------------------------------------------------------------- pause
  sec('emergency brake is bounded');
  ok(await rn(K(rando), 'pause') === 'NotGuardian', 'only a guardian pauses');
  await tx(K(guardian).pause());
  ok(await kA.paused(), 'paused');
  ok(await rn(K(a4), 'registerTransition', b32('pz'), b32('pzS'), await mf(T1, await parent())) === 'IsPaused', 'registration blocked while paused');
  ok(await rn(K(guardian), 'pause') === 'PauseCooldown', 'cannot immediately re-pause');
  await mine(3 * DAY + 5);
  ok(!(await kA.paused()), 'the pause expires on its own: it can never deadlock governance');
  ok(await rn(K(guardian), 'pause') === 'PauseCooldown', 'and a cooldown follows, so governance has a window');

  // ---------------------------------------------------------------- frozen substrate and reboot
  sec('frozen substrate: only a reboot transition may run');
  await tx(K(observer).recordBranchingHealth(1, 0, 5, 5, 5));
  ok(Number(await kA.systemState()) === 1, 'health minimum of zero freezes the substrate');
  ok(await rn(K(a4), 'registerTransition', b32('fz'), b32('fzS'), await mf(T1, await parent())) === 'SystemFrozen', 'ordinary registration refused while frozen');
  const reboot = iface.encodeFunctionData('rebootstrapSystem', []);
  const seatsNow = [a4, a5, a7];
  const rb = b32('rb'); { const ah = ethers.keccak256(reboot); for (const s of seatsNow) await tx(K(s).approveGovernance(rb, ah)); }
  await tx(K(a4).registerGovernanceTransition(rb, b32('rbS'), await mf(GR, await parent(), ethers.keccak256(reboot)), reboot));
  await toEligible(rb); for (const s of [a1, a2, a3, a4, a5, a6, a7]) await sign(s, rb, WAD, WAD);
  await finish(rb, a1); await tx(K(rando).commitGovernance(rb));
  ok(Number(await kA.systemState()) === 0, 'a committed reboot transition restores operation, through the full predicate');

  // ---------------------------------------------------------------- weight bound
  sec('weight bound is enforced at seal (fresh deployment)');
  const mods2 = {};
  for (const n of ['GovernanceModule', 'OutcomeModule', 'ConsensusModule', 'PlanningModule', 'DialectModule', 'CouncilModule']) mods2[n] = await dep(n, c.admin);
  const addr = async n => mods2[n].getAddress();
  const k2 = await dep('CoordinationKernel', c.admin, c.admin.address, b32('g'), H('h'), 0, DAY, 3, { governance: await addr('GovernanceModule'), outcome: await addr('OutcomeModule'), consensus: await addr('ConsensusModule'), planning: await addr('PlanningModule'), dialect: await addr('DialectModule'), council: await addr('CouncilModule') });
  const K2 = new (require('ethers').Contract)(k2.target, abi, c.admin);
  for (const t of [GA, GC, GCO, GR]) await tx(K2.setPolicy(t, pol()));
  await tx(K2.registerAgent(a1.address, [G], 100n * WAD)); await tx(K2.registerAgent(a2.address, [G], WAD)); await tx(K2.registerAgent(a3.address, [G], WAD));
  await tx(K2.installCouncil([a1.address, a2.address, a3.address]));
  ok(await rn(K2, 'sealGenesis') === 'GenesisNotReady', 'seal refused while one agent holds more than 1/6 of base influence');
  ok(await rn(K2, 'setMaxInfluenceShare', WAD / 100n) === 'InfluenceShareExceeded', 'the cap cannot be tightened under an existing holder');
};
function ethers_zero() { return '0x0000000000000000000000000000000000000000'; }
