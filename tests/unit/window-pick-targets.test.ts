import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockDaemonCall = vi.fn();
const mockScreenToDipRect = vi.fn();
const mockGetOverlayWindowIds = vi.fn();
const mockPlatform = vi.hoisted(() => ({ isWindows: false }));

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...a: unknown[]) => mockDaemonCall(...a),
  },
}));

vi.mock('electron', () => ({
  screen: {
    screenToDipRect: (...a: unknown[]) => mockScreenToDipRect(...a),
  },
}));

vi.mock('@/main/utils/platform', () => mockPlatform);

vi.mock('@/main/capture/area-overlay/session', () => ({
  getOverlayWindowIds: () => mockGetOverlayWindowIds(),
}));

function windowList() {
  return [
    {
      windowId: 99,
      title: '',
      ownerName: 'overlay',
      ownerPid: 1,
      bounds: { x: 0, y: 0, width: 1920, height: 1080 },
    },
    {
      windowId: 4242,
      title: 'Window',
      ownerName: 'app',
      ownerPid: 42,
      bounds: { x: 200, y: 100, width: 800, height: 600 },
    },
    {
      windowId: 4243,
      title: '',
      ownerName: 'untitled-app',
      ownerPid: 43,
      bounds: { x: 300, y: 200, width: 400, height: 300 },
    },
  ];
}

describe('window pick targets', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockPlatform.isWindows = false;
    mockDaemonCall.mockResolvedValue({ windows: windowList() });
    mockGetOverlayWindowIds.mockReturnValue(new Set([99]));
    mockScreenToDipRect.mockImplementation(
      (_window: unknown, rect: Record<string, number>) => ({
        x: rect.x / 2,
        y: rect.y / 2,
        width: rect.width / 2,
        height: rect.height / 2,
      })
    );
  });

  it('returns null when the daemon lists no windows', async () => {
    mockDaemonCall.mockResolvedValue({ windows: [] });
    const consoleError = vi
      .spyOn(console, 'error')
      .mockImplementation(() => {});
    const { resolveWindowPickTargets } =
      await import('@/main/capture/area-overlay/window-pick-targets');

    expect(await resolveWindowPickTargets()).toBeNull();
    expect(consoleError).toHaveBeenCalled();
    consoleError.mockRestore();
  });

  it('never offers the overlay windows themselves as pick targets', async () => {
    mockGetOverlayWindowIds.mockReturnValue(new Set([99]));
    const { resolveWindowPickTargets } =
      await import('@/main/capture/area-overlay/window-pick-targets');

    const resolved = await resolveWindowPickTargets();

    expect(resolved?.targets.map(target => target.id)).toEqual([4242, 4243]);
  });

  it('returns null when only overlay windows are on screen', async () => {
    mockGetOverlayWindowIds.mockReturnValue(new Set([99]));
    mockDaemonCall.mockResolvedValue({ windows: [windowList()[0]] });
    const consoleError = vi
      .spyOn(console, 'error')
      .mockImplementation(() => {});
    const { resolveWindowPickTargets } =
      await import('@/main/capture/area-overlay/window-pick-targets');

    expect(await resolveWindowPickTargets()).toBeNull();
    consoleError.mockRestore();
  });

  it('converts window bounds to DIP on Windows', async () => {
    mockPlatform.isWindows = true;
    const { resolveWindowPickTargets } =
      await import('@/main/capture/area-overlay/window-pick-targets');

    const resolved = await resolveWindowPickTargets();

    expect(mockScreenToDipRect).toHaveBeenCalledWith(null, {
      x: 200,
      y: 100,
      width: 800,
      height: 600,
    });
    expect(resolved?.targets[0].rect).toEqual({
      x: 100,
      y: 50,
      width: 400,
      height: 300,
    });
  });

  it('passes window bounds through untouched off Windows', async () => {
    const { resolveWindowPickTargets } =
      await import('@/main/capture/area-overlay/window-pick-targets');

    const resolved = await resolveWindowPickTargets();

    expect(mockScreenToDipRect).not.toHaveBeenCalled();
    expect(resolved?.targets[0].rect).toEqual({
      x: 200,
      y: 100,
      width: 800,
      height: 600,
    });
    expect(resolved?.captureRects.get(4242)).toEqual({
      x: 200,
      y: 100,
      width: 800,
      height: 600,
    });
  });

  it('falls back to the owner name for untitled windows', async () => {
    const { resolveWindowPickTargets } =
      await import('@/main/capture/area-overlay/window-pick-targets');

    const resolved = await resolveWindowPickTargets();

    expect(resolved?.names.get(4242)).toBe('Window');
    expect(resolved?.names.get(4243)).toBe('untitled-app');
  });

  it('prompts the user to click a window', async () => {
    const { resolveWindowPickTargets } =
      await import('@/main/capture/area-overlay/window-pick-targets');

    const resolved = await resolveWindowPickTargets();

    expect(resolved?.prompt).toBe(
      'Click a window to select it · Esc to cancel'
    );
  });
});
