// Serves the Electron renderer WITHOUT the Electron plugin, so its pixels can be
// captured in an ordinary browser and compared against the GPUI shell.
//
//   bunx vite --config scripts/parity/vite.renderer.mts
//   open http://localhost:5599/index.html?window=settings#general
//
// The renderer needs the preload bridge, which a plain browser has not got;
// `preload-stub.js` in this directory stands in for it. Inject it as an
// on-new-document script before loading the page.
//
// Nothing in the app build references this file.
import { defineConfig } from 'vite';
import path from 'node:path';
import os from 'node:os';
import babel from '@rolldown/plugin-babel';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

const alias = {
  '@': path.resolve(import.meta.dirname, '../../src'),
  '@build': path.resolve(import.meta.dirname, '../../build'),
};

export default defineConfig({
  root: path.resolve(import.meta.dirname, '../..'),
  base: './',
  plugins: [
    react(),
    babel({ presets: [reactCompilerPreset()] }),
    tailwindcss(),
  ],
  resolve: { alias },
  server: {
    port: 5599,
    strictPort: true,
    // Without this the watcher picks up `cargo build` output and reloads the
    // page mid-comparison, which looks exactly like a rendering bug.
    watch: {
      ignored: ['**/target/**', '**/src/main/app-gpui/**', '**/dist*/**'],
    },
    // A comparison needs to feed the editor a capture. Allowing the temp
    // directory means `/@fs/<abs path>` can serve one from outside the project
    // instead of dropping a scratch image into the repository. Without this the
    // request quietly falls through to the SPA fallback and the page receives
    // `index.html` where it expected a PNG.
    fs: {
      allow: [path.resolve(import.meta.dirname, '../..'), os.tmpdir()],
    },
  },
});
