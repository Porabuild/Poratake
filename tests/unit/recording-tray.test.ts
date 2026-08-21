import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockResize = vi.fn(() => ({ resized: true }));
const mockNativeImageCreateFromPath = vi.fn(() => ({
  isEmpty: () => false,
  resize: (...a: unknown[]) => mockResize(...a),
}));
const mockStopRecordingAction = vi.fn();
const mockRebuildTrayMenu = vi.fn();
const mockFlushPendingContinuations = vi.fn();

const trayInstances: MockTray[] = [];

class MockTray {
  setToolTip = vi.fn();
  setIgnoreDoubleClickEvents = vi.fn();
  destroy = vi.fn();
  handlers: Record<string, ((...a: unknown[]) => unknown)[]> = {};
  on = vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
    this.handlers[event] ??= [];
    this.handlers[event].push(cb);
  });

  constructor(_icon: unknown) {
    void _icon;
    trayInstances.push(this);
  }
}

vi.mock('electron', () => ({
  Tray: MockTray,
  app: { getAppPath: () => '/app' },
  nativeImage: {
    createFromPath: (...a: unknown[]) => mockNativeImageCreateFromPath(...a),
  },
}));

vi.mock('@/main/utils/env.ts', () => ({ isProduction: false }));
vi.mock('@/main/utils/platform.ts', () => ({ isWindows: true }));
vi.mock('@/main/utils/paths.ts', () => ({
  getPublicAssetPath: (asset: string) => `/public/${asset}`,
}));
vi.mock('@/main/utils/event-loop.ts', () => ({
  flushPendingContinuations: () => mockFlushPendingContinuations(),
}));

async function loadRecordingTray() {
  const recordingTray = await import('@/main/menu/recording-tray');
  recordingTray.setRecordingTrayStopHandler(() => mockStopRecordingAction());
  recordingTray.setRecordingTrayMenuRebuild(() => mockRebuildTrayMenu());
  return recordingTray;
}

describe('recording-tray', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    trayInstances.splice(0);
  });

  it('showRecordingTray creates a Tray', async () => {
    const { showRecordingTray } = await loadRecordingTray();
    showRecordingTray();
    expect(trayInstances.length).toBe(1);
    expect(trayInstances[0].setToolTip).toHaveBeenCalledWith(
      'Click to stop recording'
    );
    expect(mockNativeImageCreateFromPath).toHaveBeenCalledWith(
      '/public/tray-icon.png'
    );
    expect(mockResize).toHaveBeenCalledWith({ width: 16, height: 16 });
  });

  it('showRecordingTray is idempotent', async () => {
    const { showRecordingTray } = await loadRecordingTray();
    showRecordingTray();
    showRecordingTray();
    expect(trayInstances.length).toBe(1);
  });

  it('clicking the tray stops recording and rebuilds menu', async () => {
    mockStopRecordingAction.mockResolvedValue(undefined);
    const { showRecordingTray } = await loadRecordingTray();
    showRecordingTray();
    const tray = trayInstances[0];
    await (tray.handlers['click'] || [])[0]();
    expect(mockFlushPendingContinuations).toHaveBeenCalledOnce();
    expect(
      mockFlushPendingContinuations.mock.invocationCallOrder[0]
    ).toBeLessThan(mockStopRecordingAction.mock.invocationCallOrder[0]);
    expect(mockStopRecordingAction).toHaveBeenCalled();
    expect(tray.destroy).toHaveBeenCalled();
    expect(mockRebuildTrayMenu).toHaveBeenCalled();
  });

  it('cleans up the tray when stopping fails', async () => {
    mockStopRecordingAction.mockRejectedValue(new Error('stop failed'));
    const consoleError = vi
      .spyOn(console, 'error')
      .mockImplementation(() => {});
    const { showRecordingTray } = await loadRecordingTray();
    showRecordingTray();
    const tray = trayInstances[0];

    await expect((tray.handlers['click'] || [])[0]()).resolves.toBeUndefined();

    expect(tray.destroy).toHaveBeenCalled();
    expect(mockRebuildTrayMenu).toHaveBeenCalled();
    expect(consoleError).toHaveBeenCalledWith(
      'Error stopping recording from tray:',
      expect.objectContaining({ message: 'stop failed' })
    );
    consoleError.mockRestore();
  });

  it('hideRecordingTray destroys the tray', async () => {
    const { showRecordingTray, hideRecordingTray } = await loadRecordingTray();
    showRecordingTray();
    hideRecordingTray();
    expect(trayInstances[0].destroy).toHaveBeenCalled();
  });

  it('hideRecordingTray is safe when no tray exists', async () => {
    const { hideRecordingTray } = await loadRecordingTray();
    expect(() => hideRecordingTray()).not.toThrow();
  });
});
