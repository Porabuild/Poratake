import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync } from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { PROFILE_TIMEOUT_MINUTES, runVisualProfile } from './visual-runner.mjs';

const root = path.resolve(import.meta.dirname, '../..');
const ALL_THEMES = ['light', 'dark'];
const ALL_WIDTHS = [950, 1280];
const ALL_SCALES = [1, 1.25, 1.5, 2];

function parseFilter(args, prefix, allowed, parse = value => value) {
  const values = [];
  for (const arg of args) {
    if (!arg.startsWith(prefix)) continue;
    for (const part of arg.slice(prefix.length).split(',')) {
      if (!part) continue;
      const parsed = parse(part);
      if (!allowed.includes(parsed)) {
        throw new Error(`Invalid value for ${prefix}: ${part}`);
      }
      if (!values.includes(parsed)) values.push(parsed);
    }
  }
  return values.length ? values : allowed;
}

const rawArgs = process.argv.slice(2);
const update = rawArgs.includes('--update-baselines');
const unknown = rawArgs.filter(
  arg =>
    arg !== '--update-baselines' &&
    !arg.startsWith('--theme=') &&
    !arg.startsWith('--width=') &&
    !arg.startsWith('--scale=')
);
if (unknown.length) throw new Error(`Unknown arguments: ${unknown.join(' ')}`);

const themes = parseFilter(rawArgs, '--theme=', ALL_THEMES);
const widths = parseFilter(rawArgs, '--width=', ALL_WIDTHS, Number);
const scales = parseFilter(rawArgs, '--scale=', ALL_SCALES, Number);
const totalProfiles = themes.length * widths.length * scales.length;

const build = spawnSync(
  'cargo',
  [
    'build',
    '--locked',
    '--manifest-path',
    'src/main/Cargo.toml',
    '--workspace',
    '--bin',
    'poratake-gpui',
    '--features',
    'poratake-gpui/parity',
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
let timedOutProfile = false;
let profilesRun = 0;
profileMatrix: for (const theme of themes) {
  for (const width of widths) {
    for (const scale of scales) {
      const name = `${theme}-${width}-${scale}`;
      const destination = path.join(output, name);
      mkdirSync(destination);
      console.log(`Checking ${name}`);
      const result = await runVisualProfile(artifact.executable, ['--parity'], {
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
      profilesRun += 1;
      if (result.error?.code === 'ETIMEDOUT') {
        failures += 1;
        timedOutProfile = true;
        const cleanup = result.cleanupError
          ? ` Process-tree cleanup failed: ${result.cleanupError.message}.`
          : '';
        console.error(
          `${name} timed out after ${PROFILE_TIMEOUT_MINUTES} minutes and was terminated.${cleanup} Partial visual evidence: ${destination}`
        );
        break profileMatrix;
      }
      if (result.error) throw result.error;
      if (result.status !== 0) failures += 1;
    }
  }
}
const summary = timedOutProfile
  ? `Stopped after a per-profile timeout. ${profilesRun}/${totalProfiles} visual cases ran; ${failures} failed.`
  : `${totalProfiles - failures}/${totalProfiles} visual cases passed.`;
console.log(`${summary} Evidence: ${output}`);
process.exit(failures ? 1 : 0);
