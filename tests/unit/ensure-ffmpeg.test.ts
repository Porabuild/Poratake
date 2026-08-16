import {
  afterAll,
  afterEach,
  beforeAll,
  describe,
  expect,
  it,
  vi,
} from 'vitest';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const { execFileSync } = vi.hoisted(() => ({
  execFileSync: vi.fn(),
}));

vi.mock('node:child_process', () => ({ execFileSync }));

const { ensureFFmpeg, getFFmpegOutputPath, isFFmpegBuilt } =
  await import('../../scripts/ensure-ffmpeg.mjs');

const roots: string[] = [];
const originalPlatform = process.platform;

beforeAll(() => {
  Object.defineProperty(process, 'platform', { value: 'darwin' });
});

afterAll(() => {
  Object.defineProperty(process, 'platform', { value: originalPlatform });
});

function createRoot(withBinary = false): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'poratake-ffmpeg-'));
  roots.push(root);
  if (withBinary) {
    fs.mkdirSync(path.dirname(getFFmpegOutputPath(root)), {
      recursive: true,
    });
    fs.writeFileSync(getFFmpegOutputPath(root), '');
  }
  return root;
}

type CommandInvocation = [string, string[]];

function recordedCommands(): CommandInvocation[] {
  return vi
    .mocked(execFileSync)
    .mock.calls.map(call => [call[0], call[1] ?? []]);
}

function mockEnvironment(root: string, commands: string[]): void {
  const available = new Set(commands);

  vi.mocked(execFileSync).mockImplementation(((
    command: string,
    args: string[]
  ) => {
    if (command === 'sh') {
      const target = args[1].split(' ').pop();
      if (!available.has(target)) throw new Error('not found');
      return '';
    }
    if (command === 'brew') {
      for (const name of args.slice(1)) {
        available.add(name);
      }
      return '';
    }
    if (command === 'bash') {
      fs.mkdirSync(path.dirname(getFFmpegOutputPath(root)), {
        recursive: true,
      });
      fs.writeFileSync(getFFmpegOutputPath(root), '');
      return undefined;
    }
    throw new Error(`unexpected command: ${command}`);
  }) as typeof execFileSync);
}

afterEach(() => {
  vi.mocked(execFileSync).mockReset();
  for (const root of roots.splice(0)) {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

describe('ensure-ffmpeg', () => {
  it('skips the build when the binary already exists', () => {
    const root = createRoot(true);

    expect(ensureFFmpeg(root)).toBe(true);
    expect(isFFmpegBuilt(root)).toBe(true);
    expect(execFileSync).not.toHaveBeenCalled();
  });

  it('builds via the pinned script when the binary is missing', () => {
    const root = createRoot(false);
    mockEnvironment(root, ['nasm', 'pkg-config']);

    expect(ensureFFmpeg(root)).toBe(true);
    expect(recordedCommands()).toEqual([
      ['sh', ['-c', 'command -v nasm']],
      ['sh', ['-c', 'command -v pkg-config']],
      ['bash', ['scripts/build-ffmpeg.sh']],
    ]);
    expect(isFFmpegBuilt(root)).toBe(true);
  });

  it('does not install missing prerequisites without user approval', () => {
    const root = createRoot(false);
    mockEnvironment(root, ['brew', 'pkg-config']);

    expect(ensureFFmpeg(root)).toBe(false);
    expect(recordedCommands()).toEqual([
      ['sh', ['-c', 'command -v nasm']],
      ['sh', ['-c', 'command -v pkg-config']],
    ]);
    expect(isFFmpegBuilt(root)).toBe(false);
  });

  it('returns false when build prerequisites are unavailable', () => {
    const root = createRoot(false);
    mockEnvironment(root, []);

    expect(ensureFFmpeg(root)).toBe(false);
    expect(isFFmpegBuilt(root)).toBe(false);
    expect(
      recordedCommands().some(([, args]) => args.includes('install'))
    ).toBe(false);
  });

  it('returns false when the build fails', () => {
    const root = createRoot(false);
    mockEnvironment(root, ['nasm', 'pkg-config']);
    vi.mocked(execFileSync).mockImplementation(((command: string) => {
      if (command === 'bash') throw new Error('build error');
      return '';
    }) as typeof execFileSync);

    expect(ensureFFmpeg(root)).toBe(false);
    expect(isFFmpegBuilt(root)).toBe(false);
  });

  it('returns false when the build succeeds without producing the binary', () => {
    const root = createRoot(false);
    mockEnvironment(root, ['nasm', 'pkg-config']);
    vi.mocked(execFileSync).mockImplementation(
      (() => undefined) as typeof execFileSync
    );

    expect(ensureFFmpeg(root)).toBe(false);
    expect(isFFmpegBuilt(root)).toBe(false);
  });
});
