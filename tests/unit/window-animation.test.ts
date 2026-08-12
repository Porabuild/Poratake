import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { animateWindowMove } from '@/main/utils/window-animation';

interface FakeWindow {
  setBounds: ReturnType<typeof vi.fn>;
  setPosition: ReturnType<typeof vi.fn>;
  getBounds: () => { x: number; y: number; width: number; height: number };
  isDestroyed: ReturnType<typeof vi.fn>;
}

function makeFakeWindow(
  initial = { x: 0, y: 0, width: 400, height: 300 }
): FakeWindow {
  return {
    setBounds: vi.fn(),
    setPosition: vi.fn(),
    getBounds: () => initial,
    isDestroyed: vi.fn(() => false),
  };
}

describe('window-animation', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  describe('animateWindowMove', () => {
    it('no-ops when target equals current position', () => {
      const win = makeFakeWindow({ x: 50, y: 50, width: 100, height: 100 });
      animateWindowMove(win as never, { x: 50, y: 50 });
      expect(win.setPosition).not.toHaveBeenCalled();
    });

    it('eases toward the target', () => {
      const win = makeFakeWindow({ x: 0, y: 0, width: 100, height: 100 });
      animateWindowMove(
        win as never,
        { x: 100, y: 100 },
        { steps: 4, duration: 40 }
      );
      for (let i = 0; i < 4; i++) {
        vi.advanceTimersByTime(10);
      }
      const calls = win.setPosition.mock.calls;
      expect(calls.length).toBe(4);
      expect(calls[calls.length - 1]).toEqual([100, 100]);
    });

    it('stops on destroyed window', () => {
      const win = makeFakeWindow({ x: 0, y: 0, width: 100, height: 100 });
      win.isDestroyed = vi.fn(() => true);
      animateWindowMove(
        win as never,
        { x: 100, y: 100 },
        { steps: 4, duration: 40 }
      );
      vi.advanceTimersByTime(100);
      expect(win.setPosition).not.toHaveBeenCalled();
    });
  });
});
