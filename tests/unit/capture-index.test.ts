import { describe, it, expect, vi, beforeEach } from 'vitest';

type ExecCallback = (error: Error | null) => void;
const mockExecFile =
  vi.fn<(file: string, args: string[], cb: ExecCallback) => void>();
const mockPrewarmCapturePreview = vi.fn();
const mockPrewarmAreaOverlay = vi.fn();
const mockPrewarmFreezeScreen = vi.fn();
const mockInitVideoCapture = vi.fn();
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

vi.mock('@/main/capture/freeze-screen', () => ({
  prewarmFreezeScreen: () => mockPrewarmFreezeScreen(),
}));

vi.mock('@/main/capture/video', () => ({
  init: () => mockInitVideoCapture(),
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

  it('init prewarms the overlay and freeze but not the preview', async () => {
    mockPlatform.isMac = false;
    const { init } = await import('@/main/capture');
    init();
    expect(mockInitVideoCapture).toHaveBeenCalledTimes(1);
    expect(mockPrewarmCapturePreview).not.toHaveBeenCalled();
    expect(mockPrewarmAreaOverlay).toHaveBeenCalledTimes(1);
    expect(mockPrewarmFreezeScreen).toHaveBeenCalledTimes(1);
  });

  it('prewarms the freeze screen on macOS too', async () => {
    const { init } = await import('@/main/capture');
    init();
    expect(mockPrewarmFreezeScreen).toHaveBeenCalledTimes(1);
  });
});
