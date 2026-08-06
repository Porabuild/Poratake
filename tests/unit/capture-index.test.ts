import { describe, it, expect, vi, beforeEach } from 'vitest';

type ExecCallback = (error: Error | null) => void;
const mockExec = vi.fn<(cmd: string, cb: ExecCallback) => void>();

vi.mock('child_process', () => ({
  exec: (cmd: string, cb: ExecCallback) => mockExec(cmd, cb),
}));

describe('capture index', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it('resetScreenCaptureCache execs killall', async () => {
    mockExec.mockImplementation((cmd, cb) => {
      expect(cmd).toContain('killall screencapturemgr');
      cb(null);
    });
    const { resetScreenCaptureCache } = await import('@/main/capture');
    resetScreenCaptureCache();
    expect(mockExec).toHaveBeenCalled();
  });

  it('resetScreenCaptureCache survives exec error', async () => {
    mockExec.mockImplementation((_cmd, cb) => cb(new Error('not found')));
    const { resetScreenCaptureCache } = await import('@/main/capture');
    expect(() => resetScreenCaptureCache()).not.toThrow();
  });

  it('init is a no-op that completes synchronously', async () => {
    const { init } = await import('@/main/capture');
    expect(() => init()).not.toThrow();
  });
});
