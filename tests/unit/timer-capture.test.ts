import { describe, it, expect, vi, beforeEach } from 'vitest';

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
