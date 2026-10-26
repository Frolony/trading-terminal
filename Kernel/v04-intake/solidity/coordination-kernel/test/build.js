const solc=require('solc'),fs=require('fs'),path=require('path');
const dir=path.join(__dirname,'..','contracts')+'/';
const files=fs.readdirSync(dir).filter(f=>f.endsWith('.sol'));
const sources={};files.forEach(f=>sources[f]={content:fs.readFileSync(dir+f,'utf8')});
sources['Mocks.sol']={content:fs.readFileSync(path.join(__dirname,'Mocks.sol'),'utf8')};
const input={language:'Solidity',sources,settings:{evmVersion:'paris',optimizer:{enabled:true,runs:1},outputSelection:{'*':{'*':['abi','evm.deployedBytecode.object','evm.bytecode.object']}}}};
const out=JSON.parse(solc.compile(JSON.stringify(input)));
let bad=false;(out.errors||[]).forEach(e=>{if(e.severity==='error'){bad=true;console.log(e.formattedMessage)}else if(!/SPDX|Unused|shadow/.test(e.message))console.log('warn:',e.formattedMessage.split('\n')[0])});
if(bad)process.exit(1);
const art={};for(const f of Object.keys(out.contracts))for(const [n,c] of Object.entries(out.contracts[f])){art[n]={abi:c.abi,bin:'0x'+c.evm.bytecode.object};const sz=c.evm.deployedBytecode.object.length/2;if(sz>0&&!f.startsWith('Mocks')&&!f.startsWith('I')&&n!=='KernelStorage')console.log(n.padEnd(20),sz,sz>24576?'OVER EIP-170':'ok');}
fs.writeFileSync(path.join(__dirname,'art.json'),JSON.stringify(art));console.log('built');
