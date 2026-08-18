import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockDaemonCall = vi.fn();
const mockGetAccentColor = vi.fn();
const mockGetControlWindow = vi.fn();
let configUpdatedHandler: ((updates: { appearance?: unknown }) => void) | null =
  null;

vi.mock('electron', () => ({
  app: { isPackaged: false, getVersion: () => '0.0.0' },
  nativeTheme: { on: vi.fn() },
}));

vi.mock('@/main/daemon', () => ({
  daemon: { call: (...args: unknown[]) => mockDaemonCall(...args) },
}));

vi.mock('@/main/settings', () => ({
  onConfigUpdated: (handler: (updates: { appearance?: unknown }) => void) => {
    configUpdatedHandler = handler;
    return () => {
      configUpdatedHandler = null;
    };
  },
}));

vi.mock('@/main/settings/accent', () => ({
  getAccentColor: () => mockGetAccentColor(),
}));

vi.mock('@/main/capture/video/recording-control-window', () => ({
  getRecordingControlBrowserWindow: () => mockGetControlWindow(),
}));

function controlWindowWithHandle(handle: number) {
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64LE(BigInt(handle));
  return {
    isDestroyed: () => false,
    getNativeWindowHandle: () => buffer,
  };
}

describe('recording overlay', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    configUpdatedHandler = null;
    mockGetAccentColor.mockReturnValue('#8892ef');
    mockGetControlWindow.mockReturnValue(null);
  });

  function restorePlatform(): void {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  }

  it('outlines a window in the live theme accent', async () => {
    mockDaemonCall.mockResolvedValue({});
    mockGetAccentColor.mockReturnValue('#5f6cd9');
    const { showRecordedWindowOutline } =
      await import('@/main/capture/video/overlay');

    await showRecordedWindowOutline(4242);

    expect(mockDaemonCall).toHaveBeenCalledWith(
      'recording-overlay',
      'showWindow',
      { windowId: 4242, color: '#5f6cd9' }
    );
  });

  it('anchors the outline below the toolbar on Windows', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    mockDaemonCall.mockResolvedValue({});
    mockGetControlWindow.mockReturnValue(controlWindowWithHandle(305419896));
    const { showRecordedWindowOutline } =
      await import('@/main/capture/video/overlay');

    await showRecordedWindowOutline(4242);
    restorePlatform();

    expect(mockDaemonCall).toHaveBeenCalledWith(
      'recording-overlay',
      'showWindow',
      { windowId: 4242, color: '#8892ef', belowWindowId: 305419896 }
    );
  });

  it('skips the toolbar anchor when the toolbar window is missing', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    mockDaemonCall.mockResolvedValue({});
    const { showRecordedWindowOutline } =
      await import('@/main/capture/video/overlay');

    await showRecordedWindowOutline(4242);
    restorePlatform();

    expect(mockDaemonCall).toHaveBeenCalledWith(
      'recording-overlay',
      'showWindow',
      { windowId: 4242, color: '#8892ef' }
    );
  });

  it('does not restart an outline that is already up', async () => {
    mockDaemonCall.mockResolvedValue({});
    const { showRecordedWindowOutline } =
      await import('@/main/capture/video/overlay');

    await showRecordedWindowOutline(4242);
    await showRecordedWindowOutline(4242);

    expect(mockDaemonCall).toHaveBeenCalledTimes(1);
  });

  it('re-applies the outline accent when the appearance setting changes', async () => {
    mockDaemonCall.mockResolvedValue({});
    const { showRecordedWindowOutline } =
      await import('@/main/capture/video/overlay');

    await showRecordedWindowOutline(4242);
    mockGetAccentColor.mockReturnValue('#22c55e');
    configUpdatedHandler?.({ appearance: { theme: 'forest' } });
    await vi.waitFor(() => expect(mockDaemonCall).toHaveBeenCalledTimes(2));

    expect(mockDaemonCall).toHaveBeenLastCalledWith(
      'recording-overlay',
      'showWindow',
      { windowId: 4242, color: '#22c55e' }
    );
  });

  it('ignores appearance changes while no window is outlined', async () => {
    mockDaemonCall.mockResolvedValue({});
    await import('@/main/capture/video/overlay');

    configUpdatedHandler?.({ screenshot: {} });

    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it.skipIf(process.platform === 'win32')(
    'showRecordingOverlay calls daemon with bounds',
    async () => {
      mockDaemonCall.mockResolvedValue({});
      const { showRecordingOverlay } =
        await import('@/main/capture/video/overlay');
      await showRecordingOverlay(10, 20, 800, 600);

      expect(mockDaemonCall).toHaveBeenCalledWith('recording-overlay', 'show', {
        x: 10,
        y: 20,
        width: 800,
        height: 600,
      });
    }
  );

  it.skipIf(process.platform !== 'win32')(
    'showRecordingOverlay is a no-op on Windows',
    async () => {
      mockDaemonCall.mockResolvedValue({});
      const { showRecordingOverlay } =
        await import('@/main/capture/video/overlay');
      await showRecordingOverlay(10, 20, 800, 600);

      expect(mockDaemonCall).not.toHaveBeenCalled();
    }
  );

  it.skipIf(process.platform === 'win32')(
    'show propagates daemon failure',
    async () => {
      mockDaemonCall.mockRejectedValue(new Error('boom'));
      const { showRecordingOverlay } =
        await import('@/main/capture/video/overlay');

      await expect(showRecordingOverlay(0, 0, 100, 100)).rejects.toThrow(
        'boom'
      );
    }
  );

  it.skipIf(process.platform !== 'win32')(
    'show ignores daemon failure on Windows',
    async () => {
      mockDaemonCall.mockRejectedValue(new Error('boom'));
      const { showRecordingOverlay } =
        await import('@/main/capture/video/overlay');

      await expect(
        showRecordingOverlay(0, 0, 100, 100)
      ).resolves.toBeUndefined();
      expect(mockDaemonCall).not.toHaveBeenCalled();
    }
  );

  it('hide is a no-op when overlay was never shown', async () => {
    const { hideRecordingOverlay } =
      await import('@/main/capture/video/overlay');
    await hideRecordingOverlay();
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it.skipIf(process.platform === 'win32')(
    'hide calls daemon after show',
    async () => {
      mockDaemonCall.mockResolvedValue({});
      const m = await import('@/main/capture/video/overlay');
      await m.showRecordingOverlay(0, 0, 100, 100);
      mockDaemonCall.mockClear();
      await m.hideRecordingOverlay();

      expect(mockDaemonCall).toHaveBeenCalledWith('recording-overlay', 'hide');
    }
  );

  it.skipIf(process.platform !== 'win32')(
    'hide is a no-op on Windows after show',
    async () => {
      mockDaemonCall.mockResolvedValue({});
      const m = await import('@/main/capture/video/overlay');
      await m.showRecordingOverlay(0, 0, 100, 100);
      mockDaemonCall.mockClear();
      await m.hideRecordingOverlay();

      expect(mockDaemonCall).not.toHaveBeenCalled();
    }
  );

  it('force hide calls daemon when show did not complete', async () => {
    mockDaemonCall.mockResolvedValue({});
    const { hideRecordingOverlay } =
      await import('@/main/capture/video/overlay');
    await hideRecordingOverlay(true);
    expect(mockDaemonCall).toHaveBeenCalledWith('recording-overlay', 'hide');
  });

  it('hide swallows daemon errors', async () => {
    mockDaemonCall
      .mockResolvedValueOnce({})
      .mockRejectedValueOnce(new Error('boom'));
    const m = await import('@/main/capture/video/overlay');
    await m.showRecordingOverlay(0, 0, 100, 100);
    await expect(m.hideRecordingOverlay()).resolves.toBeUndefined();
  });

  it('prewarmOverlay is a noop', async () => {
    const { prewarmOverlay } = await import('@/main/capture/video/overlay');
    await expect(prewarmOverlay()).resolves.toBeUndefined();
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });
});
