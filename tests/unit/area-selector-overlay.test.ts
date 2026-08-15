import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockStartInteractiveOverlay = vi.fn();
const mockConfirmOverlaySelection = vi.fn();
const mockConcealOverlayHandoff = vi.fn();
const mockHasOverlayHandoff = vi.fn();
const mockCancelOverlaySelection = vi.fn();
const mockUpdateOverlaySelection = vi.fn();
const mockSetOverlayAspectRatio = vi.fn();
const mockSetOverlayVisible = vi.fn();
const mockIsOverlayActive = vi.fn();
const mockSetOverlayPickTargets = vi.fn();
const mockGetOverlayWindowIds = vi.fn();

const mockSelectDisplay = vi.fn();
const mockDisplayFromSelection = vi.fn();
const mockResolveWindowPickTargets = vi.fn();
const mockGetAllDisplays = vi.fn();
const mockGetPrimaryDisplay = vi.fn();
const mockScreenToDipRect = vi.fn();

vi.mock('electron', () => ({
  screen: {
    getAllDisplays: () => mockGetAllDisplays(),
    getPrimaryDisplay: () => mockGetPrimaryDisplay(),
    screenToDipRect: (...a: unknown[]) => mockScreenToDipRect(...a),
  },
}));

vi.mock('@/main/capture/area-overlay', () => ({
  startInteractiveOverlay: (...a: unknown[]) =>
    mockStartInteractiveOverlay(...a),
  confirmOverlaySelection: (...a: unknown[]) =>
    mockConfirmOverlaySelection(...a),
  concealOverlayHandoff: () => mockConcealOverlayHandoff(),
  hasOverlayHandoff: () => mockHasOverlayHandoff(),
  cancelOverlaySelection: (...a: unknown[]) => mockCancelOverlaySelection(...a),
  updateOverlaySelection: (...a: unknown[]) => mockUpdateOverlaySelection(...a),
  setOverlayAspectRatio: (...a: unknown[]) => mockSetOverlayAspectRatio(...a),
  setOverlayVisible: (...a: unknown[]) => mockSetOverlayVisible(...a),
  isOverlayActive: () => mockIsOverlayActive(),
  setOverlayPickTargets: (...a: unknown[]) => mockSetOverlayPickTargets(...a),
  getOverlayWindowIds: () => mockGetOverlayWindowIds(),
  resolveWindowPickTargets: () => mockResolveWindowPickTargets(),
}));

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: vi.fn().mockResolvedValue({}),
    onEvent: vi.fn(),
    offEvent: vi.fn(),
  },
}));

vi.mock('@/main/capture/display-selector', () => ({
  selectDisplay: () => mockSelectDisplay(),
  displayFromSelection: (...a: unknown[]) => mockDisplayFromSelection(...a),
}));

const primary = { id: 7, bounds: { x: 0, y: 0, width: 1920, height: 1080 } };
const secondary = {
  id: 8,
  bounds: { x: 1920, y: 0, width: 1280, height: 1024 },
};

function overlayOptions(): Record<string, unknown> {
  return mockStartInteractiveOverlay.mock.calls[0][0];
}

function flush(): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, 0));
}

function windowPickTargets() {
  return {
    targets: [{ id: 4242, rect: { x: 200, y: 100, width: 800, height: 600 } }],
    names: new Map([[4242, 'Window']]),
    captureRects: new Map([
      [4242, { x: 400, y: 200, width: 1600, height: 1200 }],
    ]),
    prompt: 'Click a window to select it · Esc to cancel',
  };
}

