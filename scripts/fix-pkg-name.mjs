// wasm-pack regenerates `pkg/package.json` on every build with only the
// fields it knows about ("name", "version", "files", etc.). This script
// runs after `wasm-pack build` and merges in the publish metadata that
// npm expects (license, repository, homepage, bugs, author, keywords)
// plus copies LICENSE / README into the package directory so they ship
// inside the tarball.
//
// It also renames "knot" → "knot-wasm" because the workspace's other
// package (`knot-cad`) takes a `workspace:*` dep on that name, and
// because the unscoped name `knot` is already taken on npm.

import { readFileSync, writeFileSync, copyFileSync, existsSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(__dirname, '..');
const pkgDir = join(repoRoot, 'pkg');
const pkgJsonPath = join(pkgDir, 'package.json');

const pkg = JSON.parse(readFileSync(pkgJsonPath, 'utf8'));

const merged = {
  ...pkg,
  name: 'knot-wasm',
  description: 'NURBS-based CAD kernel — WASM build (low-level handle layer)',
  license: 'MIT',
  author: 'Garrett Stranger',
  homepage: 'https://github.com/gstranger/knot',
  repository: {
    type: 'git',
    url: 'git+https://github.com/gstranger/knot.git',
  },
  bugs: { url: 'https://github.com/gstranger/knot/issues' },
  keywords: [
    'cad', 'nurbs', 'brep', 'wasm', 'rust',
    'kernel', 'boolean', 'mesh', 'step', 'geometry',
  ],
};

// Make sure LICENSE / README ship inside the tarball.
merged.files = [
  'knot_bg.wasm',
  'knot.js',
  'knot.d.ts',
  'README.md',
  'LICENSE',
];

writeFileSync(pkgJsonPath, JSON.stringify(merged, null, 2) + '\n');

// Mirror LICENSE + README into pkg/ so they're physically inside the
// directory that gets tarred up by `npm publish`.
const licenseSrc = join(repoRoot, 'LICENSE');
const licenseDst = join(pkgDir, 'LICENSE');
if (existsSync(licenseSrc)) copyFileSync(licenseSrc, licenseDst);

// wasm-pack overwrites pkg/README.md with the workspace README on
// every build. Replace it with the focused npm-landing version
// kept under scripts/, so the page on npmjs.com is scoped to the
// wasm package rather than the whole monorepo.
const readmeSrc = join(__dirname, 'wasm-readme.md');
const readmeDst = join(pkgDir, 'README.md');
if (existsSync(readmeSrc)) copyFileSync(readmeSrc, readmeDst);

console.log('pkg/package.json patched: name=knot-wasm + publish metadata');
