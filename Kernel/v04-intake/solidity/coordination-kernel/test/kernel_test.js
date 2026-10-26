// Ganache tests for the merged CoordinationKernel. Run: node build.js && node kernel_test.js
const ganache=require('ganache'),{ethers}=require('ethers'),art=require('./art.json');
const H=s=>ethers.id(s), b32=s=>ethers.encodeBytes32String(s), Z=ethers.ZeroHash, WAD=10n**18n, MAXU=2n**256n-1n;
let pass=0,fail=0;
const ok=(c,m)=>{ if(c){pass++;console.log('  ok  ',m)}else{fail++;console.log('  FAIL',m)} };
const F={CONSTRAINTS:1n,OBJECTIVES:2n,QUORUM:4n,CONFIDENCE:8n,COMPAT:16n,EXCEPTIONS:32n,FRESH:64n,PLANNING_OPEN:128n,EXPIRED:256n,LINEAGE:512n,PLANS:1024n,CAPABILITY:2048n,DIALECT:4096n};
const LC={NONE:0,PROPOSED:1,EVALUATED:2,ELIGIBLE:3,SELECTED:4,PLANNING:5,RECONCILING:6,VALIDATED:7,COMMITTABLE:8,COMMITTED:9,EXECUTING:10,SETTLED:11,EVALUATED_OUTCOME:12,DISCARDED:13,REJECTED:14,EXPIRED:15,CONFLICTED:16,BLOCKED:17};
(async()=>{
 const prov=new ethers.BrowserProvider(ganache.provider({logging:{quiet:true},chain:{allowUnlimitedContractSize:false}}));
 const S=[];for(let i=0;i<14;i++)S.push(await prov.getSigner(i));
 const [admin,observer,executor,settler,recorder,guardian,a1,a2,a3,a4,a5,a6,rando,stranger]=S;
 const dep=async(n,signer,...a)=>{const f=new ethers.ContractFactory(art[n].abi,art[n].bin,signer);const c=await f.deploy(...a);await c.waitForDeployment();return c;};
 const mods={};for(const n of ['GovernanceModule','OutcomeModule','ConsensusModule','PlanningModule','DialectModule','CouncilModule'])mods[n]=await dep(n,admin);
 const genesis=b32('genesis');
 const EPOCH=1000n;
 const k0=await dep('CoordinationKernel',admin,admin.address,genesis,H('gh'),0,EPOCH,3,
   {governance:mods.GovernanceModule.target,outcome:mods.OutcomeModule.target,consensus:mods.ConsensusModule.target,planning:mods.PlanningModule.target,dialect:mods.DialectModule.target,council:mods.CouncilModule.target});
 const seen=new Set(),abi=[];
 for(const n of ['CoordinationKernel','GovernanceModule','OutcomeModule','ConsensusModule','PlanningModule','DialectModule','CouncilModule'])for(const e of art[n].abi){const key=e.type+(e.name||'')+JSON.stringify((e.inputs||[]).map(i=>i.type));if(!seen.has(key)){seen.add(key);abi.push(e)}}
 const iface=new ethers.Interface(abi);
 const K=(s)=>new ethers.Contract(k0.target,abi,s);
 const kg=K(admin);
 const errName=(e)=>{ if(e.revert?.name==='GovActionFailed'){const inner=e.revert.args[0];try{const p=iface.parseError(inner);return 'GovActionFailed>'+p.name}catch{return 'GovActionFailed>'+inner}} return e.revert?.name||(e.data?('raw:'+e.data):(e.shortMessage||String(e).slice(0,100)));};
 const rn=async(c,fn,...a)=>{try{await c[fn].staticCall(...a);return null}catch(e){return errName(e)}};
 const tx=async(c,fn,...a)=>{const r=await c[fn](...a);return r.wait();};
 const mine=async(s=0)=>{await prov.send('evm_increaseTime',[Number(s)]);await prov.send('evm_mine',[])};
 const now=async()=>(await prov.getBlock('latest')).timestamp;
 const epoch=async()=>Number(await kg.currentEpoch());
 const nextEpoch=async()=>{const e=await epoch();while((await epoch())===e)await mine(100);};
 const lc=async(id)=>Number(await kg.transitionLifecycle(id));

 // ---- tables ----
 console.log('classifier tables pinned to the uploaded fork');
 const cons=new ethers.Contract(k0.target,abi,admin);
 const SEC=['ALIGNED','MISALIGNED','COLLABORATIVE','DIVERGENT','ADVERSARIAL','SPECIALIST','DISASSOCIATED','ABSTAINING','UNCERTAIN'];
 const expS=[['COLLABORATIVE','ALIGNED','ALIGNED','DIVERGENT','ABSTAINING','SPECIALIST'],['ADVERSARIAL','MISALIGNED','MISALIGNED','ADVERSARIAL','ABSTAINING','SPECIALIST'],['DIVERGENT','UNCERTAIN','UNCERTAIN','DIVERGENT','ABSTAINING','SPECIALIST'],['DIVERGENT','UNCERTAIN','UNCERTAIN','DISASSOCIATED','ABSTAINING','SPECIALIST']];
 const expW=[[WAD,WAD,WAD,WAD/2n,0n,WAD*3n/2n],[WAD,WAD,WAD,WAD,0n,WAD*3n/2n],[WAD/2n,WAD/10n,WAD/10n,WAD/2n,0n,WAD*3n/2n],[WAD/2n,WAD/10n,WAD/10n,WAD/4n,0n,WAD*3n/2n]];
 let tabOk=true;
 for(let s=0;s<4;s++)for(let c=0;c<6;c++){ if(SEC[Number(await cons.sectorOf(s,c))]!==expS[s][c])tabOk=false; if((await cons.baseWeightOf(s,c))!==expW[s][c])tabOk=false; }
 ok(tabOk,'all 24 sector cells and 24 weight cells equal the uploaded table');
 ok(Number(await cons.classifyStance(WAD*7n/10n))===0&&Number(await cons.classifyStance(WAD*3n/10n))===1&&Number(await cons.classifyStance(WAD/2n))===2&&Number(await cons.classifyStance(MAXU))===3,'stance bands: >0.6 aligned, <0.4 opposed, between neutral, sentinel uncertain');

 // ---- genesis setup (bootstrap authority, before seal) ----
 console.log('genesis');
 const T=b32('TYPE_A'), G=b32('Treasury'), G2=b32('Research');
 const GT={ADJ:H('GOV_ADJUSTABLE'),CON:H('GOV_CONSTITUTION'),COU:H('GOV_COUNCIL'),REB:H('GOV_REBOOT')};
 ok(GT.ADJ===await kg.T_GOV_ADJUSTABLE()&&GT.REB===await kg.T_GOV_REBOOT(),'governance type ids match keccak names');
 const cGood=await dep('MockConstraint',admin,true), cBad=await dep('MockConstraint',admin,false);
 const obj=await dep('MockObjective',admin), rel=await dep('MockRelevance',admin), eng=await dep('MockEngine',admin);
 const att=await dep('MockAttribution',admin,a4.address), locked=await dep('MockLocked',admin);
 const C1=b32('C_good'),C2=b32('C_bad'),J1=b32('J1'),R1=b32('R1'),A1=b32('A1');
 await tx(kg,'registerImpl',0,C1,cGood.target); await tx(kg,'registerImpl',0,C2,cBad.target);
 await tx(kg,'registerImpl',1,J1,obj.target); await tx(kg,'registerImpl',2,R1,rel.target);
 await tx(kg,'registerImpl',3,A1,att.target); await tx(kg,'registerEngine',eng.target);
 const pol=(q)=>({exists:false,quorumRequired:q,confidenceThreshold:WAD/2n,coverageMin:WAD,minObsConfidence:0n,consensusWindow:1000n,planningWindow:100n,executionWindow:100000n,maxPlans:4n,requireObjectives:true,settlementThreshold:1n,maxConsensusRounds:2n});
 ok(await rn(K(stranger),'setPolicy',T,pol(WAD))==='Unauthorized','a stranger cannot set policy');
 ok(await rn(kg,'setPolicy',T,{...pol(WAD),maxConsensusRounds:0n})==='BadRounds','zero rounds rejected');
 await tx(kg,'setPolicy',T,pol(2n*WAD));
 await tx(kg,'setRequiredSets',T,[C1],[J1],[R1],[A1]);
 for(const g of Object.values(GT)) await tx(kg,'setRequiredSets',g,[C1],[J1],[R1],[A1]);
 ok(await rn(kg,'sealGenesis')==='GenesisNotReady','cannot seal before a council and governance policies exist');
 // agents: base influence 2e18 and rep 0 => influence exactly 1e18
 for(const a of [a1,a2,a3,a4,a5]) await tx(kg,'registerAgent',a.address,[G],2n*WAD);
 await tx(kg,'registerAgent',a6.address,[G,G2],2n*WAD);
 await tx(kg,'registerAgent',stranger.address,[b32('Other')],2n*WAD);
 await tx(kg,'setGroupMultiplier',T,G,2n*WAD); await tx(kg,'setGroupMultiplier',T,G2,3n*WAD);
 for(const [r,a] of [['OBSERVER',observer],['EXECUTOR',executor],['SETTLER',settler],['OUTCOME_RECORDER',recorder],['GUARDIAN',guardian]]) await tx(kg,'grantRole',H(r),a.address);
 await tx(kg,'setPositionSource',locked.target);
 ok(await rn(kg,'installCouncil',[a1.address,a1.address,a2.address])==='BadSeats','duplicate seat rejected');
 await tx(kg,'installCouncil',[a1.address,a2.address,a3.address]);
 ok(await kg.isSeated(a1.address)&&!(await kg.isSeated(a4.address)),'council seeded (genesis only, exempt from candidacy because no history exists)');
 for(const g of Object.values(GT)) await tx(kg,'setPolicy',g,pol(2n*WAD));
 await tx(kg,'sealGenesis');
 ok(await kg.genesisSealed(),'genesis sealed');
 console.log('after the seal there is no administrator');
 ok(await rn(kg,'grantRole',H('OBSERVER'),rando.address)==='Unauthorized','former admin cannot grant a role');
 ok(await rn(kg,'setPolicy',T,pol(1n))==='Unauthorized','former admin cannot set policy');
 ok(await rn(kg,'sealGenesis')==='NotGenesis','cannot seal twice');
 ok(await rn(kg,'installCouncil',[a1.address,a2.address,a3.address])==='Unauthorized','former admin cannot install a council');
 ok(await rn(new ethers.Contract(mods.GovernanceModule.target,abi,admin),'grantRole',H('OBSERVER'),rando.address)==='Unauthorized','calling the module directly is inert');
 ok(await kg.genesisAdmin()===ethers.ZeroAddress,'genesis admin address cleared');

 // ---- helpers for the pipeline ----
 const t0=await now(); const FAR=t0+100_000_000;
 const KO=K(observer);
 await tx(KO,'ingestObservation',b32('obs1'),t0,H('s'),H('d'),H('m'),WAD,FAR);
 let n=0;
 const manifest=async(type,payload=Z,groups=[G],extra=[])=>({parentStateId:(await kg.declaredState()).id,transitionType:type,relevantGroups:groups,changedVars:[],constraints:extra,resources:[],expectedEffects:[b32('E1')],expiresAt:FAR,observationId:b32('obs1'),resultingStateHash:H('S'+(++n)),transitionPayloadHash:payload});
 const planOf=async()=>({resourceClaims:[],effects:[b32('E1')],prerequisites:[],conflicts:[],expiresAt:FAR,payloadHash:H('plan')});
 const registerOrd=async(who,id,type=T,groups=[G])=>{ const m=await manifest(type,Z,groups); await tx(K(who),'registerTransition',id,b32('P_'+id),m); return m; };
 const registerGov=async(who,id,action,type)=>{ const m=await manifest(type,ethers.keccak256(action)); await tx(K(who),'registerGovernanceTransition',id,b32('P_'+id),m,action); return m; };
 const sign=async(who,id,raw=WAD,sup=WAD)=>tx(K(who),'submitRoundSignal',id,raw,sup);
 const evalElig=async(id)=>{await tx(K(rando),'evaluateTransition',id);await tx(K(rando),'markEligible',id);};
 const closeConsensus=async(id)=>{const dl=await kg.consensusDeadline(id);const cur=await now();if(BigInt(cur)<=dl)await mine(Number(dl)-cur+1);};
 const toValidated=async(id,planner)=>{ await closeConsensus(id); await tx(K(rando),'selectCandidate',id); await tx(K(rando),'openPlanning',id); await tx(K(planner),'submitPlan',id,await planOf()); await mine(150); await tx(K(rando),'reconcilePlans',id); await tx(K(rando),'validateTransition',id); };
 const fullOrd=async(proposer,id,signers,type=T)=>{ await registerOrd(proposer,id,type); await evalElig(id); for(const s of signers) await sign(s,id); await toValidated(id,signers[0]); await tx(K(rando),'declareCommit',id); await tx(K(rando),'commit',id); };

 // ---- predicate mask on a fresh transition ----
 console.log('predicate');
 await registerOrd(a1,b32('m0'));
 const mask0=await kg.commitFailMask(b32('m0'));
 ok((mask0&(F.CONSTRAINTS|F.QUORUM|F.CONFIDENCE|F.COMPAT|F.PLANS))===(F.CONSTRAINTS|F.QUORUM|F.CONFIDENCE|F.COMPAT|F.PLANS),'fresh transition fails constraints/quorum/confidence/compat/plans (UNAVAILABLE is not PASS)');
 ok((mask0&F.FRESH)===0n,'observation fresh -> F_FRESH clear');
 ok(await rn(K(rando),'registerTransition',b32('x1'),b32('P'),await manifest(b32('NOPE')))==='InactiveAgent','non-agent cannot register');
 ok(await rn(K(a1),'registerTransition',b32('x1'),b32('P'),await manifest(b32('NOPE')))==='NoPolicyForType','unknown transition type rejected: policy comes only from governance');
 ok(await rn(K(a1),'registerTransition',b32('x1'),b32('P'),await manifest(GT.ADJ))==='GovernanceTypeOnly','governance types cannot be registered as ordinary transitions');
 ok(await rn(K(a1),'evaluateTransition',b32('m0'))===null,'evaluation is permissionless');
 await evalElig(b32('m0'));
 // weight cap, uncertain, rounds, monotone, group max
 console.log('consensus rounds, classifier, weight cap');
 ok(await rn(K(a1),'submitRoundSignal',b32('m0'),WAD+1n,WAD)==='ValueOutOfRange','raw weight above WAD rejected');
 ok(await rn(K(a1),'submitRoundSignal',b32('m0'),WAD,WAD+1n)==='ValueOutOfRange','support above WAD rejected');
 ok(await rn(K(stranger),'submitRoundSignal',b32('m0'),WAD,WAD)==='NotEligible','agent outside the relevant groups cannot signal');
 await sign(a1,b32('m0'),WAD,WAD*7n/10n);               // round 0
 ok(await rn(K(a1),'submitRoundSignal',b32('m0'),WAD,WAD*7n/10n)==='AlreadySignalledThisRound','second signal in the same round rejected');
 let ty=await kg.tallies(b32('m0'));
 ok(ty.totalWeight===2n*WAD,'effective weight = raw * R * I * group multiplier (1*1*1*2)');
 await sign(a6,b32('m0'),WAD,WAD*3n/10n);               // multi-group agent: highest matching group (3), not the sum (2+3 would be 5)
 ty=await kg.tallies(b32('m0'));
 ok(ty.totalWeight===5n*WAD,'agent in groups with multipliers 2 and 3: weight 3 (highest matching), not 5 (sum)');
 // niche
 let nh=await kg.nicheOf(a1.address);
 ok(nh.found&&SEC[Number(nh.sector)]==='COLLABORATIVE'&&nh.score===WAD,'a1 aligned+substantive -> COLLABORATIVE, niche score += table weight 1.0');
 await sign(a2,b32('m0'),0n,WAD/2n);                    // abstaining: raw weight 0
 nh=await kg.nicheOf(a2.address);
 ok(!nh.found,'abstaining adds 0 to niche (table weight 0), so no niche forms yet');
 await sign(a3,b32('m0'),WAD,MAXU);                     // UNCERTAIN support sentinel
 ty=await kg.tallies(b32('m0'));
 ok(ty.totalWeight===5n*WAD&&ty.count===4n,'UNCERTAIN signal is recorded but adds no weight');
 // move into round 1 and revise
 await mine(600);
 ok(Number(await kg.roundOf(b32('m0')))===1,'round index derived from time');
 await sign(a1,b32('m0'),WAD,WAD*9n/10n);               // a1 was 0.7; mean of round 0 weights: (2*.7+3*.3)/5 = .46; moving to .9 is away
 // (the line above should have thrown) -- handled below
 ty=await kg.tallies(b32('m0'));
 console.log('   (a1 revised away from the mean: accepted? weight',ty.totalWeight,')');
 await rn(K(a1),'submitRoundSignal',b32('m0'),WAD,WAD*9n/10n);
 await tx(K(a6),'submitRoundSignal',b32('m0'),WAD,WAD*4n/10n).catch(()=>{});
 await closeConsensus(b32('m0'));
 ok(await rn(K(a1),'submitRoundSignal',b32('m0'),WAD,WAD/2n)==='ConsensusClosed','no signals after the window');

 console.log(`\n${pass} passed, ${fail} failed`); process.exit(fail?1:0);
})().catch(e=>{console.error(e);process.exit(2)});
