const solc=require('solc'),fs=require('fs');
const dir='/mnt/user-data/outputs/coordination-kernel/contracts/';
const files=fs.readdirSync(dir).filter(f=>f.endsWith('.sol'));
const sources={};files.forEach(f=>sources[f]={content:fs.readFileSync(dir+f,'utf8')});
sources['Mocks.sol']={content:fs.readFileSync(__dirname+'/Mocks.sol','utf8')};
const input={language:'Solidity',sources,settings:{evmVersion:'paris',optimizer:{enabled:true,runs:1},outputSelection:{'*':{'*':['abi','evm.deployedBytecode.object','evm.bytecode.object']}}}};
const out=JSON.parse(solc.compile(JSON.stringify(input)));
let bad=0;(out.errors||[]).forEach(e=>{if(e.severity==='error'){bad++;console.log(e.formattedMessage)}else if(process.env.W)console.log(e.formattedMessage.split('\n')[0])});
const art={};
for(const f of Object.keys(out.contracts||{}))for(const [n,c] of Object.entries(out.contracts[f])){art[n]={abi:c.abi,bin:'0x'+c.evm.bytecode.object};
 const sz=c.evm.deployedBytecode.object.length/2; if(sz>0) console.log(n.padEnd(22),sz,sz>24576?'OVER LIMIT':'');}
fs.writeFileSync(__dirname+'/art.json',JSON.stringify(art));console.log(bad?'ERRORS':'built');
