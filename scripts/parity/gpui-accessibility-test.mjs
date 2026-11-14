import { execFileSync } from 'node:child_process';
import {
  cpSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../..'
);
const host = execFileSync('rustc', ['-vV'], { encoding: 'utf8' })
  .split('\n')
  .find(line => line.startsWith('host: '))
  ?.slice(6)
  .trim();
if (!host) throw new Error('rustc did not report a host target');

const metadata = JSON.parse(
  execFileSync(
    'cargo',
    [
      'metadata',
      '--offline',
      '--locked',
      '--manifest-path',
      path.join(root, 'src/main/Cargo.toml'),
      '--format-version',
      '1',
      '--filter-platform',
      host,
    ],
    { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }
  )
);
const gpui = metadata.packages.find(
  pkg => pkg.name === 'gpui-pre' && pkg.version === '0.3.11'
);
if (!gpui) throw new Error('the GPUI test adapter requires gpui-pre 0.3.11');

const tempRoot = realpathSync(tmpdir());
const tempDir = realpathSync(
  mkdtempSync(path.join(tempRoot, 'poratake-a11y-'))
);
const gpuiDir = path.join(tempDir, 'gpui-pre');
const tomlPath = value =>
  JSON.stringify(path.resolve(value).replaceAll('\\', '/'));
if (
  path.dirname(tempDir) !== tempRoot ||
  !path.basename(tempDir).startsWith('poratake-a11y-')
) {
  throw new Error(
    'refusing to remove an unexpected accessibility test directory'
  );
}

try {
  cpSync(path.dirname(gpui.manifest_path), gpuiDir, { recursive: true });
  execFileSync(
    'git',
    ['apply', path.join(root, 'scripts/parity/gpui-accessibility-test.patch')],
    {
      cwd: gpuiDir,
      stdio: 'inherit',
    }
  );
  writeFileSync(
    path.join(tempDir, 'Cargo.toml'),
    `[package]
name = "poratake-a11y-probe"
version = "0.0.0"
edition = "2024"

[dependencies]
gpui = { package = "gpui-pre", version = "=0.3.11", features = ["test-support"] }
herogpui-components = "=0.15.1"
herogpui-theme = "=0.15.1"

[patch.crates-io]
gpui-pre = { path = ${tomlPath(gpuiDir)} }
herogpui-components = { path = ${tomlPath(path.join(root, 'vendor/herogpui-components'))} }

[[test]]
name = "gpui-accessibility"
path = ${tomlPath(path.join(root, 'scripts/parity/gpui-accessibility-test.rs'))}

[profile.dev]
opt-level = 1
debug = "line-tables-only"

[profile.dev.package."*"]
opt-level = 3
debug = false
`
  );
  writeFileSync(
    path.join(tempDir, 'Cargo.lock'),
    readFileSync(path.join(root, 'src/main/Cargo.lock'))
  );
  execFileSync(
    'cargo',
    [
      'test',
      '--offline',
      '--manifest-path',
      path.join(tempDir, 'Cargo.toml'),
      '--test',
      'gpui-accessibility',
    ],
    {
      stdio: 'inherit',
      env: {
        ...process.env,
        CARGO_TARGET_DIR:
          process.env.CARGO_TARGET_DIR ?? path.join(root, 'src/main/target'),
      },
    }
  );
} finally {
  rmSync(tempDir, { recursive: true, force: true });
}
