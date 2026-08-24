import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import path from 'node:path';
import { runDevSession } from './dev-session.mjs';

const require = createRequire(import.meta.url);
const viteBin = path.join(
  path.dirname(require.resolve('vite/package.json')),
  require('vite/package.json').bin.vite
);

await runDevSession('dev', ({ root, env }) =>
  spawn(process.execPath, [viteBin], {
    stdio: 'inherit',
    cwd: root,
    env,
    detached: process.platform !== 'win32',
  })
);
