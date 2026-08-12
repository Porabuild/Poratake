import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockGetConfig = vi.fn();
const mockUpdateConfig = vi.fn();
const mockStartAreaSelection = vi.fn();
const mockCancelAreaSelection = vi.fn();
const mockCaptureArea = vi.fn();
const mockHideDesktopIcons = vi.fn();
const mockShowDesktopIcons = vi.fn();
const mockIsSupported = vi.fn();
const mockCheckAccessibility = vi.fn();
const mockDaemonCall = vi.fn();
const mockDaemonOnEvent = vi.fn();
const mockDaemonOffEvent = vi.fn();
const mockSelectAreaWithOverlay = vi.fn();
const mockReleaseSelection = vi.fn();
const mockIsFreezeScreenEnabled = vi.fn(() => true);

vi.mock('electron', () => ({
  screen: {
    dipToScreenPoint: (point: { x: number; y: number }) => point,
  },
}));

vi.mock('@/main/capture/area-overlay', () => ({
  selectAreaWithOverlay: (...a: unknown[]) => mockSelectAreaWithOverlay(...a),
}));

vi.mock('@/main/capture/freeze-screen/preference', () => ({
  isFreezeScreenEnabled: () => mockIsFreezeScreenEnabled(),
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
    vi.resetAllMocks();
    vi.resetModules();
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: false } });
    mockIsSupported.mockReturnValue(true);
    mockCheckAccessibility.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({});
    mockStartAreaSelection.mockResolvedValue(null);
    mockCaptureArea.mockResolvedValue(undefined);
  });

  it('starts and cancels when area selection cancelled', async () => {
    mockStartAreaSelection.mockImplementation(
      (opts: { onCancelled: () => Promise<void> }) => {
        setImmediate(() => opts.onCancelled());
        return new Promise(() => {});
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
        return new Promise(() => {});
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
        return new Promise(() => {});
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
        return new Promise(() => {});
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
        Promise.resolve().then(async () => {
          await opts.onSelected({
            status: 'selected',
            x: 10,
            y: 20,
            width: 800,
            height: 600,
          });
        });
        setTimeout(() => {
          [...handlers].forEach(h => h('timer-control:completed'));
        }, 10);
        return new Promise(() => {});
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
        return new Promise(() => {});
      }
    );
    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();
    expect(mockCaptureArea).not.toHaveBeenCalled();
  }, 10000);

  it('ignores a second timer request while area selection is active', async () => {
    let cancelSelection: () => void = () => {};
    mockStartAreaSelection.mockImplementation(
      (opts: { onCancelled: () => void }) => {
        cancelSelection = opts.onCancelled;
        return new Promise(() => {});
      }
    );
    const timerCapture = (await import('@/main/capture/timer-capture')).default;

    const firstCapture = timerCapture();
    await vi.waitFor(() => expect(mockStartAreaSelection).toHaveBeenCalled());
    await timerCapture();

    expect(mockStartAreaSelection).toHaveBeenCalledTimes(1);

    cancelSelection();
    await firstCapture;
  });

  it('settles and resets state when the timer control cannot open', async () => {
    mockStartAreaSelection.mockImplementation(
      (opts: { onSelected: (s: unknown) => Promise<void> }) => {
        Promise.resolve().then(() =>
          opts.onSelected({ x: 0, y: 0, width: 100, height: 100 })
        );
        return new Promise(() => {});
      }
    );
    mockDaemonCall.mockImplementation(async (_module, method) => {
      if (method === 'show') {
        throw new Error('timer unavailable');
      }
      return {};
    });
    const timerCapture = (await import('@/main/capture/timer-capture')).default;

    await expect(timerCapture()).resolves.toBeUndefined();
    expect(mockCancelAreaSelection).toHaveBeenCalledWith(true);

    mockStartAreaSelection.mockResolvedValue(null);
    await timerCapture();
    expect(mockStartAreaSelection).toHaveBeenCalledTimes(2);
  });

  it('restores icons and state when capture fails', async () => {
    const handlers: Array<(event: string) => void> = [];
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockDaemonOnEvent.mockImplementation((handler: (event: string) => void) => {
      handlers.push(handler);
    });
    mockDaemonOffEvent.mockImplementation(
      (handler: (event: string) => void) => {
        const index = handlers.indexOf(handler);
        if (index >= 0) handlers.splice(index, 1);
      }
    );
    mockStartAreaSelection.mockImplementation(
      (opts: { onSelected: (s: unknown) => Promise<void> }) => {
        Promise.resolve().then(() =>
          opts.onSelected({ x: 0, y: 0, width: 100, height: 100 })
        );
        setImmediate(() => {
          [...handlers].forEach(handler => handler('timer-control:completed'));
        });
        return new Promise(() => {});
      }
    );
    mockCaptureArea.mockRejectedValue(new Error('capture failed'));
    const timerCapture = (await import('@/main/capture/timer-capture')).default;

    await expect(timerCapture()).resolves.toBeUndefined();
    expect(mockShowDesktopIcons).toHaveBeenCalledTimes(1);

    mockStartAreaSelection.mockResolvedValue(null);
    await timerCapture();
    expect(mockStartAreaSelection).toHaveBeenCalledTimes(2);
  });

  it('waits for an in-flight timer show before settling cancellation', async () => {
    let selectArea: (selection: unknown) => Promise<void> = async () => {};
    let cancelArea: () => void = () => {};
    let finishShow: () => void = () => {};
    mockStartAreaSelection.mockImplementation(
      (opts: {
        onSelected: (selection: unknown) => Promise<void>;
        onCancelled: () => void;
      }) => {
        selectArea = opts.onSelected;
        cancelArea = opts.onCancelled;
        return new Promise(() => {});
      }
    );
    mockDaemonCall.mockImplementation((_module, method) => {
      if (method !== 'show') {
        return Promise.resolve({});
      }
      return new Promise(resolve => {
        finishShow = () => resolve({});
      });
    });
    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    let settled = false;
    const capture = timerCapture().then(() => {
      settled = true;
    });

    await vi.waitFor(() => expect(mockStartAreaSelection).toHaveBeenCalled());
    void selectArea({ x: 0, y: 0, width: 100, height: 100 });
    await vi.waitFor(() =>
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'timer-control',
        'show',
        expect.any(Object)
      )
    );
    cancelArea();
    await Promise.resolve();

    expect(settled).toBe(false);

    finishShow();
    await capture;
    expect(mockCancelAreaSelection).toHaveBeenCalledWith(true);
  });
});

