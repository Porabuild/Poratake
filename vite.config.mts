import { defineConfig } from 'vite';
import path from 'node:path';
import electron from 'vite-plugin-electron/simple';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

const alias = {
  '@': path.resolve(__dirname, './src'),
  '@build': path.resolve(__dirname, './build'),
};

// https://vitejs.dev/config/
export default defineConfig({
  base: './',
  plugins: [
    react(),
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
        input: path.join(__dirname, 'src/preload/preload.ts'),
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
        main: path.resolve(__dirname, 'index.html'),
        history: path.resolve(__dirname, 'history.html'),
      },
    },
  },
});
