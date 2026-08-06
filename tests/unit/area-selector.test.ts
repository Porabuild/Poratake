import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockDaemonCall = vi.fn();
const mockOnEvent = vi.fn();
const mockOffEvent = vi.fn();
const mockSelectDisplay = vi.fn();
const mockSelectWindow = vi.fn();
const mockGetAllDisplays = vi.fn();
const mockGetPrimaryDisplay = vi.fn();

vi.mock('electron', () => ({
  screen: {
    getAllDisplays: () => mockGetAllDisplays(),
    getPrimaryDisplay: () => mockGetPrimaryDisplay(),
  },
}));

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...a: unknown[]) => mockDaemonCall(...a),
    onEvent: (...a: unknown[]) => mockOnEvent(...a),
    offEvent: (...a: unknown[]) => mockOffEvent(...a),
  },
}));

vi.mock('@/main/capture/display-selector', () => ({
  selectDisplay: () => mockSelectDisplay(),
}));

vi.mock('@/main/capture/window-selector', () => ({
  selectWindow: () => mockSelectWindow(),
}));

describe('area-selector', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockDaemonCall.mockResolvedValue({});
    mockGetPrimaryDisplay.mockReturnValue({
      id: 1,
      bounds: { x: 0, y: 0, width: 1920, height: 1080 },
    });
    mockGetAllDisplays.mockReturnValue([
      { id: 1, bounds: { x: 0, y: 0, width: 1920, height: 1080 } },
    ]);
  });

  describe('hasPendingSelection', () => {
    it('returns false initially', async () => {
      const m = await import('@/main/capture/area-selector');
      expect(m.hasPendingSelection()).toBe(false);
    });
  });

  describe('cancelAreaSelection', () => {
    it('calls daemon cancel', async () => {
      const m = await import('@/main/capture/area-selector');
      await m.cancelAreaSelection();
      expect(mockDaemonCall).toHaveBeenCalledWith('area-selector', 'cancel');
    });

    it('swallows daemon errors', async () => {
      mockDaemonCall.mockRejectedValue(new Error('boom'));
      const m = await import('@/main/capture/area-selector');
      await expect(m.cancelAreaSelection()).resolves.toBeUndefined();
    });
  });

  describe('confirmAreaSelection', () => {
    it('returns null when no pending selection', async () => {
      const m = await import('@/main/capture/area-selector');
      expect(await m.confirmAreaSelection()).toBeNull();
    });
  });

  describe('hideAreaSelector/showAreaSelector', () => {
    it('hideAreaSelector calls daemon hide', async () => {
      const m = await import('@/main/capture/area-selector');
      await m.hideAreaSelector();
      expect(mockDaemonCall).toHaveBeenCalledWith('area-selector', 'hide');
    });

    it('showAreaSelector calls daemon show', async () => {
      const m = await import('@/main/capture/area-selector');
      await m.showAreaSelector();
      expect(mockDaemonCall).toHaveBeenCalledWith('area-selector', 'show');
    });

    it('swallows hide errors', async () => {
      mockDaemonCall.mockRejectedValue(new Error('boom'));
      const m = await import('@/main/capture/area-selector');
      await expect(m.hideAreaSelector()).resolves.toBeUndefined();
    });

    it('swallows show errors', async () => {
      mockDaemonCall.mockRejectedValue(new Error('boom'));
      const m = await import('@/main/capture/area-selector');
      await expect(m.showAreaSelector()).resolves.toBeUndefined();
    });
  });

  describe('killAreaSelector', () => {
    it('cancels selection silently', async () => {
      const m = await import('@/main/capture/area-selector');
      await m.killAreaSelector();
      expect(mockDaemonCall).toHaveBeenCalledWith('area-selector', 'cancel');
    });
  });

  describe('updateAreaSelection', () => {
    it('forwards bounds to daemon update', async () => {
      const m = await import('@/main/capture/area-selector');
      const result = await m.updateAreaSelection({
        x: 100,
        y: 50,
        width: 800,
        height: 600,
      });
      expect(result).toBe(true);
      expect(mockDaemonCall).toHaveBeenCalledWith('area-selector', 'update', {
        x: 100,
        y: 50,
        width: 800,
        height: 600,
      });
    });

    it('returns false on daemon error', async () => {
      mockDaemonCall.mockRejectedValue(new Error('boom'));
      const m = await import('@/main/capture/area-selector');
      await expect(
        m.updateAreaSelection({
          x: 100,
          y: 50,
          width: 800,
          height: 600,
        })
      ).resolves.toBe(false);
    });
  });

  describe('setAreaSelectorAspectRatio', () => {
    it('forwards aspect ratio to daemon', async () => {
      const m = await import('@/main/capture/area-selector');
      await m.setAreaSelectorAspectRatio({
        id: 'r1',
        label: '16:9',
        width: 16,
        height: 9,
      } as never);
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'area-selector',
        'setAspectRatio',
        { width: 16, height: 9 }
      );
    });

    it('swallows errors', async () => {
      mockDaemonCall.mockRejectedValue(new Error('boom'));
      const m = await import('@/main/capture/area-selector');
      await expect(
        m.setAreaSelectorAspectRatio({
          id: 'r1',
          label: '16:9',
          width: 16,
          height: 9,
        } as never)
      ).resolves.toBeUndefined();
    });
  });

  function fireAllHandlersAfterStart(event: string): void {
    mockDaemonCall.mockImplementation(async () => {
      setImmediate(() => {
        for (const call of mockOnEvent.mock.calls) {
          (call[0] as (e: string, d: unknown) => void)(event, null);
        }
      });
    });
  }

  describe('startAreaSelection display mode', () => {
    it('uses primary display when only one display', async () => {
      fireAllHandlersAfterStart('area-selector:cancelled');
      const m = await import('@/main/capture/area-selector');
      const result = await m.startAreaSelection({ mode: 'display' });
      expect(result).toBeNull();
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'area-selector',
        'start',
        expect.objectContaining({ fullscreen: true, displayId: 1 })
      );
    });

    it('prompts user when multiple displays present', async () => {
      mockGetAllDisplays.mockReturnValue([
        { id: 1, bounds: { x: 0, y: 0, width: 1920, height: 1080 } },
        { id: 2, bounds: { x: 1920, y: 0, width: 1920, height: 1080 } },
      ]);
      mockSelectDisplay.mockResolvedValue({
        status: 'selected',
        screenId: 2,
      });
      fireAllHandlersAfterStart('area-selector:cancelled');
      const m = await import('@/main/capture/area-selector');
      const result = await m.startAreaSelection({ mode: 'display' });
      expect(result).toBeNull();
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'area-selector',
        'start',
        expect.objectContaining({ displayId: 2 })
      );
    });

    it('returns null when display selection cancelled', async () => {
      mockGetAllDisplays.mockReturnValue([
        { id: 1, bounds: { x: 0, y: 0, width: 1920, height: 1080 } },
        { id: 2, bounds: { x: 1920, y: 0, width: 1920, height: 1080 } },
      ]);
      mockSelectDisplay.mockResolvedValue({ status: 'cancelled' });
      const m = await import('@/main/capture/area-selector');
      const result = await m.startAreaSelection({ mode: 'display' });
      expect(result).toBeNull();
    });
  });

  describe('startAreaSelection window mode', () => {
    it('returns null when window selection cancelled', async () => {
      mockSelectWindow.mockResolvedValue({ status: 'cancelled' });
      const m = await import('@/main/capture/area-selector');
      const result = await m.startAreaSelection({ mode: 'window' });
      expect(result).toBeNull();
    });

    it('returns null on window selection error', async () => {
      mockSelectWindow.mockResolvedValue({ status: 'error' });
      const m = await import('@/main/capture/area-selector');
      const result = await m.startAreaSelection({ mode: 'window' });
      expect(result).toBeNull();
    });
  });

  describe('startAreaSelection preset', () => {
    it('starts with preset bounds', async () => {
      fireAllHandlersAfterStart('area-selector:cancelled');
      const m = await import('@/main/capture/area-selector');
      const result = await m.startAreaSelection({
        preset: { x: 100, y: 50, width: 200, height: 150 },
      });
      expect(result).toBeNull();
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'area-selector',
        'start',
        expect.objectContaining({
          presetX: 100,
          presetY: 50,
          presetWidth: 200,
          presetHeight: 150,
        })
      );
    });
  });

  describe('startAreaSelection manual', () => {
    it('returns null on daemon error during start', async () => {
      mockDaemonCall.mockRejectedValue(new Error('boom'));
      const m = await import('@/main/capture/area-selector');
      const result = await m.startAreaSelection();
      expect(result).toBeNull();
    });
  });

  describe('handleDaemonEvent (via callbacks)', () => {
    it('selected event fires onSelected and onUpdate', async () => {
      const handlers: Array<(e: string, d: unknown) => void> = [];
      mockOnEvent.mockImplementation((cb: (e: string, d: unknown) => void) => {
        handlers.push(cb);
      });

      fireAllHandlersAfterStart('area-selector:cancelled');
      const onSelected = vi.fn();
      const onUpdate = vi.fn();
      const m = await import('@/main/capture/area-selector');

      mockDaemonCall.mockImplementation(async () => {
        setImmediate(() => {
          handlers.forEach(h =>
            h('area-selector:selected', { x: 1, y: 2, width: 3, height: 4 })
          );
          handlers.forEach(h => h('area-selector:cancelled', null));
        });
      });
      await m.startAreaSelection({ onSelected, onUpdate });
      expect(onSelected).toHaveBeenCalled();
      expect(onUpdate).toHaveBeenCalled();
    });

    it('updated event fires onUpdate only', async () => {
      const handlers: Array<(e: string, d: unknown) => void> = [];
      mockOnEvent.mockImplementation((cb: (e: string, d: unknown) => void) => {
        handlers.push(cb);
      });
      const onSelected = vi.fn();
      const onUpdate = vi.fn();
      const m = await import('@/main/capture/area-selector');

      mockDaemonCall.mockImplementation(async () => {
        setImmediate(() => {
          handlers.forEach(h =>
            h('area-selector:updated', { x: 5, y: 6, width: 7, height: 8 })
          );
          handlers.forEach(h => h('area-selector:cancelled', null));
        });
      });
      await m.startAreaSelection({ onSelected, onUpdate });
      expect(onSelected).not.toHaveBeenCalled();
      expect(onUpdate).toHaveBeenCalled();
    });

    it('confirmAreaSelection returns current selection', async () => {
      const handlers: Array<(e: string, d: unknown) => void> = [];
      mockOnEvent.mockImplementation((cb: (e: string, d: unknown) => void) => {
        handlers.push(cb);
      });
      const m = await import('@/main/capture/area-selector');

      mockDaemonCall.mockImplementation(async (_module, method) => {
        if (method === 'start') {
          setImmediate(() => {
            handlers.forEach(h =>
              h('area-selector:selected', {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
              })
            );
          });
        }
        return {};
      });

      const promise = m.startAreaSelection();
      await new Promise(r => setImmediate(r));
      await new Promise(r => setImmediate(r));

      expect(m.hasPendingSelection()).toBe(true);
      const result = await m.confirmAreaSelection();
      expect(result?.x).toBe(1);

      // resolve the original promise
      handlers.forEach(h => h('area-selector:cancelled', null));
      await promise;
    });

    it('confirmAreaSelection returns null on daemon error', async () => {
      const handlers: Array<(e: string, d: unknown) => void> = [];
      mockOnEvent.mockImplementation((cb: (e: string, d: unknown) => void) => {
        handlers.push(cb);
      });
      const m = await import('@/main/capture/area-selector');

      mockDaemonCall.mockImplementationOnce(async () => {
        setImmediate(() => {
          handlers.forEach(h =>
            h('area-selector:selected', {
              x: 1,
              y: 2,
              width: 3,
              height: 4,
            })
          );
        });
        return {};
      });

      const promise = m.startAreaSelection();
      await new Promise(r => setImmediate(r));
      await new Promise(r => setImmediate(r));

      mockDaemonCall.mockRejectedValueOnce(new Error('boom'));
      const result = await m.confirmAreaSelection();
      expect(result).toBeNull();

      handlers.forEach(h => h('area-selector:cancelled', null));
      await promise;
    });
  });

  describe('updateAreaSelectionCallbacks', () => {
    it('replaces existing callbacks', async () => {
      const m = await import('@/main/capture/area-selector');
      m.updateAreaSelectionCallbacks({ onSelected: vi.fn() });
      m.updateAreaSelectionCallbacks({ onSelected: vi.fn() });
      expect(mockOffEvent).toHaveBeenCalled();
    });
  });
});