describe('timer-capture on Windows', () => {
  const originalPlatform = process.platform;
  const handlers: Array<(e: string, d?: unknown) => void> = [];

  const selectedRegion = { x: 110, y: 70, width: 300, height: 200 };
  const overlaySelection = {
    display: { id: 1, bounds: { x: 100, y: 50, width: 1920, height: 1080 } },
    rect: selectedRegion,
    release: (...a: unknown[]) => mockReleaseSelection(...a),
  };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockSelectAreaWithOverlay.mockReset();
    mockReleaseSelection.mockReset();
    mockReleaseSelection.mockResolvedValue(undefined);
    mockCaptureArea.mockReset();
    mockDaemonCall.mockReset();
    handlers.length = 0;
    mockIsFreezeScreenEnabled.mockReturnValue(true);
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
    mockSelectAreaWithOverlay.mockImplementation(async () => {
      setTimeout(() => {
        [...handlers].forEach(h => h('timer-control:completed'));
      }, 10);
      return overlaySelection;
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
    mockSelectAreaWithOverlay.mockImplementation(async () => {
      setTimeout(() => {
        [...handlers].forEach(h => h('timer-control:cancel'));
      }, 10);
      return overlaySelection;
    });

    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();

    expect(mockCaptureArea).not.toHaveBeenCalled();
  }, 10000);

  it('bails out when area selection is cancelled', async () => {
    mockSelectAreaWithOverlay.mockResolvedValue(null);

    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();

    expect(mockDaemonCall).not.toHaveBeenCalled();
    expect(mockCaptureArea).not.toHaveBeenCalled();
  });

  it('follows the freeze screen setting when selecting the area', async () => {
    mockIsFreezeScreenEnabled.mockReturnValue(false);
    mockSelectAreaWithOverlay.mockResolvedValue(null);

    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();

    expect(mockSelectAreaWithOverlay).toHaveBeenCalledWith({ freeze: false });
  });

  it('restores desktop icons after a cancelled selection', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockSelectAreaWithOverlay.mockResolvedValue(null);

    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await timerCapture();

    expect(mockHideDesktopIcons).toHaveBeenCalledWith('capture');
    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
  });

  it('restores state and desktop icons when area selection fails', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockSelectAreaWithOverlay.mockRejectedValueOnce(
      new Error('capture unavailable')
    );

    const timerCapture = (await import('@/main/capture/timer-capture')).default;
    await expect(timerCapture()).resolves.toBeUndefined();

    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');

    mockSelectAreaWithOverlay.mockResolvedValueOnce(null);
    await timerCapture();
    expect(mockSelectAreaWithOverlay).toHaveBeenCalledTimes(2);
  });

  it('settles and restores state when the timer control cannot open', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockSelectAreaWithOverlay.mockResolvedValueOnce(overlaySelection);
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

    mockSelectAreaWithOverlay.mockResolvedValueOnce(null);
    await timerCapture();
    expect(mockSelectAreaWithOverlay).toHaveBeenCalledTimes(2);
  });
});
