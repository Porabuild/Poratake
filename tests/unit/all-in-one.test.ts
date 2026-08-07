import { describe, it, expect, vi, beforeEach } from 'vitest';

let onEventHandler: ((e: string, d?: unknown) => void) | null = null;
const mockDaemonCall = vi.fn();
const mockDaemonOnEvent = vi.fn((cb: (e: string, d?: unknown) => void) => {
  onEventHandler = cb;
});
const mockDaemonOffEvent = vi.fn();
const mockSetAspectRatio = vi.fn();
const mockIsFeatureSupported = vi.fn();

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...a: unknown[]) => mockDaemonCall(...a),
    onEvent: (cb: (e: string, d?: unknown) => void) => mockDaemonOnEvent(cb),
    offEvent: (cb: (e: string, d?: unknown) => void) => mockDaemonOffEvent(cb),
  },
}));

vi.mock('@/main/capture/area-selector', () => ({
  setAreaSelectorAspectRatio: (...a: unknown[]) => mockSetAspectRatio(...a),
}));

vi.mock('@/main/system/capabilities', () => ({
  isFeatureSupported: (...a: unknown[]) => mockIsFeatureSupported(...a),
}));

describe('open-all-in-one', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    onEventHandler = null;
    mockDaemonCall.mockResolvedValue({});
    mockIsFeatureSupported.mockReturnValue(true);
  });

  it('showAllInOneControl calls daemon show with computed position', async () => {
    const m = await import('@/main/capture/all-in-one/open-all-in-one');
    await m.showAllInOneControl({ x: 100, y: 100, width: 400, height: 300 });
    expect(mockDaemonCall).toHaveBeenCalledWith(
      'all-in-one',
      'show',
      expect.objectContaining({
        x: expect.any(Number),
        y: expect.any(Number),
        selectionWidth: 400,
        selectionHeight: 300,
      })
    );
  });

  it('showAllInOneControl works without area', async () => {
    const m = await import('@/main/capture/all-in-one/open-all-in-one');
    await m.showAllInOneControl();
    expect(mockDaemonCall).toHaveBeenCalledWith('all-in-one', 'show', {
      x: 100,
      y: 100,
      recordingEnabled: true,
    });
  });

  it('centers the narrower control when recording is unavailable', async () => {
    mockIsFeatureSupported.mockReturnValue(false);
    const m = await import('@/main/capture/all-in-one/open-all-in-one');
    await m.showAllInOneControl({ x: 100, y: 100, width: 400, height: 300 });
    expect(mockDaemonCall).toHaveBeenCalledWith('all-in-one', 'show', {
      x: 180,
      y: 226,
      selectionWidth: 400,
      selectionHeight: 300,
      recordingEnabled: false,
    });
  });

  it('updateAllInOnePosition calls daemon update', async () => {
    const m = await import('@/main/capture/all-in-one/open-all-in-one');
    await m.updateAllInOnePosition({
      x: 200,
      y: 200,
      width: 100,
      height: 50,
    });
    expect(mockDaemonCall).toHaveBeenCalledWith(
      'all-in-one',
      'update',
      expect.objectContaining({
        x: expect.any(Number),
        y: expect.any(Number),
        selectionWidth: 100,
        selectionHeight: 50,
      })
    );
  });

  it('hideAllInOneControl calls daemon hide and clears selection', async () => {
    const m = await import('@/main/capture/all-in-one/open-all-in-one');
    await m.showAllInOneControl({ x: 0, y: 0, width: 10, height: 10 });
    await m.hideAllInOneControl();
    expect(mockDaemonCall).toHaveBeenCalledWith('all-in-one', 'hide');
    expect(m.getCurrentAreaSelection()).toBeNull();
  });

  it('swallows daemon errors on show/update/hide', async () => {
    mockDaemonCall.mockRejectedValue(new Error('boom'));
    const m = await import('@/main/capture/all-in-one/open-all-in-one');
    await expect(
      m.showAllInOneControl({ x: 0, y: 0, width: 10, height: 10 })
    ).resolves.toBeUndefined();
    await expect(
      m.updateAllInOnePosition({ x: 0, y: 0, width: 10, height: 10 })
    ).resolves.toBeUndefined();
    await expect(m.hideAllInOneControl()).resolves.toBeUndefined();
  });

  describe('event handling', () => {
    it('invokes onClose, onScreenshot, onRecord callbacks', async () => {
      const m = await import('@/main/capture/all-in-one/open-all-in-one');
      const onClose = vi.fn();
      const onScreenshot = vi.fn();
      const onRecord = vi.fn();
      m.setAllInOneCallbacks({ onClose, onScreenshot, onRecord });
      await m.showAllInOneControl({ x: 0, y: 0, width: 10, height: 10 });
      expect(onEventHandler).toBeDefined();
      onEventHandler!('all-in-one:close');
      onEventHandler!('all-in-one:screenshot');
      onEventHandler!('all-in-one:record');
      expect(onClose).toHaveBeenCalled();
      expect(onScreenshot).toHaveBeenCalled();
      expect(onRecord).toHaveBeenCalled();
    });

    it('forwards aspect ratio event to setAreaSelectorAspectRatio', async () => {
      const m = await import('@/main/capture/all-in-one/open-all-in-one');
      await m.showAllInOneControl();
      onEventHandler!('all-in-one:select-aspect-ratio', {
        width: 16,
        height: 9,
        name: '16:9',
      });
      expect(mockSetAspectRatio).toHaveBeenCalledWith({
        name: '16:9',
        width: 16,
        height: 9,
      });
    });

    it('forwards size and size editor events', async () => {
      const m = await import('@/main/capture/all-in-one/open-all-in-one');
      const onUpdateSize = vi.fn();
      const onSizeEditorOpened = vi.fn();
      const onSizeEditorClosed = vi.fn();
      m.setAllInOneCallbacks({
        onUpdateSize,
        onSizeEditorOpened,
        onSizeEditorClosed,
      });
      await m.showAllInOneControl();
      onEventHandler!('all-in-one:update-size', {
        width: 1280,
        height: 720,
      });
      onEventHandler!('all-in-one:size-editor-opened');
      onEventHandler!('all-in-one:size-editor-closed');
      expect(onUpdateSize).toHaveBeenCalledWith({
        width: 1280,
        height: 720,
      });
      expect(onSizeEditorOpened).toHaveBeenCalled();
      expect(onSizeEditorClosed).toHaveBeenCalled();
    });

    it('ignores invalid size events', async () => {
      const m = await import('@/main/capture/all-in-one/open-all-in-one');
      const onUpdateSize = vi.fn();
      m.setAllInOneCallbacks({ onUpdateSize });
      await m.showAllInOneControl();
      onEventHandler!('all-in-one:update-size', { width: '1280' });
      expect(onUpdateSize).not.toHaveBeenCalled();
    });

    it('ignores unknown events', async () => {
      const m = await import('@/main/capture/all-in-one/open-all-in-one');
      const onClose = vi.fn();
      m.setAllInOneCallbacks({ onClose });
      await m.showAllInOneControl();
      onEventHandler!('unknown:event');
      expect(onClose).not.toHaveBeenCalled();
    });
  });
});
