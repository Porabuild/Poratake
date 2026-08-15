import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockSetOverlayToolbar = vi.fn();

vi.mock('@/main/capture/area-overlay', () => ({
  setOverlayToolbar: (...a: unknown[]) => mockSetOverlayToolbar(...a),
}));

describe('open-all-in-one', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it('showAllInOneControl records the selection', async () => {
    const m = await import('@/main/capture/all-in-one/open-all-in-one');
    await m.showAllInOneControl({ x: 10, y: 20, width: 300, height: 200 });
    expect(m.getCurrentAreaSelection()).toEqual({
      x: 10,
      y: 20,
      width: 300,
      height: 200,
    });
  });

  it('showAllInOneControl clears the selection without an area', async () => {
    const m = await import('@/main/capture/all-in-one/open-all-in-one');
    await m.showAllInOneControl({ x: 10, y: 20, width: 300, height: 200 });
    await m.showAllInOneControl();
    expect(m.getCurrentAreaSelection()).toBeNull();
  });

  it('updateAllInOnePosition tracks bounds', async () => {
    const m = await import('@/main/capture/all-in-one/open-all-in-one');
    await m.updateAllInOnePosition({ x: 5, y: 5, width: 50, height: 60 });
    expect(m.getCurrentAreaSelection()).toEqual({
      x: 5,
      y: 5,
      width: 50,
      height: 60,
    });
  });

  it('hideAllInOneControl clears the overlay toolbar and selection', async () => {
    const m = await import('@/main/capture/all-in-one/open-all-in-one');
    await m.showAllInOneControl({ x: 0, y: 0, width: 10, height: 10 });
    await m.hideAllInOneControl();
    expect(mockSetOverlayToolbar).toHaveBeenCalledWith(null);
    expect(m.getCurrentAreaSelection()).toBeNull();
  });
});
