import { defineConfig } from 'vite';
import path from 'node:path';
import babel from '@rolldown/plugin-babel';
import electron from 'vite-plugin-electron/simple';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

const alias = {
  '@': path.resolve(import.meta.dirname, './src'),
  '@build': path.resolve(import.meta.dirname, './build'),
};

// https://vitejs.dev/config/
export default defineConfig({
  base: './',
  plugins: [
    react(),
    babel({ presets: [reactCompilerPreset()] }),
    tailwindcss(),
    electron({
      main: {
        // Shortcut of `build.lib.entry`.
        entry: 'src/main/main.ts',
        onstart({ startup }) {
          const env = { ...process.env };
          delete env.ELECTRON_RUN_AS_NODE;
          startup(['.', '--no-sandbox'], { env });
        },
        vite: {
          resolve: { alias },
        },
      },
      preload: {
        // Shortcut of `build.rolldownOptions.input`.
        // Preload scripts may contain Web assets, so use the `build.rolldownOptions.input` instead `build.lib.entry`.
        input: path.join(import.meta.dirname, 'src/preload/preload.ts'),
        vite: {
          resolve: { alias },
        },
      },
    }),
  ],
  resolve: {
    alias,
  },
  build: {
    rolldownOptions: {
      input: {
        main: path.resolve(import.meta.dirname, 'index.html'),
        history: path.resolve(import.meta.dirname, 'history.html'),
      },
    },
  },
});
