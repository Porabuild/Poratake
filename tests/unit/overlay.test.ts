import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockDaemonCall = vi.fn();
const mockGetAccentColor = vi.fn();

vi.mock('@/main/daemon', () => ({
  daemon: { call: (...args: unknown[]) => mockDaemonCall(...args) },
}));

vi.mock('@/main/settings/accent', () => ({
  getAccentColor: () => mockGetAccentColor(),
}));

describe('recording overlay', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockGetAccentColor.mockReturnValue('#8892ef');
  });

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

  it('does not restart an outline that is already up', async () => {
    mockDaemonCall.mockResolvedValue({});
    const { showRecordedWindowOutline } =
      await import('@/main/capture/video/overlay');

    await showRecordedWindowOutline(4242);
    await showRecordedWindowOutline(4242);

    expect(mockDaemonCall).toHaveBeenCalledTimes(1);
  });

  it('showRecordingOverlay calls daemon with bounds', async () => {
    mockDaemonCall.mockResolvedValue({});
    const { showRecordingOverlay } =
      await import('@/main/capture/video/overlay');
    await showRecordingOverlay(10, 20, 800, 600);

    if (process.platform === 'win32') {
      expect(mockDaemonCall).not.toHaveBeenCalled();
      return;
    }

    expect(mockDaemonCall).toHaveBeenCalledWith('recording-overlay', 'show', {
      x: 10,
      y: 20,
      width: 800,
      height: 600,
    });
  });

  it('show propagates daemon failure', async () => {
    mockDaemonCall.mockRejectedValue(new Error('boom'));
    const { showRecordingOverlay } =
      await import('@/main/capture/video/overlay');

    if (process.platform === 'win32') {
      await expect(
        showRecordingOverlay(0, 0, 100, 100)
      ).resolves.toBeUndefined();
      expect(mockDaemonCall).not.toHaveBeenCalled();
      return;
    }

    await expect(showRecordingOverlay(0, 0, 100, 100)).rejects.toThrow('boom');
  });

  it('hide is a no-op when overlay was never shown', async () => {
    const { hideRecordingOverlay } =
      await import('@/main/capture/video/overlay');
    await hideRecordingOverlay();
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it('hide calls daemon after show', async () => {
    mockDaemonCall.mockResolvedValue({});
    const m = await import('@/main/capture/video/overlay');
    await m.showRecordingOverlay(0, 0, 100, 100);
    mockDaemonCall.mockClear();
    await m.hideRecordingOverlay();

    if (process.platform === 'win32') {
      expect(mockDaemonCall).not.toHaveBeenCalled();
      return;
    }

    expect(mockDaemonCall).toHaveBeenCalledWith('recording-overlay', 'hide');
  });

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
