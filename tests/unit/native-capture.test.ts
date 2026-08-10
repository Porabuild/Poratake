import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockDaemonCall = vi.fn();
const mockDipToScreenRect = vi.fn();

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...a: unknown[]) => mockDaemonCall(...a),
  },
}));

vi.mock('electron', () => ({
  screen: {
    dipToScreenRect: (...a: unknown[]) => mockDipToScreenRect(...a),
  },
}));

describe('native-capture', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockDaemonCall.mockResolvedValue({ path: '/tmp/shot.png' });
    mockDipToScreenRect.mockImplementation(
      (_window: unknown, rect: Record<string, number>) => ({
        x: rect.x * 2,
        y: rect.y * 2,
        width: rect.width * 2,
        height: rect.height * 2,
      })
    );
  });

  it('converts the requested area to physical pixels', async () => {
    const { captureRegionToFile } =
      await import('@/main/capture/screenshot/native-capture');

    const captured = await captureRegionToFile(
      { x: 10, y: 20, width: 300, height: 200 },
      '/tmp/shot.png'
    );

    expect(captured).toBe(true);
    expect(mockDaemonCall).toHaveBeenCalledWith('screenshot', 'capture-area', {
      x: 20,
      y: 40,
      width: 600,
      height: 400,
      path: '/tmp/shot.png',
      cached: false,
    });
  });

  it('reads the retained frame when requested', async () => {
    const { captureRegionToFile } =
      await import('@/main/capture/screenshot/native-capture');

    await captureRegionToFile(
      { x: 0, y: 0, width: 100, height: 100 },
      '/tmp/shot.png',
      { cached: true }
    );

    expect(mockDaemonCall).toHaveBeenCalledWith(
      'screenshot',
      'capture-area',
      expect.objectContaining({ cached: true })
    );
  });

  it('crops physical window bounds from the retained frozen frame', async () => {
    const { captureFrozenScreenRegionToFile } =
      await import('@/main/capture/screenshot/native-capture');

    await captureFrozenScreenRegionToFile(
      { x: 200, y: 100, width: 800, height: 600 },
      '/tmp/window.png',
      264610
    );

    expect(mockDipToScreenRect).not.toHaveBeenCalled();
    expect(mockDaemonCall).toHaveBeenCalledWith('screenshot', 'capture-area', {
      x: 200,
      y: 100,
      width: 800,
      height: 600,
      path: '/tmp/window.png',
      cached: true,
      windowId: 264610,
    });
  });

  it('captures a display through its bounds', async () => {
    const { captureDisplayToFile } =
      await import('@/main/capture/screenshot/native-capture');

    await captureDisplayToFile(
      { bounds: { x: 0, y: 0, width: 1920, height: 1080 } } as never,
      '/tmp/screen.png'
    );

    expect(mockDaemonCall).toHaveBeenCalledWith(
      'screenshot',
      'capture-area',
      expect.objectContaining({ width: 3840, height: 2160 })
    );
  });

  it('captures a window without converting coordinates', async () => {
    const { captureWindowToFile } =
      await import('@/main/capture/screenshot/native-capture');

    const captured = await captureWindowToFile(264610, '/tmp/window.png');

    expect(captured).toBe(true);
    expect(mockDipToScreenRect).not.toHaveBeenCalled();
    expect(mockDaemonCall).toHaveBeenCalledWith(
      'screenshot',
      'capture-window',
      {
        windowId: 264610,
        path: '/tmp/window.png',
      }
    );
  });

  it('reports failure when the daemon rejects', async () => {
    mockDaemonCall.mockRejectedValue(new Error('CAPTURE_FAILED'));
    const { captureRegionToFile, captureWindowToFile } =
      await import('@/main/capture/screenshot/native-capture');

    await expect(
      captureRegionToFile({ x: 0, y: 0, width: 10, height: 10 }, '/tmp/a.png')
    ).resolves.toBe(false);
    await expect(captureWindowToFile(1, '/tmp/b.png')).resolves.toBe(false);
  });
});
