import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const REPO_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..'
);

export const RUST_WORKSPACE_MANIFEST = 'src/main/Cargo.toml';

const FMT_COMMAND = {
  command: 'cargo',
  args: [
    'fmt',
    '--manifest-path',
    RUST_WORKSPACE_MANIFEST,
    '--all',
    '--',
    '--check',
  ],
};

const CLIPPY_COMMAND = {
  command: 'cargo',
  args: [
    'clippy',
    '--manifest-path',
    RUST_WORKSPACE_MANIFEST,
    '--workspace',
    '--all-targets',
    '--',
    '-D',
    'warnings',
  ],
};

const DENY_COMMAND = {
  command: 'cargo',
  args: ['deny', '--manifest-path', RUST_WORKSPACE_MANIFEST, 'check'],
};

const NEXTEST_COMMAND = {
  command: 'cargo',
  args: [
    'nextest',
    'run',
    '--manifest-path',
    RUST_WORKSPACE_MANIFEST,
    '--workspace',
    '--no-fail-fast',
  ],
};

const TEST_COMMAND = {
  command: 'cargo',
  args: [
    'test',
    '--manifest-path',
    RUST_WORKSPACE_MANIFEST,
    '--workspace',
    '--no-fail-fast',
  ],
};

export function rustCheckCommands({ nextest = false, deny = false } = {}) {
  return [
    FMT_COMMAND,
    CLIPPY_COMMAND,
    ...(deny ? [DENY_COMMAND] : []),
    nextest ? NEXTEST_COMMAND : TEST_COMMAND,
  ];
}

function cargoAvailable(spawn) {
  return (
    spawn('cargo', ['--version'], { stdio: 'ignore', shell: true }).status === 0
  );
}

function cargoSubcommandAvailable(spawn, name) {
  return (
    spawn('cargo', [name, '--version'], { stdio: 'ignore', shell: true })
      .status === 0
  );
}

function ensureRustfmtAndClippy(spawn) {
  if (
    cargoSubcommandAvailable(spawn, 'fmt') &&
    cargoSubcommandAvailable(spawn, 'clippy')
  ) {
    return;
  }
  spawn('rustup', ['component', 'add', 'rustfmt', 'clippy'], {
    stdio: 'inherit',
    shell: true,
  });
}

export function runRustChecks({
  platform = process.platform,
  spawn = spawnSync,
  ci = process.env.CI,
} = {}) {
  if (platform !== 'win32') {
    console.log('Skipping Windows native check: not running on Windows.');
    return 0;
  }

  if (!cargoAvailable(spawn)) {
    if (ci) {
      console.error('cargo is required in CI.');
      return 1;
    }
    console.log('Skipping Windows native check: cargo is not installed.');
    return 0;
  }

  ensureRustfmtAndClippy(spawn);

  const nextest = cargoSubcommandAvailable(spawn, 'nextest');
  const deny = cargoSubcommandAvailable(spawn, 'deny');
  if (!deny) {
    if (ci) {
      console.error(
        'cargo-deny is required in CI: the license and advisory gate cannot be skipped.'
      );
      return 1;
    }
    console.log('Skipping cargo-deny check: cargo-deny is not installed.');
  }

  for (const step of rustCheckCommands({ nextest, deny })) {
    const result = spawn(step.command, step.args, {
      stdio: 'inherit',
      shell: true,
      cwd: REPO_ROOT,
    });
    if ((result.status ?? 1) !== 0) {
      return result.status ?? 1;
    }
  }

  return 0;
}

const invokedDirectly =
  Boolean(import.meta.main) ||
  (process.argv[1] !== undefined &&
    path.normalize(fileURLToPath(import.meta.url)) ===
      path.normalize(path.resolve(process.argv[1])));

if (invokedDirectly) {
  process.exit(runRustChecks());
}
