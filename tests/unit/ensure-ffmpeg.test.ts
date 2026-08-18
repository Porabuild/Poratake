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

const {
  ensureFFmpeg,
  getFFmpegBuildIdentity,
  getFFmpegBuildStampPath,
  getFFmpegOutputPath,
  isFFmpegBuilt,
} = await import('../../scripts/ensure-ffmpeg.mjs');

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
  fs.mkdirSync(path.join(root, 'scripts'), { recursive: true });
  fs.writeFileSync(path.join(root, 'scripts', 'build-ffmpeg.sh'), 'build');
  if (withBinary) {
    fs.mkdirSync(path.dirname(getFFmpegOutputPath(root)), {
      recursive: true,
    });
    fs.writeFileSync(getFFmpegOutputPath(root), '');
    fs.writeFileSync(
      getFFmpegBuildStampPath(root),
      `${getFFmpegBuildIdentity(root)}\n`
    );
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
      fs.writeFileSync(
        getFFmpegBuildStampPath(root),
        `${getFFmpegBuildIdentity(root)}\n`
      );
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

  it('rebuilds when the binary has no build stamp', () => {
    const root = createRoot(true);
    fs.rmSync(getFFmpegBuildStampPath(root));
    mockEnvironment(root, ['nasm', 'pkg-config']);

    expect(ensureFFmpeg(root)).toBe(true);
    expect(recordedCommands()).toContainEqual([
      'bash',
      ['scripts/build-ffmpeg.sh'],
    ]);
  });

  it('rebuilds when the build script changed after the binary was built', () => {
    const root = createRoot(true);
    fs.writeFileSync(path.join(root, 'scripts', 'build-ffmpeg.sh'), 'updated');
    mockEnvironment(root, ['nasm', 'pkg-config']);

    expect(ensureFFmpeg(root)).toBe(true);
    expect(recordedCommands()).toContainEqual([
      'bash',
      ['scripts/build-ffmpeg.sh'],
    ]);
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

  it('preserves the previous binary when a stale build fails', () => {
    const root = createRoot(true);
    fs.writeFileSync(getFFmpegOutputPath(root), 'previous');
    const previousStamp = fs.readFileSync(
      getFFmpegBuildStampPath(root),
      'utf8'
    );
    fs.writeFileSync(path.join(root, 'scripts', 'build-ffmpeg.sh'), 'updated');
    mockEnvironment(root, ['nasm', 'pkg-config']);
    const implementation = vi.mocked(execFileSync).getMockImplementation();
    vi.mocked(execFileSync).mockImplementation(((
      command: string,
      args: string[]
    ) => {
      if (command === 'bash') throw new Error('build error');
      return implementation?.(command, args);
    }) as typeof execFileSync);

    expect(ensureFFmpeg(root)).toBe(false);
    expect(fs.readFileSync(getFFmpegOutputPath(root), 'utf8')).toBe('previous');
    expect(fs.readFileSync(getFFmpegBuildStampPath(root), 'utf8')).toBe(
      previousStamp
    );
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
