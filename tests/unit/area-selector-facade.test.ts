import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockStartAreaSelection = vi.fn();
const mockUpdateAreaSelectionCallbacks = vi.fn();
const mockConfirmAreaSelection = vi.fn();
const mockConcealAreaSelectorOverlay = vi.fn();
const mockSetAreaSelectorFreeze = vi.fn();
const mockHasVisibleSelectorOverlay = vi.fn();
const mockCancelAreaSelection = vi.fn();
const mockHasPendingSelection = vi.fn();
const mockHideAreaSelector = vi.fn();
const mockShowAreaSelector = vi.fn();
const mockUpdateAreaSelection = vi.fn();
const mockSetAreaSelectionMode = vi.fn();
const mockSetAreaSelectorAspectRatio = vi.fn();

vi.mock('@/main/capture/area-selector/overlay-backend', () => ({
  startAreaSelection: (...a: unknown[]) => mockStartAreaSelection(...a),
  updateAreaSelectionCallbacks: (...a: unknown[]) =>
    mockUpdateAreaSelectionCallbacks(...a),
  confirmAreaSelection: (...a: unknown[]) => mockConfirmAreaSelection(...a),
  concealAreaSelectorOverlay: () => mockConcealAreaSelectorOverlay(),
  setAreaSelectorFreeze: (...a: unknown[]) => mockSetAreaSelectorFreeze(...a),
  hasVisibleSelectorOverlay: () => mockHasVisibleSelectorOverlay(),
  cancelAreaSelection: (...a: unknown[]) => mockCancelAreaSelection(...a),
  hasPendingSelection: () => mockHasPendingSelection(),
  hideAreaSelector: () => mockHideAreaSelector(),
  showAreaSelector: () => mockShowAreaSelector(),
  updateAreaSelection: (...a: unknown[]) => mockUpdateAreaSelection(...a),
  setAreaSelectionMode: (...a: unknown[]) => mockSetAreaSelectionMode(...a),
  setAreaSelectorAspectRatio: (...a: unknown[]) =>
    mockSetAreaSelectorAspectRatio(...a),
}));

const backendSurface = [
  'startAreaSelection',
  'updateAreaSelectionCallbacks',
  'confirmAreaSelection',
  'concealAreaSelectorOverlay',
  'setAreaSelectorFreeze',
  'hasVisibleSelectorOverlay',
  'cancelAreaSelection',
  'hasPendingSelection',
  'hideAreaSelector',
  'showAreaSelector',
  'updateAreaSelection',
  'setAreaSelectionMode',
  'setAreaSelectorAspectRatio',
] as const;

describe('area-selector facade', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('exposes the whole overlay backend surface', async () => {
    const facade = await import('@/main/capture/area-selector');

    backendSurface.forEach(name => {
      expect(typeof facade[name], name).toBe('function');
    });
  });

  it('forwards calls straight through to the overlay backend', async () => {
    const facade = await import('@/main/capture/area-selector');

    await facade.startAreaSelection({ mode: 'area' });
    await facade.confirmAreaSelection({ keepOverlayVisible: true });
    await facade.setAreaSelectorFreeze(false);

    expect(mockStartAreaSelection).toHaveBeenCalledWith({ mode: 'area' });
    expect(mockConfirmAreaSelection).toHaveBeenCalledWith({
      keepOverlayVisible: true,
    });
    expect(mockSetAreaSelectorFreeze).toHaveBeenCalledWith(false);
  });

  it('kills the selector by cancelling the overlay selection silently', async () => {
    const facade = await import('@/main/capture/area-selector');

    await facade.killAreaSelector();

    expect(mockCancelAreaSelection).toHaveBeenCalledWith(true);
  });
});
