import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { EventEmitter } from 'events';
import fs from 'fs';
import path from 'path';

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
    expect(args.join(' ')).toContain('$env:PORATAKE_DAEMON_PATH');
    expect(args.join(' ')).not.toContain('taskkill');
    expect(options).toMatchObject({
      stdio: 'ignore',
      env: { PORATAKE_DAEMON_PATH: '/mock/bin/poratake-daemon' },
    });
    expect(mockExecSync).not.toHaveBeenCalled();

    const stopPromise = daemon.stop();
    child.emit('exit', 0, null);
    await stopPromise;
  });

  it('passes the macOS daemon path to pgrep without a shell', async () => {
    Object.defineProperty(process, 'platform', { value: 'darwin' });
    const child = makeMockChild();
    mockSpawn.mockReturnValue(child);
    mockExecFileSync.mockReturnValue('321\n');
    const processKill = vi.spyOn(process, 'kill').mockReturnValue(true);

    const { daemon } = await import('@/main/daemon');
    const startPromise = daemon.start();
    child.stdout.emit(
      'data',
      Buffer.from(JSON.stringify({ event: 'system:ready' }) + '\n')
    );
    await startPromise;

    expect(mockExecFileSync).toHaveBeenCalledWith(
      'pgrep',
      ['-f', '/mock/bin/poratake-daemon'],
      { encoding: 'utf-8' }
    );
    expect(mockExecSync).not.toHaveBeenCalled();
    expect(processKill).toHaveBeenCalledWith(321, 'SIGKILL');

    const stopPromise = daemon.stop();
    child.emit('exit', 0, null);
    await stopPromise;
    processKill.mockRestore();
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

describe('native recording startup', () => {
  it('clears completed camera paths before later abort cleanup', () => {
    const source = fs.readFileSync(
      path.join(
        process.cwd(),
        'src',
        'main',
        'daemon',
        'ScreenRecorder',
        'Recorders',
        'CameraRecorder.swift'
      ),
      'utf8'
    );
    const stop = source.slice(
      source.indexOf('func stop()'),
      source.indexOf('func abort()')
    );
    const abort = source.slice(
      source.indexOf('func abort()'),
      source.indexOf('func captureOutput(')
    );

    expect(stop.indexOf('let finalOutputPath = outputPath')).toBeLessThan(
      stop.indexOf('outputPath = nil')
    );
    expect(stop.indexOf('let finalMetadataPath = metadataPath')).toBeLessThan(
      stop.indexOf('metadataPath = nil')
    );
    expect(abort).toMatch(/removeItem[\s\S]*outputPath = nil/);
    expect(abort).toMatch(/removeItem[\s\S]*metadataPath = nil/);
  });

  it('resolves iOS startup from the first frame without polling', () => {
    const source = fs.readFileSync(
      path.join(
        process.cwd(),
        'src',
        'main',
        'daemon',
        'ScreenRecorder',
        'Recorders',
        'IOSDeviceRecorder.swift'
      ),
      'utf8'
    );

    expect(source).toContain('withCheckedThrowingContinuation');
    expect(source).toContain('markFirstFrameReady()');
    expect(source).not.toContain('pollIntervalNs');
  });

  it('fails iOS startup when its video writer cannot start', () => {
    const source = fs.readFileSync(
      path.join(
        process.cwd(),
        'src',
        'main',
        'daemon',
        'ScreenRecorder',
        'Recorders',
        'IOSDeviceRecorder.swift'
      ),
      'utf8'
    );

    expect(source).toContain('guard assetWriter?.startWriting() == true else');
    expect(source).toContain('Failed to start video writer');
  });

  it('anchors a pre-first-frame screen pause to the first paused frame', () => {
    const source = fs.readFileSync(
      path.join(
        process.cwd(),
        'src',
        'main',
        'daemon',
        'ScreenRecorder',
        'Recorders',
        'ScreenCaptureRecorder.swift'
      ),
      'utf8'
    );

    expect(source).toContain(
      'pauseStartTime = videoFrameCount > 0 ? lastFrameTime : nil'
    );
    expect(source.indexOf('videoFrameCount = 0')).toBeLessThan(
      source.indexOf('try await stream?.startCapture()')
    );
  });

  it('caps macOS recording at the selected display refresh rate', () => {
    const types = fs.readFileSync(
      path.join(
        process.cwd(),
        'src',
        'main',
        'daemon',
        'ScreenRecorder',
        'Types',
        'RecorderTypes.swift'
      ),
      'utf8'
    );
    const recorder = fs.readFileSync(
      path.join(
        process.cwd(),
        'src',
        'main',
        'daemon',
        'ScreenRecorder',
        'Recorders',
        'ScreenCaptureRecorder.swift'
      ),
      'utf8'
    );

    expect(types).toContain('let selected = min(max(1, configured), 240)');
    expect(types).toContain(
      'return min(selected, max(1, maximum ?? selected))'
    );
    expect(recorder).toContain('maximum: targetScreen?.maximumFramesPerSecond');
    expect(recorder).toContain(
      'CMTime(value: 1, timescale: CMTimeScale(frameRate))'
    );
  });
});
