import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockAppHide = vi.fn();
const mockDockHide = vi.fn();
const mockShow = vi.fn(() => Promise.resolve());

vi.mock('electron', () => ({
  app: {
    hide: () => mockAppHide(),
    dock: {
      hide: () => mockDockHide(),
      show: () => mockShow(),
    },
  },
  BrowserWindow: vi.fn(),
}));

interface MockWindow {
  id: number;
  listeners: Record<string, Array<() => void>>;
  on: (event: string, cb: () => void) => void;
  emit: (event: string) => void;
}

function makeWindow(id: number): MockWindow {
  const listeners: Record<string, Array<() => void>> = {};
  return {
    id,
    listeners,
    on(event, cb) {
      listeners[event] ??= [];
      listeners[event].push(cb);
    },
    emit(event) {
      (listeners[event] || []).forEach(cb => cb());
    },
  };
}

async function flushMicrotasks(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe('dock', () => {
  let originalPlatform: NodeJS.Platform;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    originalPlatform = process.platform;
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  it('no-ops on non-darwin', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    const { registerDockWindow } = await import('@/main/utils/dock');
    const win = makeWindow(1);
    await registerDockWindow(win as never, 'screenshot');
    expect(mockShow).not.toHaveBeenCalled();
    expect(mockDockHide).not.toHaveBeenCalled();
  });

  it('initDock hides the dock icon on darwin startup', async () => {
    Object.defineProperty(process, 'platform', { value: 'darwin' });
    const { initDock } = await import('@/main/utils/dock');
    initDock();
    expect(mockDockHide).toHaveBeenCalledTimes(1);
  });

  it('initDock no-ops on non-darwin', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    const { initDock } = await import('@/main/utils/dock');
    initDock();
    expect(mockDockHide).not.toHaveBeenCalled();
  });

  it('shows dock for the first window', async () => {
    Object.defineProperty(process, 'platform', { value: 'darwin' });
    const { registerDockWindow } = await import('@/main/utils/dock');
    const win = makeWindow(1);
    await registerDockWindow(win as never, 'screenshot');
    expect(mockShow).toHaveBeenCalledTimes(1);
  });

  it('does not show dock for subsequent windows', async () => {
    Object.defineProperty(process, 'platform', { value: 'darwin' });
    const { registerDockWindow } = await import('@/main/utils/dock');
    const win1 = makeWindow(1);
    const win2 = makeWindow(2);
    await registerDockWindow(win1 as never, 'screenshot');
    mockShow.mockClear();
    await registerDockWindow(win2 as never, 'pin');
    expect(mockShow).not.toHaveBeenCalled();
  });

  it('hides dock after the last window closes', async () => {
    Object.defineProperty(process, 'platform', { value: 'darwin' });
    const { registerDockWindow } = await import('@/main/utils/dock');
    const win = makeWindow(1);
    await registerDockWindow(win as never, 'screenshot');
    win.emit('closed');
    await flushMicrotasks();
    expect(mockDockHide).toHaveBeenCalled();
  });

  it('keeps dock visible while other windows remain', async () => {
    Object.defineProperty(process, 'platform', { value: 'darwin' });
    const { registerDockWindow } = await import('@/main/utils/dock');
    const win1 = makeWindow(1);
    const win2 = makeWindow(2);
    await registerDockWindow(win1 as never, 'screenshot');
    await registerDockWindow(win2 as never, 'video-editor');
    mockDockHide.mockClear();
    win1.emit('closed');
    await flushMicrotasks();
    expect(mockDockHide).not.toHaveBeenCalled();
    win2.emit('closed');
    await flushMicrotasks();
    expect(mockDockHide).toHaveBeenCalled();
  });

  it('waits for pending show before hiding when window closes mid-show', async () => {
    Object.defineProperty(process, 'platform', { value: 'darwin' });
    let resolveShow: () => void = () => {};
    mockShow.mockImplementationOnce(
      () =>
        new Promise<void>(resolve => {
          resolveShow = resolve;
        })
    );

    const { registerDockWindow } = await import('@/main/utils/dock');
    const win = makeWindow(1);
    const registerPromise = registerDockWindow(win as never, 'screenshot');

    win.emit('closed');
    await flushMicrotasks();
    expect(mockDockHide).not.toHaveBeenCalled();

    resolveShow();
    await registerPromise;
    await flushMicrotasks();
    expect(mockDockHide).toHaveBeenCalled();
  });
});
