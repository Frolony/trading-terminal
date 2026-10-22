import { mkdir, readFile, writeFile } from 'node:fs/promises';
const source=await readFile('src/index.html','utf8');
const link='<a href="/raydium/" aria-label="Open Raydium swap" style="position:fixed;right:18px;bottom:18px;z-index:2147483000;border:1px solid #3b625c;border-radius:999px;padding:10px 15px;background:#13251f;color:#b8f36b;font:700 13px/1.2 system-ui,sans-serif;text-decoration:none;box-shadow:0 8px 28px #0008">Raydium Swap ↗</a>';
const enhanced=source.includes('href="/raydium/"')?source:(/<\/body\s*>/i.test(source)?source.replace(/<\/body\s*>/i,link+'\n</body>'):source+'\n'+link);
await mkdir('dist',{recursive:true});await writeFile('dist/index.html',enhanced,'utf8');console.log('Preserved Gecko Terminal home and ensured a single Raydium Swap link.');
