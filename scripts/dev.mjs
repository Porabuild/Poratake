import { execSync, spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  claimDevLock,
  clearDevLock,
  stopProcessTree,
  stopRunningDev,
  writeDevLock,
} from './dev-lock.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const env = { ...process.env };
delete env.ELECTRON_RUN_AS_NODE;

let stoppedPreviousSession = false;
while (!claimDevLock(root)) {
  stoppedPreviousSession =
    (await stopRunningDev(root)) || stoppedPreviousSession;
  await new Promise(resolve => setTimeout(resolve, 50));
}

if (stoppedPreviousSession) {
  console.log('[dev] previous dev session terminated');
}

if (process.platform === 'win32') {
  execSync(
    'powershell -ExecutionPolicy Bypass -File scripts/build-daemon-win.ps1',
    { stdio: 'inherit', cwd: root, env }
  );
} else if (process.platform === 'darwin') {
  execSync('node scripts/build-daemon-dev.mjs', {
    stdio: 'inherit',
    cwd: root,
    env,
  });
}

const require = createRequire(import.meta.url);
const viteBin = path.join(
  path.dirname(require.resolve('vite/package.json')),
  require('vite/package.json').bin.vite
);
const child = spawn(process.execPath, [viteBin], {
  stdio: 'inherit',
  cwd: root,
  env,
  detached: process.platform !== 'win32',
});

writeDevLock(root, child.pid);

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => stopProcessTree(child.pid, signal));
}

process.on('exit', () => clearDevLock(root));

child.on('exit', code => {
  clearDevLock(root);
  process.exit(code ?? 1);
});