describe('area-selector overlay backend', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockGetAllDisplays.mockReturnValue([primary]);
    mockGetPrimaryDisplay.mockReturnValue(primary);
    mockScreenToDipRect.mockImplementation((_window, bounds) => bounds);
    mockIsOverlayActive.mockReturnValue(false);
    mockGetOverlayWindowIds.mockReturnValue(new Set<number>());
    mockUpdateOverlaySelection.mockReturnValue(true);
    mockStartInteractiveOverlay.mockReturnValue(new Promise(() => {}));
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  it('is the backend the facade exposes on every platform', async () => {
    for (const platform of ['win32', 'darwin'] as const) {
      Object.defineProperty(process, 'platform', { value: platform });
      const [facade, overlay] = await Promise.all([
        import('@/main/capture/area-selector'),
        import('@/main/capture/area-selector/overlay-backend'),
      ]);

      expect(facade.startAreaSelection).toBe(overlay.startAreaSelection);
    }
  });

  it('presets the whole display for single-display screen recording', async () => {
    const m = await import('@/main/capture/area-selector/overlay-backend');
    void m.startAreaSelection({ mode: 'display' });
    await Promise.resolve();

    expect(mockSelectDisplay).not.toHaveBeenCalled();
    expect(overlayOptions().preset).toEqual(primary.bounds);
  });

  it('passes native freeze ownership through the interactive overlay', async () => {
    const release = vi.fn().mockResolvedValue(undefined);
    mockStartInteractiveOverlay.mockResolvedValue({
      display: primary,
      rect: { x: 10, y: 20, width: 300, height: 200 },
      frozen: true,
      release,
    });
    const m = await import('@/main/capture/area-selector/overlay-backend');

    await expect(m.startAreaSelection({ freeze: true })).resolves.toEqual({
      status: 'confirmed',
      x: 10,
      y: 20,
      width: 300,
      height: 200,
      screenId: primary.id,
    });
    expect(overlayOptions().freeze).toBe(true);
    expect(release).toHaveBeenCalled();
  });

  it('offers every display as a pick target when several are connected', async () => {
    mockGetAllDisplays.mockReturnValue([primary, secondary]);

    const m = await import('@/main/capture/area-selector/overlay-backend');
    void m.startAreaSelection({ mode: 'display' });
    await Promise.resolve();

    expect(mockSelectDisplay).not.toHaveBeenCalled();
    expect(overlayOptions().preset).toBeUndefined();
    expect(overlayOptions().pickTargets).toEqual([
      { id: primary.id, rect: primary.bounds },
      { id: secondary.id, rect: secondary.bounds },
    ]);
    expect(overlayOptions().autoConfirm).toBe(false);
    expect(overlayOptions().repeatablePicks).toBe(true);
    expect(overlayOptions().prompt).toBe(
      'Click a display to select it · Esc to cancel'
    );
  });

  it('keeps the overlay visible when the user has to pick a display', async () => {
    mockGetAllDisplays.mockReturnValue([primary, secondary]);

    const m = await import('@/main/capture/area-selector/overlay-backend');
    void m.startAreaSelection({ mode: 'display', visible: false });
    await Promise.resolve();

    expect(overlayOptions().visible).toBe(true);
  });

  it('hides the overlay for a presetted single-display recording', async () => {
    const m = await import('@/main/capture/area-selector/overlay-backend');
    void m.startAreaSelection({ mode: 'display', visible: false });
    await Promise.resolve();

    expect(overlayOptions().visible).toBe(false);
  });

  it('returns null when no windows are available for picking', async () => {
    mockResolveWindowPickTargets.mockResolvedValue(null);

    const m = await import('@/main/capture/area-selector/overlay-backend');

    expect(await m.startAreaSelection({ mode: 'window' })).toBeNull();
    expect(mockStartInteractiveOverlay).not.toHaveBeenCalled();
  });

  it('passes the resolved window pick targets to the overlay', async () => {
    mockResolveWindowPickTargets.mockResolvedValue({
      targets: [{ id: 1, rect: { x: 100, y: 50, width: 400, height: 300 } }],
      names: new Map([[1, 'Window']]),
      captureRects: new Map([[1, { x: 100, y: 50, width: 400, height: 300 }]]),
      prompt: 'Click a window to select it · Esc to cancel',
    });

    const m = await import('@/main/capture/area-selector/overlay-backend');
    void m.startAreaSelection({ mode: 'window' });
    await flush();

    expect(overlayOptions().preset).toBeUndefined();
    expect(overlayOptions().pickTargets).toEqual([
      { id: 1, rect: { x: 100, y: 50, width: 400, height: 300 } },
    ]);
    expect(overlayOptions().prompt).toBe(
      'Click a window to select it · Esc to cancel'
    );
  });

  it('reports the picked window and drops it once the box is dragged', async () => {
    mockResolveWindowPickTargets.mockResolvedValue(windowPickTargets());

    const m = await import('@/main/capture/area-selector/overlay-backend');
    const onSelected = vi.fn();
    const onUpdate = vi.fn();

    void m.startAreaSelection({ mode: 'window', onSelected, onUpdate });
    await flush();

    const { callbacks } = overlayOptions() as {
      callbacks: {
        onSelected: (region: unknown) => void;
        onUpdated: (region: unknown) => void;
      };
    };

    callbacks.onSelected({
      display: primary,
      rect: { x: 200, y: 100, width: 800, height: 600 },
      pickId: 4242,
    });
    expect(onSelected).toHaveBeenCalledWith(
      expect.objectContaining({
        screenId: primary.id,
        windowId: 4242,
        windowName: 'Window',
        windowBounds: { x: 400, y: 200, width: 1600, height: 1200 },
      })
    );

    callbacks.onUpdated({
      display: primary,
      rect: { x: 210, y: 100, width: 800, height: 600 },
    });

    expect(await m.confirmAreaSelection()).toEqual({
      status: 'confirmed',
      x: 210,
      y: 100,
      width: 800,
      height: 600,
      screenId: primary.id,
    });
  });

  it('retargets a running selection at the windows on screen', async () => {
    mockResolveWindowPickTargets.mockResolvedValue(windowPickTargets());
    const m = await import('@/main/capture/area-selector/overlay-backend');
    const onSelected = vi.fn();

    void m.startAreaSelection({ onSelected });
    await flush();

    mockIsOverlayActive.mockReturnValue(true);
    await m.setAreaSelectionMode('window');

    expect(mockSetOverlayPickTargets).toHaveBeenCalledWith(
      [{ id: 4242, rect: { x: 200, y: 100, width: 800, height: 600 } }],
      'Click a window to select it · Esc to cancel',
      false
    );
    expect(m.hasPendingSelection()).toBe(false);

    const { callbacks } = overlayOptions() as {
      callbacks: { onSelected: (region: unknown) => void };
    };
    callbacks.onSelected({
      display: primary,
      rect: { x: 200, y: 100, width: 800, height: 600 },
      pickId: 4242,
    });

    expect(onSelected).toHaveBeenCalledWith(
      expect.objectContaining({ windowId: 4242, windowName: 'Window' })
    );
  });

  it('offers every display when retargeting at the full screen', async () => {
    mockIsOverlayActive.mockReturnValue(true);

    const m = await import('@/main/capture/area-selector/overlay-backend');
    await m.setAreaSelectionMode('display');

    expect(mockSetOverlayPickTargets).toHaveBeenCalledWith(
      [{ id: primary.id, rect: primary.bounds }],
      'Click a display to select it · Esc to cancel',
      true
    );
  });

  it('drops the pick targets when retargeting at a free-form area', async () => {
    mockIsOverlayActive.mockReturnValue(true);

    const m = await import('@/main/capture/area-selector/overlay-backend');
    await m.setAreaSelectionMode('manual');

    expect(mockSetOverlayPickTargets).toHaveBeenCalledWith(null, null, false);
  });

  it('ignores a retarget without a running overlay', async () => {
    const m = await import('@/main/capture/area-selector/overlay-backend');
    await m.setAreaSelectionMode('window');

    expect(mockResolveWindowPickTargets).not.toHaveBeenCalled();
    expect(mockSetOverlayPickTargets).not.toHaveBeenCalled();
  });

  it('never reads a picked display as a window', async () => {
    mockGetAllDisplays.mockReturnValue([primary, secondary]);

    const m = await import('@/main/capture/area-selector/overlay-backend');
    const onSelected = vi.fn();

    void m.startAreaSelection({ mode: 'display', onSelected });
    await Promise.resolve();

    const { callbacks } = overlayOptions() as {
      callbacks: { onSelected: (region: unknown) => void };
    };
    callbacks.onSelected({
      display: secondary,
      rect: secondary.bounds,
      pickId: secondary.id,
    });

    expect(onSelected).toHaveBeenCalledWith(
      expect.objectContaining({
        screenId: secondary.id,
        windowId: undefined,
        windowName: undefined,
      })
    );
  });

  it('tracks the pending selection through the overlay callbacks', async () => {
    const m = await import('@/main/capture/area-selector/overlay-backend');
    const onSelected = vi.fn();
    const onUpdate = vi.fn();

    void m.startAreaSelection({ onSelected, onUpdate });
    await Promise.resolve();

    const { callbacks } = overlayOptions() as {
      callbacks: {
        onSelected: (region: unknown) => void;
        onUpdated: (region: unknown) => void;
      };
    };

    expect(m.hasPendingSelection()).toBe(false);

    callbacks.onSelected({
      display: primary,
      rect: { x: 10, y: 20, width: 300, height: 200 },
    });

    expect(m.hasPendingSelection()).toBe(true);
    expect(onSelected).toHaveBeenCalledWith({
      status: 'selected',
      x: 10,
      y: 20,
      width: 300,
      height: 200,
      screenId: primary.id,
    });
    expect(onUpdate).toHaveBeenCalledTimes(1);

    callbacks.onUpdated({
      display: primary,
      rect: { x: 10, y: 30, width: 300, height: 200 },
    });
    expect(onUpdate).toHaveBeenCalledTimes(2);

    expect(await m.confirmAreaSelection()).toEqual({
      status: 'confirmed',
      x: 10,
      y: 30,
      width: 300,
      height: 200,
      screenId: primary.id,
    });
    expect(mockConfirmOverlaySelection).toHaveBeenCalled();
    expect(m.hasPendingSelection()).toBe(false);
  });

  it('does not confirm without a pending selection', async () => {
    const m = await import('@/main/capture/area-selector/overlay-backend');

    expect(await m.confirmAreaSelection()).toBeNull();
    expect(mockConfirmOverlaySelection).not.toHaveBeenCalled();
  });

  it('hands the overlay off when confirm asks to keep it visible', async () => {
    const m = await import('@/main/capture/area-selector/overlay-backend');

    void m.startAreaSelection({});
    await Promise.resolve();

    const { callbacks } = overlayOptions() as {
      callbacks: {
        onSelected: (region: unknown) => void;
      };
    };
    callbacks.onSelected({
      display: primary,
      rect: { x: 10, y: 20, width: 300, height: 200 },
    });

    expect(await m.confirmAreaSelection({ keepOverlayVisible: true })).toEqual({
      status: 'confirmed',
      x: 10,
      y: 20,
      width: 300,
      height: 200,
      screenId: primary.id,
    });
    expect(mockConfirmOverlaySelection).toHaveBeenCalledWith(true);

    mockHasOverlayHandoff.mockReturnValue(true);
    expect(m.hasVisibleSelectorOverlay()).toBe(true);

    m.concealAreaSelectorOverlay();
    expect(mockConcealOverlayHandoff).toHaveBeenCalledTimes(1);
  });

  it('parks the overlay right away on a plain confirm', async () => {
    const m = await import('@/main/capture/area-selector/overlay-backend');

    void m.startAreaSelection({});
    await Promise.resolve();

    const { callbacks } = overlayOptions() as {
      callbacks: {
        onSelected: (region: unknown) => void;
      };
    };
    callbacks.onSelected({
      display: primary,
      rect: { x: 10, y: 20, width: 300, height: 200 },
    });

    await m.confirmAreaSelection();
    expect(mockConfirmOverlaySelection).toHaveBeenCalledWith(false);
  });

  it('does not replace an active overlay session', async () => {
    const m = await import('@/main/capture/area-selector/overlay-backend');
    const firstOptions = { onSelected: vi.fn() };
    const first = m.startAreaSelection(firstOptions);
    await Promise.resolve();

    mockIsOverlayActive.mockReturnValue(true);

    expect(await m.startAreaSelection({ onSelected: vi.fn() })).toBeNull();
    expect(mockStartInteractiveOverlay).toHaveBeenCalledTimes(1);

    void first;
  });

  it('forwards visibility, updates and aspect ratios to the overlay', async () => {
    mockIsOverlayActive.mockReturnValue(true);
    const m = await import('@/main/capture/area-selector/overlay-backend');

    await m.hideAreaSelector();
    expect(mockSetOverlayVisible).toHaveBeenCalledWith(false);

    await m.showAreaSelector();
    expect(mockSetOverlayVisible).toHaveBeenCalledWith(true);

    await m.cancelAreaSelection(true);
    expect(mockCancelOverlaySelection).toHaveBeenCalledWith(true);

    const bounds = { x: 1, y: 2, width: 3, height: 4 };
    expect(await m.updateAreaSelection(bounds)).toBe(true);
    expect(mockUpdateOverlaySelection).toHaveBeenCalledWith(bounds);

    await m.setAreaSelectorAspectRatio({ name: '16:9', width: 16, height: 9 });
    expect(mockSetOverlayAspectRatio).toHaveBeenCalledWith(16 / 9);

    await m.setAreaSelectorAspectRatio({ name: 'Free', width: 0, height: 0 });
    expect(mockSetOverlayAspectRatio).toHaveBeenCalledWith(null);
  });

  it('skips updates when no overlay is active', async () => {
    const m = await import('@/main/capture/area-selector/overlay-backend');

    expect(
      await m.updateAreaSelection({ x: 1, y: 2, width: 3, height: 4 })
    ).toBe(false);
    expect(mockUpdateOverlaySelection).not.toHaveBeenCalled();
  });
});
