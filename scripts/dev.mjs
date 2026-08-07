import { execSync, spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const env = { ...process.env };
delete env.ELECTRON_RUN_AS_NODE;

if (process.platform === 'win32') {
  execSync(
    'powershell -ExecutionPolicy Bypass -File scripts/build-daemon-win.ps1',
    { stdio: 'inherit', cwd: root, env }
  );
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
});

child.on('exit', code => {
  process.exit(code ?? 1);
});
