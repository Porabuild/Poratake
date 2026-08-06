import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { EventEmitter } from 'events';

interface MockStdio extends EventEmitter {
  write: (data: string) => boolean;
  writable: boolean;
  destroyed: boolean;
}

interface MockChild extends EventEmitter {
  stdin: MockStdio;
  stdout: EventEmitter;
  stderr: EventEmitter;
  kill: ReturnType<typeof vi.fn>;
  pid: number;
}

let mockChild: MockChild;
const mockSpawn = vi.fn();
const mockExecSync = vi.fn();

vi.mock('child_process', () => ({
  spawn: (...a: unknown[]) => mockSpawn(...a),
  execSync: (...a: unknown[]) => mockExecSync(...a),
}));

vi.mock('@/main/utils/paths', () => ({
  getNativeBinaryPath: (name: string) => `/mock/bin/${name}`,
}));

function makeMockChild(): MockChild {
  const child = new EventEmitter() as MockChild;
  const stdin = new EventEmitter() as MockStdio;
  stdin.write = vi.fn(() => true);
  stdin.writable = true;
  stdin.destroyed = false;
  child.stdin = stdin;
  child.stdout = new EventEmitter();
  child.stderr = new EventEmitter();
  child.kill = vi.fn();
  child.pid = 1234;
  return child;
}

describe('NativeDaemon', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockChild = makeMockChild();
    mockSpawn.mockReturnValue(mockChild);
    mockExecSync.mockImplementation(() => {
      throw new Error('no stale');
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('start spawns the daemon process and waits for system:ready', async () => {
    const { daemon } = await import('@/main/daemon');
    const startPromise = daemon.start();
    setImmediate(() => {
      mockChild.stdout.emit(
        'data',
        Buffer.from(JSON.stringify({ event: 'system:ready' }) + '\n')
      );
    });
    await startPromise;
    expect(mockSpawn).toHaveBeenCalledWith('/mock/bin/capty-daemon', [], {
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    const stopPromise = daemon.stop();
    mockChild.emit('exit', 0, null);
    await stopPromise;
  });

  it('call rejects when daemon is not running', async () => {
    const { daemon } = await import('@/main/daemon');
    await expect(daemon.call('m', 'foo')).rejects.toThrow('Daemon not running');
  });

  it('call sends a request and resolves on success response', async () => {
    const { daemon } = await import('@/main/daemon');
    const startPromise = daemon.start();
    setImmediate(() => {
      mockChild.stdout.emit(
        'data',
        Buffer.from(JSON.stringify({ event: 'system:ready' }) + '\n')
      );
    });
    await startPromise;

    const writeSpy = mockChild.stdin.write as ReturnType<typeof vi.fn>;
    writeSpy.mockClear();

    const callPromise = daemon.call<{ data: number }>('mymod', 'method', {
      a: 1,
    });
    expect(writeSpy).toHaveBeenCalled();
    const sentRaw = String(writeSpy.mock.calls[0][0]);
    const sent = JSON.parse(sentRaw);
    expect(sent.module).toBe('mymod');
    expect(sent.method).toBe('method');
    expect(sent.params).toEqual({ a: 1 });
    expect(typeof sent.id).toBe('string');

    mockChild.stdout.emit(
      'data',
      Buffer.from(
        JSON.stringify({ id: sent.id, success: true, result: { data: 7 } }) +
          '\n'
      )
    );

    const result = await callPromise;
    expect(result).toEqual({ data: 7 });
    const stopPromise = daemon.stop();
    mockChild.emit('exit', 0, null);
    await stopPromise;
  });

  it('call rejects when daemon returns error response', async () => {
    const { daemon } = await import('@/main/daemon');
    const start = daemon.start();
    setImmediate(() => {
      mockChild.stdout.emit(
        'data',
        Buffer.from(JSON.stringify({ event: 'system:ready' }) + '\n')
      );
    });
    await start;

    const writeSpy = mockChild.stdin.write as ReturnType<typeof vi.fn>;
    writeSpy.mockClear();

    const callPromise = daemon.call('mymod', 'fail');
    const sent = JSON.parse(String(writeSpy.mock.calls[0][0]));
    mockChild.stdout.emit(
      'data',
      Buffer.from(
        JSON.stringify({
          id: sent.id,
          success: false,
          error: { code: 'ERR', message: 'boom' },
        }) + '\n'
      )
    );

    await expect(callPromise).rejects.toThrow('boom');
    const stopPromise = daemon.stop();
    mockChild.emit('exit', 0, null);
    await stopPromise;
  });

  it('event handlers receive daemon-event messages', async () => {
    const { daemon } = await import('@/main/daemon');
    const start = daemon.start();
    setImmediate(() => {
      mockChild.stdout.emit(
        'data',
        Buffer.from(JSON.stringify({ event: 'system:ready' }) + '\n')
      );
    });
    await start;

    const handler = vi.fn();
    daemon.onEvent(handler);
    mockChild.stdout.emit(
      'data',
      Buffer.from(
        JSON.stringify({ event: 'my:event', data: { foo: 'bar' } }) + '\n'
      )
    );
    expect(handler).toHaveBeenCalledWith('my:event', { foo: 'bar' });

    daemon.offEvent(handler);
    handler.mockClear();
    mockChild.stdout.emit(
      'data',
      Buffer.from(JSON.stringify({ event: 'another' }) + '\n')
    );
    expect(handler).not.toHaveBeenCalled();
    const stopPromise = daemon.stop();
    mockChild.emit('exit', 0, null);
    await stopPromise;
  });

  it('handles invalid JSON lines gracefully', async () => {
    const { daemon } = await import('@/main/daemon');
    const start = daemon.start();
    setImmediate(() => {
      mockChild.stdout.emit(
        'data',
        Buffer.from(JSON.stringify({ event: 'system:ready' }) + '\n')
      );
    });
    await start;
    expect(() => {
      mockChild.stdout.emit('data', Buffer.from('not json\n'));
    }).not.toThrow();
    const stopPromise = daemon.stop();
    mockChild.emit('exit', 0, null);
    await stopPromise;
  });

  it('buffers partial lines until a newline arrives', async () => {
    const { daemon } = await import('@/main/daemon');
    const start = daemon.start();
    setImmediate(() => {
      mockChild.stdout.emit(
        'data',
        Buffer.from(JSON.stringify({ event: 'system:ready' }) + '\n')
      );
    });
    await start;
    const handler = vi.fn();
    daemon.onEvent(handler);
    const payload = JSON.stringify({ event: 'partial' });
    mockChild.stdout.emit('data', Buffer.from(payload.slice(0, 10)));
    mockChild.stdout.emit('data', Buffer.from(payload.slice(10) + '\n'));
    expect(handler).toHaveBeenCalledWith('partial', undefined);
    const stopPromise = daemon.stop();
    mockChild.emit('exit', 0, null);
    await stopPromise;
  });

  it('rejects pending requests when daemon exits', async () => {
    const { daemon } = await import('@/main/daemon');
    const start = daemon.start();
    setImmediate(() => {
      mockChild.stdout.emit(
        'data',
        Buffer.from(JSON.stringify({ event: 'system:ready' }) + '\n')
      );
    });
    await start;

    const callPromise = daemon.call('mymod', 'method');
    mockChild.emit('exit', 1, null);
    await expect(callPromise).rejects.toThrow(/Daemon exited/);
  });
});
