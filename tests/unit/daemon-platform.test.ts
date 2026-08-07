import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { EventEmitter } from 'events';

const mockSpawn = vi.fn();
const mockExecFileSync = vi.fn();
const mockExecSync = vi.fn();

vi.mock('child_process', () => ({
  spawn: (...a: unknown[]) => mockSpawn(...a),
  execFileSync: (...a: unknown[]) => mockExecFileSync(...a),
  execSync: (...a: unknown[]) => mockExecSync(...a),
}));

vi.mock('@/main/utils/paths', () => ({
  getNativeBinaryPath: (name: string) => `/mock/bin/${name}`,
}));

function makeMockChild() {
  const child = new EventEmitter() as EventEmitter & {
    stdin: EventEmitter & {
      write: ReturnType<typeof vi.fn>;
      writable: boolean;
      destroyed: boolean;
    };
    stdout: EventEmitter;
    stderr: EventEmitter;
    kill: ReturnType<typeof vi.fn>;
  };
  child.stdin = Object.assign(new EventEmitter(), {
    write: vi.fn(() => true),
    writable: true,
    destroyed: false,
  });
  child.stdout = new EventEmitter();
  child.stderr = new EventEmitter();
  child.kill = vi.fn();
  return child;
}

describe('NativeDaemon platform support', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
    vi.resetModules();
  });

  it('reports the daemon as supported on macOS and Windows', async () => {
    Object.defineProperty(process, 'platform', { value: 'darwin' });
    let { daemon } = await import('@/main/daemon');
    expect(daemon.isSupported).toBe(true);

    vi.resetModules();
    Object.defineProperty(process, 'platform', { value: 'win32' });
    ({ daemon } = await import('@/main/daemon'));
    expect(daemon.isSupported).toBe(true);
  });

  it('only kills Windows daemon processes with the same executable path', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    const child = makeMockChild();
    mockSpawn.mockReturnValue(child);

    const { daemon } = await import('@/main/daemon');
    const startPromise = daemon.start();
    child.stdout.emit(
      'data',
      Buffer.from(JSON.stringify({ event: 'system:ready' }) + '\n')
    );
    await startPromise;

    expect(mockExecFileSync).toHaveBeenCalledTimes(1);
    const [executable, args, options] = mockExecFileSync.mock.calls[0] as [
      string,
      string[],
      { stdio: string; env: Record<string, string> },
    ];
    expect(executable).toBe('powershell.exe');
    expect(args.join(' ')).toContain('$env:CAPTY_DAEMON_PATH');
    expect(args.join(' ')).not.toContain('taskkill');
    expect(options).toMatchObject({
      stdio: 'ignore',
      env: { CAPTY_DAEMON_PATH: '/mock/bin/capty-daemon' },
    });
    expect(mockExecSync).not.toHaveBeenCalled();

    const stopPromise = daemon.stop();
    child.emit('exit', 0, null);
    await stopPromise;
  });
});

describe('NativeDaemon on unsupported platforms', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.defineProperty(process, 'platform', { value: 'linux' });
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
    vi.resetModules();
  });

  it('reports the daemon as unsupported', async () => {
    const { daemon } = await import('@/main/daemon');
    expect(daemon.isSupported).toBe(false);
  });

  it('start resolves without spawning a process', async () => {
    const { daemon } = await import('@/main/daemon');
    await expect(daemon.start()).resolves.toBeUndefined();
    expect(mockSpawn).not.toHaveBeenCalled();
    expect(mockExecFileSync).not.toHaveBeenCalled();
    expect(mockExecSync).not.toHaveBeenCalled();
  });

  it('call rejects with a platform error', async () => {
    const { daemon } = await import('@/main/daemon');
    await expect(daemon.call('ocr', 'recognize')).rejects.toThrow(
      'Daemon not supported on this platform: ocr.recognize'
    );
  });
});
