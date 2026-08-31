import { defineConfig } from 'vitest/config';
import path from 'path';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    setupFiles: ['tests/setup.ts'],
    include: ['tests/**/*.test.ts', 'tests/**/*.spec.ts'],
    onUnhandledError(error) {
      if (
        error.message.includes(
          'Closing rpc while "onUserConsoleLog" was pending'
        )
      ) {
        return false;
      }
    },
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      include: ['src/main/**/*.ts'],
      exclude: [
        'src/main/**/*.d.ts',
        'src/main/**/*.test.ts',
        'src/main/**/*.spec.ts',
        'src/main/binaries/**',
      ],
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
});
