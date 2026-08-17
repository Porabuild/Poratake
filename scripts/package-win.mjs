import { spawnSync } from 'node:child_process';

const allowed = new Set(['x64', 'arm64']);
const hostArch = process.arch === 'arm64' ? 'arm64' : 'x64';
const arch = process.env.PORATAKE_WIN_ARCH || hostArch;

if (!allowed.has(arch)) {
  process.stderr.write(`Unsupported PORATAKE_WIN_ARCH: ${arch}\n`);
  process.exit(1);
}

const result = spawnSync(
  'electron-builder',
  ['--win', `--${arch}`, '-p', 'never'],
  { stdio: 'inherit', shell: true }
);

process.exit(result.status ?? 1);
