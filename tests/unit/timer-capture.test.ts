import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockGetConfig = vi.fn();
const mockUpdateConfig = vi.fn();
const mockStartAreaSelection = vi.fn();
const mockCancelAreaSelection = vi.fn();
const mockHideAreaSelector = vi.fn();
const mockCaptureArea = vi.fn();
const mockHideDesktopIcons = vi.fn();
const mockShowDesktopIcons = vi.fn();
const mockIsSupported = vi.fn();
const mockCheckAccessibility = vi.fn();
const mockDaemonCall = vi.fn();
const mockDaemonOnEvent = vi.fn();
const mockDaemonOffEvent = vi.fn();
const mockSelectAreaRegion = vi.fn();

vi.mock('electron', () => ({
  screen: {
    dipToScreenPoint: (point: { x: number; y: number }) => point,
  },
}));

vi.mock('@/main/capture/area-capture', () => ({
  selectAreaRegion: (...a: unknown[]) => mockSelectAreaRegion(...a),
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
  updateConfig: (...a: unknown[]) => mockUpdateConfig(...a),
}));

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...a: unknown[]) => mockDaemonCall(...a),
    onEvent: (...a: unknown[]) => mockDaemonOnEvent(...a),
    offEvent: (...a: unknown[]) => mockDaemonOffEvent(...a),
  },
}));

vi.mock('@/main/capture/area-selector', () => ({
  startAreaSelection: (...a: unknown[]) => mockStartAreaSelection(...a),
  cancelAreaSelection: (...a: unknown[]) => mockCancelAreaSelection(...a),
  hideAreaSelector: (...a: unknown[]) => mockHideAreaSelector(...a),
}));

vi.mock('@/main/capture/screenshot', () => ({
  captureArea: (...a: unknown[]) => mockCaptureArea(...a),
}));

vi.mock('@/main/capture/desktop-icons', () => ({
  hideDesktopIcons: (...a: unknown[]) => mockHideDesktopIcons(...a),
  showDesktopIcons: (...a: unknown[]) => mockShowDesktopIcons(...a),
  isSupported: () => mockIsSupported(),
  checkAccessibilityPermission: (...a: unknown[]) =>
    mockCheckAccessibility(...a),
}));

describe('timer-capture', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: false } });
    mockIsSupported.mockReturnValue(true);
    mockCheckAccessibility.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({});
  });

  it('starts and cancels when area selection cancelled', async () => {
    mockStartAreaSelection.mockImplementation(
      (opts: { onCancelled: () => Promise<void> }) => {
        setImmediate(() => opts.onCancelled());
      }
    );
    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();
    expect(mockStartAreaSelection).toHaveBeenCalled();
  });

  it('disables desktop icons hide when accessibility missing', async () => {
    mockGetConfig.mockReturnValue({
      screenshot: { hideDesktopIcons: true },
    });
    mockCheckAccessibility.mockReturnValue(false);
    mockStartAreaSelection.mockImplementation(
      (opts: { onCancelled: () => Promise<void> }) => {
        setImmediate(() => opts.onCancelled());
      }
    );
    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();
    expect(mockUpdateConfig).toHaveBeenCalled();
    expect(mockHideDesktopIcons).not.toHaveBeenCalled();
  });

  it('hides desktop icons when enabled with accessibility', async () => {
    mockGetConfig.mockReturnValue({
      screenshot: { hideDesktopIcons: true },
    });
    mockStartAreaSelection.mockImplementation(
      (opts: { onCancelled: () => Promise<void> }) => {
        setImmediate(() => opts.onCancelled());
      }
    );
    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();
    expect(mockHideDesktopIcons).toHaveBeenCalledWith('capture');
    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
  });

  it('ignores area selection without bounds', async () => {
    mockStartAreaSelection.mockImplementation(
      (opts: {
        onSelected: (s: unknown) => Promise<void>;
        onCancelled: () => Promise<void>;
      }) => {
        setImmediate(async () => {
          await opts.onSelected({ status: 'selected' });
          await opts.onCancelled();
        });
      }
    );
    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();
    expect(mockCaptureArea).not.toHaveBeenCalled();
  });

  it('processes timer-completed event and triggers captureArea', async () => {
    const handlers: Array<(e: string, d?: unknown) => void> = [];
    mockDaemonOnEvent.mockImplementation(
      (cb: (e: string, d?: unknown) => void) => {
        handlers.push(cb);
      }
    );
    mockDaemonOffEvent.mockImplementation(
      (cb: (e: string, d?: unknown) => void) => {
        const i = handlers.indexOf(cb);
        if (i >= 0) handlers.splice(i, 1);
      }
    );
    mockCaptureArea.mockResolvedValue(undefined);
    mockStartAreaSelection.mockImplementation(
      (opts: { onSelected: (s: unknown) => Promise<void> }) => {
        // Run async; fire completed event after handler attaches
        Promise.resolve().then(async () => {
          await opts.onSelected({
            status: 'selected',
            x: 10,
            y: 20,
            width: 800,
            height: 600,
          });
        });
        // After microtask, queue event dispatch
        setTimeout(() => {
          [...handlers].forEach(h => h('timer-control:completed'));
        }, 10);
      }
    );
    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();
    expect(mockCaptureArea).toHaveBeenCalled();
  }, 10000);

  it('processes timer-cancel event without capture', async () => {
    const handlers: Array<(e: string, d?: unknown) => void> = [];
    mockDaemonOnEvent.mockImplementation(
      (cb: (e: string, d?: unknown) => void) => {
        handlers.push(cb);
      }
    );
    mockDaemonOffEvent.mockImplementation(
      (cb: (e: string, d?: unknown) => void) => {
        const i = handlers.indexOf(cb);
        if (i >= 0) handlers.splice(i, 1);
      }
    );
    mockStartAreaSelection.mockImplementation(
      (opts: { onSelected: (s: unknown) => Promise<void> }) => {
        Promise.resolve().then(async () => {
          await opts.onSelected({
            status: 'selected',
            x: 0,
            y: 0,
            width: 100,
            height: 100,
          });
        });
        setTimeout(() => {
          [...handlers].forEach(h => h('timer-control:cancel'));
        }, 10);
      }
    );
    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();
    expect(mockCaptureArea).not.toHaveBeenCalled();
  }, 10000);
});

