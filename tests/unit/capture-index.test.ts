import { describe, it, expect, vi, beforeEach } from 'vitest';

type ExecCallback = (error: Error | null) => void;
const mockExecFile =
  vi.fn<(file: string, args: string[], cb: ExecCallback) => void>();
const mockPrewarmCapturePreview = vi.fn();
const mockPrewarmAreaOverlay = vi.fn();
const mockOnConfigUpdated = vi.fn();
const mockPlatform = vi.hoisted(() => ({ isMac: true }));

vi.mock('child_process', () => ({
  execFile: (file: string, args: string[], cb: ExecCallback) =>
    mockExecFile(file, args, cb),
}));

vi.mock('@/main/capture/capture-preview', () => ({
  prewarmCapturePreview: () => mockPrewarmCapturePreview(),
}));

vi.mock('@/main/capture/area-overlay', () => ({
  prewarmAreaOverlay: () => mockPrewarmAreaOverlay(),
}));

vi.mock('@/main/settings', () => ({
  onConfigUpdated: (listener: (updates: Record<string, unknown>) => void) =>
    mockOnConfigUpdated(listener),
}));

vi.mock('@/main/utils/platform', () => ({
  get isMac() {
    return mockPlatform.isMac;
  },
}));

describe('capture index', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockPlatform.isMac = true;
  });

  it('resetScreenCaptureCache execs killall', async () => {
    mockExecFile.mockImplementation((file, args, cb) => {
      expect(file).toBe('killall');
      expect(args).toEqual(['screencapturemgr']);
      cb(null);
    });
    const { resetScreenCaptureCache } = await import('@/main/capture');
    resetScreenCaptureCache();
    expect(mockExecFile).toHaveBeenCalled();
  });

  it('resetScreenCaptureCache survives exec error', async () => {
    mockExecFile.mockImplementation((_file, _args, cb) =>
      cb(new Error('not found'))
    );
    const { resetScreenCaptureCache } = await import('@/main/capture');
    expect(() => resetScreenCaptureCache()).not.toThrow();
  });

  it('init prewarms the capture preview', async () => {
    mockPlatform.isMac = false;
    const { init } = await import('@/main/capture');
    init();
    expect(mockPrewarmCapturePreview).toHaveBeenCalledTimes(1);
    expect(mockPrewarmAreaOverlay).toHaveBeenCalledTimes(1);
  });

  it('synchronizes the warm preview when preview settings change', async () => {
    const { init } = await import('@/main/capture');
    init();
    const listener = mockOnConfigUpdated.mock.calls[0][0];

    listener({ appearance: {} });
    expect(mockPrewarmCapturePreview).toHaveBeenCalledTimes(1);

    listener({ screenshot: {} });
    expect(mockPrewarmCapturePreview).toHaveBeenCalledTimes(2);

    listener({ recording: {} });
    expect(mockPrewarmCapturePreview).toHaveBeenCalledTimes(3);
  });
});
