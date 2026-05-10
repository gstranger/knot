import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import wasm from 'vite-plugin-wasm';

// `BASE_URL` is set in CI by the GitHub Pages deploy workflow to
// match the repo subpath (e.g. `/knot/`). For local `pnpm dev` it's
// unset and Vite uses the root path. Setting it via env keeps the
// vite config repo-name-agnostic.
const base = process.env.BASE_URL ?? '/';

export default defineConfig({
  base,
  plugins: [react(), wasm()],
});