describe('timer-capture on Windows', () => {
  const originalPlatform = process.platform;
  const handlers: Array<(e: string, d?: unknown) => void> = [];

  const selectedRegion = { x: 110, y: 70, width: 300, height: 200 };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockSelectAreaRegion.mockReset();
    mockCaptureArea.mockReset();
    mockDaemonCall.mockReset();
    handlers.length = 0;
    Object.defineProperty(process, 'platform', { value: 'win32' });
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: false } });
    mockIsSupported.mockReturnValue(true);
    mockCheckAccessibility.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({});
    mockDaemonOnEvent.mockImplementation(
      (cb: (e: string, d?: unknown) => void) => {
        handlers.push(cb);
      }
    );
    mockDaemonOffEvent.mockImplementation(
      (cb: (e: string, d?: unknown) => void) => {
        const i = handlers.indexOf(cb);
        if (i >= 0) handlers.splice(i, 1);
      }
    );
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  it('captures the selected area after the timer completes', async () => {
    mockSelectAreaRegion.mockImplementation(async () => {
      setTimeout(() => {
        [...handlers].forEach(h => h('timer-control:completed'));
      }, 10);
      return selectedRegion;
    });
    mockCaptureArea.mockResolvedValue('/tmp/shot.png');

    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();

    expect(mockDaemonCall).toHaveBeenCalledWith(
      'timer-control',
      'show',
      expect.objectContaining({ duration: 5 })
    );
    expect(mockCaptureArea).toHaveBeenCalledWith({
      status: 'confirmed',
      x: 110,
      y: 70,
      width: 300,
      height: 200,
    });
    expect(mockStartAreaSelection).not.toHaveBeenCalled();
  }, 10000);

  it('skips capture when the timer is cancelled', async () => {
    mockSelectAreaRegion.mockImplementation(async () => {
      setTimeout(() => {
        [...handlers].forEach(h => h('timer-control:cancel'));
      }, 10);
      return selectedRegion;
    });

    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();

    expect(mockCaptureArea).not.toHaveBeenCalled();
  }, 10000);

  it('bails out when area selection is cancelled', async () => {
    mockSelectAreaRegion.mockResolvedValue(null);

    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();

    expect(mockDaemonCall).not.toHaveBeenCalled();
    expect(mockCaptureArea).not.toHaveBeenCalled();
  });

  it('restores desktop icons after a cancelled selection', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockSelectAreaRegion.mockResolvedValue(null);

    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();

    expect(mockHideDesktopIcons).toHaveBeenCalledWith('capture');
    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
  });

  it('restores state and desktop icons when area selection fails', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockSelectAreaRegion.mockRejectedValueOnce(
      new Error('capture unavailable')
    );

    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await expect(timerCapture()).resolves.toBeUndefined();

    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');

    mockSelectAreaRegion.mockResolvedValueOnce(null);
    await timerCapture();
    expect(mockSelectAreaRegion).toHaveBeenCalledTimes(2);
  });

  it('settles and restores state when the timer control cannot open', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockSelectAreaRegion.mockResolvedValueOnce(selectedRegion);
    mockDaemonCall.mockImplementation(async (_module, method) => {
      if (method === 'show') {
        throw new Error('timer unavailable');
      }
      return {};
    });

    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await expect(timerCapture()).resolves.toBeUndefined();

    expect(mockDaemonCall).toHaveBeenCalledWith('timer-control', 'hide');
    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
    expect(handlers).toHaveLength(0);

    mockSelectAreaRegion.mockResolvedValueOnce(null);
    await timerCapture();
    expect(mockSelectAreaRegion).toHaveBeenCalledTimes(2);
  });
});
