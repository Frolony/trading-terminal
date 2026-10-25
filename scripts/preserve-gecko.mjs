import { mkdir, readFile, writeFile } from 'node:fs/promises';
// N5.13 (2026-10-24, A58): the build step copies the home page verbatim.
// It used to inject a floating "Raydium Swap" pill pinned to the bottom-right
// of the page; that pill is gone for good — the swap door is the header
// button (#swapHeaderBtn) in src/index.html. Nothing here may re-add a
// floating swap link: if the header button is ever removed, add its
// replacement to the header in the source, never as a build-time injection.
const source = await readFile('src/index.html', 'utf8');
await mkdir('dist', { recursive: true });
await writeFile('dist/index.html', source, 'utf8');
console.log('Preserved Turnpool home (header carries the Raydium Swap link).');
