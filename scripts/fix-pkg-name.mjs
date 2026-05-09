// wasm-pack generates pkg/package.json with "name": "knot".
// Rename to "knot-wasm" so the pnpm workspace link resolves.
import { readFileSync, writeFileSync } from 'fs';

const path = new URL('../pkg/package.json', import.meta.url).pathname;
const pkg = JSON.parse(readFileSync(path, 'utf8'));
pkg.name = 'knot-wasm';
writeFileSync(path, JSON.stringify(pkg, null, 2) + '\n');
console.log('pkg/package.json name set to "knot-wasm"');
