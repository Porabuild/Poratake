import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const REPO_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..'
);

export const RUST_MANIFESTS = [
  'src/main/daemon-win/Cargo.toml',
  'src/main/app-gpui/Cargo.toml',
];

export const DAEMON_WIN_MANIFEST = RUST_MANIFESTS[0];

export function rustCheckCommands() {
  return [
    ...RUST_MANIFESTS.flatMap(manifest => [
      {
        command: 'cargo',
        args: ['fmt', '--manifest-path', manifest, '--', '--check'],
      },
      {
        command: 'cargo',
        args: [
          'clippy',
          '--manifest-path',
          manifest,
          '--all-targets',
          '--',
          '-D',
          'clippy::disallowed_methods',
        ],
      },
    ]),
    ...RUST_MANIFESTS.map(manifest => ({
      command: 'cargo',
      args: ['test', '--manifest-path', manifest, '--no-fail-fast'],
    })),
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
} = {}) {
  if (platform !== 'win32') {
    console.log('Skipping Windows native check: not running on Windows.');
    return 0;
  }

  if (!cargoAvailable(spawn)) {
    console.log('Skipping Windows native check: cargo is not installed.');
    return 0;
  }

  ensureRustfmtAndClippy(spawn);

  for (const step of rustCheckCommands()) {
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
