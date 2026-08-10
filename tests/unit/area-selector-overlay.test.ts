import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockStartInteractiveOverlay = vi.fn();
const mockConfirmOverlaySelection = vi.fn();
const mockCancelOverlaySelection = vi.fn();
const mockUpdateOverlaySelection = vi.fn();
const mockSetOverlayAspectRatio = vi.fn();
const mockSetOverlayVisible = vi.fn();
const mockIsOverlayActive = vi.fn();

const mockSelectDisplay = vi.fn();
const mockDisplayFromSelection = vi.fn();
const mockSelectWindow = vi.fn();
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
  confirmOverlaySelection: () => mockConfirmOverlaySelection(),
  cancelOverlaySelection: (...a: unknown[]) => mockCancelOverlaySelection(...a),
  updateOverlaySelection: (...a: unknown[]) => mockUpdateOverlaySelection(...a),
  setOverlayAspectRatio: (...a: unknown[]) => mockSetOverlayAspectRatio(...a),
  setOverlayVisible: (...a: unknown[]) => mockSetOverlayVisible(...a),
  isOverlayActive: () => mockIsOverlayActive(),
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

vi.mock('@/main/capture/window-selector', () => ({
  selectWindow: () => mockSelectWindow(),
}));

const primary = { id: 7, bounds: { x: 0, y: 0, width: 1920, height: 1080 } };
const secondary = {
  id: 8,
  bounds: { x: 1920, y: 0, width: 1280, height: 1024 },
};

function overlayOptions(): Record<string, unknown> {
  return mockStartInteractiveOverlay.mock.calls[0][0];
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
    mockUpdateOverlaySelection.mockReturnValue(true);
    mockStartInteractiveOverlay.mockReturnValue(new Promise(() => {}));
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  it('is the backend the facade exposes on Windows', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    const [facade, overlay] = await Promise.all([
      import('@/main/capture/area-selector'),
      import('@/main/capture/area-selector/overlay-backend'),
    ]);

    expect(facade.startAreaSelection).toBe(overlay.startAreaSelection);
  });

  it('falls back to the daemon backend elsewhere', async () => {
    Object.defineProperty(process, 'platform', { value: 'darwin' });
    const [facade, daemonBackend] = await Promise.all([
      import('@/main/capture/area-selector'),
      import('@/main/capture/area-selector/daemon-backend'),
    ]);

    expect(facade.startAreaSelection).toBe(daemonBackend.startAreaSelection);
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
    });
    expect(overlayOptions().freeze).toBe(true);
    expect(release).toHaveBeenCalled();
  });

  it('presets the chosen display when several are connected', async () => {
    mockGetAllDisplays.mockReturnValue([primary, secondary]);
    mockSelectDisplay.mockResolvedValue({ status: 'selected', screenId: 2 });
    mockDisplayFromSelection.mockReturnValue(secondary);

    const m = await import('@/main/capture/area-selector/overlay-backend');
    void m.startAreaSelection({ mode: 'display' });
    await Promise.resolve();
    await Promise.resolve();

    expect(overlayOptions().preset).toEqual(secondary.bounds);
  });

  it('returns null when the display picker is dismissed', async () => {
    mockGetAllDisplays.mockReturnValue([primary, secondary]);
    mockSelectDisplay.mockResolvedValue({ status: 'cancelled' });
    mockDisplayFromSelection.mockReturnValue(null);

    const m = await import('@/main/capture/area-selector/overlay-backend');

    expect(await m.startAreaSelection({ mode: 'display' })).toBeNull();
    expect(mockStartInteractiveOverlay).not.toHaveBeenCalled();
  });

  it('converts window bounds to DIP before presetting them', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
    mockSelectWindow.mockResolvedValue({
      status: 'selected',
      bounds: { x: 200, y: 100, width: 800, height: 600 },
    });
    mockScreenToDipRect.mockReturnValue({
      x: 100,
      y: 50,
      width: 400,
      height: 300,
    });

    const m = await import('@/main/capture/area-selector/overlay-backend');
    void m.startAreaSelection({ mode: 'window' });
    await Promise.resolve();
    await Promise.resolve();

    expect(mockScreenToDipRect).toHaveBeenCalled();
    expect(overlayOptions().preset).toEqual({
      x: 100,
      y: 50,
      width: 400,
      height: 300,
    });
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

    callbacks.onSelected({ rect: { x: 10, y: 20, width: 300, height: 200 } });

    expect(m.hasPendingSelection()).toBe(true);
    expect(onSelected).toHaveBeenCalledWith({
      status: 'selected',
      x: 10,
      y: 20,
      width: 300,
      height: 200,
    });
    expect(onUpdate).toHaveBeenCalledTimes(1);

    callbacks.onUpdated({ rect: { x: 10, y: 30, width: 300, height: 200 } });
    expect(onUpdate).toHaveBeenCalledTimes(2);

    expect(await m.confirmAreaSelection()).toEqual({
      status: 'confirmed',
      x: 10,
      y: 30,
      width: 300,
      height: 200,
    });
    expect(mockConfirmOverlaySelection).toHaveBeenCalled();
    expect(m.hasPendingSelection()).toBe(false);
  });

  it('does not confirm without a pending selection', async () => {
    const m = await import('@/main/capture/area-selector/overlay-backend');

    expect(await m.confirmAreaSelection()).toBeNull();
    expect(mockConfirmOverlaySelection).not.toHaveBeenCalled();
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
