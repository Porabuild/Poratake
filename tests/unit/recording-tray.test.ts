import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockNativeImageCreateFromPath = vi.fn(() => ({}));
const mockStopRecordingAction = vi.fn();
const mockRebuildTrayMenu = vi.fn();

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

vi.mock('@/main/capture/video', () => ({
  stopRecordingAction: () => mockStopRecordingAction(),
}));

vi.mock('@/main/menu/index.ts', () => ({
  rebuildTrayMenu: () => mockRebuildTrayMenu(),
}));

describe('recording-tray', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    trayInstances.splice(0);
  });

  it('showRecordingTray creates a Tray', async () => {
    const { showRecordingTray } = await import('@/main/menu/recording-tray');
    showRecordingTray();
    expect(trayInstances.length).toBe(1);
    expect(trayInstances[0].setToolTip).toHaveBeenCalledWith(
      'Click to stop recording'
    );
  });

  it('showRecordingTray is idempotent', async () => {
    const { showRecordingTray } = await import('@/main/menu/recording-tray');
    showRecordingTray();
    showRecordingTray();
    expect(trayInstances.length).toBe(1);
  });

  it('clicking the tray stops recording and rebuilds menu', async () => {
    mockStopRecordingAction.mockResolvedValue(undefined);
    const { showRecordingTray } = await import('@/main/menu/recording-tray');
    showRecordingTray();
    const tray = trayInstances[0];
    await (tray.handlers['click'] || [])[0]();
    expect(mockStopRecordingAction).toHaveBeenCalled();
    expect(tray.destroy).toHaveBeenCalled();
    expect(mockRebuildTrayMenu).toHaveBeenCalled();
  });

  it('hideRecordingTray destroys the tray', async () => {
    const { showRecordingTray, hideRecordingTray } =
      await import('@/main/menu/recording-tray');
    showRecordingTray();
    hideRecordingTray();
    expect(trayInstances[0].destroy).toHaveBeenCalled();
  });

  it('hideRecordingTray is safe when no tray exists', async () => {
    const { hideRecordingTray } = await import('@/main/menu/recording-tray');
    expect(() => hideRecordingTray()).not.toThrow();
  });
});
