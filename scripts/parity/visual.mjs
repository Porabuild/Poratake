import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync } from 'node:fs';
import path from 'node:path';
import os from 'node:os';

const root = path.resolve(import.meta.dirname, '../..');
const update = process.argv.includes('--update-baselines');
const unknown = process.argv
  .slice(2)
  .filter(arg => arg !== '--update-baselines');
if (unknown.length) throw new Error(`Unknown arguments: ${unknown.join(' ')}`);
const build = spawnSync(
  'cargo',
  [
    'build',
    '--locked',
    '--manifest-path',
    'src/main/Cargo.toml',
    '-p',
    'poratake-gpui',
    '--features',
    'parity',
    '--message-format=json-render-diagnostics',
  ],
  { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] }
);
if (build.error) throw build.error;
if (build.status !== 0) process.exit(build.status ?? 1);
const artifact = build.stdout
  .trim()
  .split('\n')
  .map(line => JSON.parse(line))
  .find(
    message =>
      message.reason === 'compiler-artifact' &&
      message.target.name === 'poratake-gpui' &&
      message.executable
  );
if (!artifact)
  throw new Error('Cargo did not report the visual harness executable');
const output = mkdtempSync(
  path.join(process.env.RUNNER_TEMP || os.tmpdir(), 'poratake-visual-')
);
console.log(`Visual evidence: ${output}`);
let failures = 0;
for (const theme of ['light', 'dark']) {
  for (const width of [950, 1280]) {
    for (const scale of [1, 1.25, 1.5, 2]) {
      const name = `${theme}-${width}-${scale}`;
      const destination = path.join(output, name);
      mkdirSync(destination);
      console.log(`Checking ${name}`);
      const result = spawnSync(artifact.executable, ['--parity'], {
        cwd: root,
        stdio: 'inherit',
        env: {
          ...process.env,
          PORATAKE_PARITY_OUTPUT: destination,
          PORATAKE_PARITY_THEME: theme,
          PORATAKE_PARITY_WIDTH: String(width),
          PORATAKE_PARITY_SCALE: String(scale),
          PORATAKE_PARITY_IMAGE: '',
          PORATAKE_PARITY_BASELINES: path.join(
            root,
            'scripts/parity/baselines',
            process.platform,
            name
          ),
          PORATAKE_PARITY_UPDATE_BASELINES: update ? '1' : '0',
        },
      });
      if (result.error) throw result.error;
      if (result.status !== 0) failures += 1;
    }
  }
}
console.log(`${16 - failures}/16 visual cases passed. Evidence: ${output}`);
process.exit(failures ? 1 : 0);
