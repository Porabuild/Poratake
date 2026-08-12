import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const manifest = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  'src',
  'main',
  'daemon-win',
  'Cargo.toml'
);

if (process.platform !== 'win32') {
  console.log('Skipping Windows daemon check: not running on Windows.');
  process.exit(0);
}

const cargo = spawnSync('cargo', ['--version'], {
  stdio: 'ignore',
  shell: true,
});

if (cargo.status !== 0) {
  console.log('Skipping Windows daemon check: cargo is not installed.');
  process.exit(0);
}

const result = spawnSync(
  'cargo',
  ['test', '--manifest-path', manifest, '--no-fail-fast'],
  { stdio: 'inherit', shell: true }
);

process.exit(result.status ?? 1);
