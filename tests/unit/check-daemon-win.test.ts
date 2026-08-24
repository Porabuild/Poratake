import { describe, expect, it, vi } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const {
  DAEMON_WIN_MANIFEST,
  RUST_MANIFESTS,
  rustCheckCommands,
  runRustChecks,
} = await import('../../scripts/check-daemon-win.mjs');

function gpuiRustSources(): string[] {
  const root = path.join(process.cwd(), 'src', 'main', 'app-gpui', 'src');
  const files: string[] = [];
  const stack = [root];
  while (stack.length > 0) {
    const current = stack.pop();
    if (!current) {
      continue;
    }
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(full);
        continue;
      }
      if (entry.name.endsWith('.rs')) {
        files.push(full);
      }
    }
  }
  return files;
}

describe('Windows native rust check', () => {
  it('fmt-checks and clippy-checks both Rust crates', () => {
    expect(RUST_MANIFESTS).toEqual([
      'src/main/daemon-win/Cargo.toml',
      'src/main/app-gpui/Cargo.toml',
    ]);
    expect(DAEMON_WIN_MANIFEST).toBe('src/main/daemon-win/Cargo.toml');

    const commands = rustCheckCommands();
    const serialized = commands.map(
      step => `${step.command} ${step.args.join(' ')}`
    );

    for (const manifest of RUST_MANIFESTS) {
      expect(serialized).toContain(
        `cargo fmt --manifest-path ${manifest} -- --check`
      );
      expect(serialized).toContain(
        `cargo clippy --manifest-path ${manifest} --all-targets -- -D clippy::disallowed_methods`
      );
    }

    for (const manifest of RUST_MANIFESTS) {
      expect(serialized).toContain(
        `cargo test --manifest-path ${manifest} --no-fail-fast`
      );
    }
    expect(
      serialized.filter(command => command.startsWith('cargo test ')).length
    ).toBe(RUST_MANIFESTS.length);
  });

  it('skips off Windows and when cargo is missing, and stops on fmt failure', () => {
    const spawn = vi.fn();
    expect(runRustChecks({ platform: 'darwin', spawn })).toBe(0);
    expect(spawn).not.toHaveBeenCalled();

    spawn.mockReset();
    spawn.mockReturnValue({ status: 1 });
    expect(runRustChecks({ platform: 'win32', spawn })).toBe(0);
    expect(spawn).toHaveBeenCalledTimes(1);
    expect(spawn.mock.calls[0][1]).toEqual(['--version']);

    spawn.mockReset();
    spawn.mockImplementation((command: string, args: string[]) => {
      if (
        command === 'cargo' &&
        args[0] === 'fmt' &&
        args.includes('--check')
      ) {
        return { status: 1 };
      }
      return { status: 0 };
    });
    expect(runRustChecks({ platform: 'win32', spawn })).toBe(1);
    const cargoChecks = spawn.mock.calls
      .filter(
        call =>
          call[0] === 'cargo' &&
          call[1][0] !== '--version' &&
          call[1][1] !== '--version'
      )
      .map(call => call[1][0]);
    expect(cargoChecks).toEqual(['fmt']);
  });

  it('bans smol::Timer::after in Clippy config and GPUI sources', () => {
    const clippy = fs.readFileSync(
      path.join(process.cwd(), 'clippy.toml'),
      'utf8'
    );
    expect(clippy).toContain('smol::Timer::after');
    expect(clippy).toContain('gpui::BackgroundExecutor::timer');

    const sources = gpuiRustSources();
    expect(sources.length).toBeGreaterThan(0);
    for (const file of sources) {
      expect(fs.readFileSync(file, 'utf8')).not.toContain('smol::Timer::after');
    }
  });

  it('lists every direct GPUI runtime dependency in third-party notices', () => {
    const manifest = fs.readFileSync(
      path.join(process.cwd(), 'src', 'main', 'app-gpui', 'Cargo.toml'),
      'utf8'
    );
    const dependencies = manifest
      .split('[dependencies]')[1]
      .split('[dev-dependencies]')[0]
      .split('\n')
      .flatMap(line => {
        const match = line.match(/^([a-z0-9_-]+)\s*=/);
        return match ? [match[1]] : [];
      });
    const notices = fs.readFileSync(
      path.join(process.cwd(), 'THIRD_PARTY_NOTICES.md'),
      'utf8'
    );

    for (const dependency of dependencies) {
      expect(notices).toContain(`\`${dependency}\``);
    }
  });
});
