import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('react', () => ({
  useCallback: <T extends (...args: never[]) => unknown>(callback: T) =>
    callback,
  useEffect: vi.fn(),
  useLayoutEffect: (callback: () => void) => callback(),
  useRef: <T>(value: T) => ({ current: value }),
  useState: <T>(value: T | (() => T)) => [
    typeof value === 'function' ? (value as () => T)() : value,
    vi.fn(),
  ],
}));

const target = { id: 7, x: 0, y: 0, width: 1000, height: 800 };

function options(repeatablePicks: boolean) {
  return {
    interactive: true,
    initialRect: null,
    initialAspectRatio: null,
    pickTargets: [target],
    repeatablePicks,
    onSelected: vi.fn(),
    onUpdated: vi.fn(),
    onDiscarded: vi.fn(),
  };
}

describe('useAreaSelection pick behavior', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    vi.stubGlobal('window', {
      innerWidth: 1000,
      innerHeight: 800,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    });
  });

  it('allows a display target to be selected repeatedly', async () => {
    const selectionOptions = options(true);
    const useAreaSelection = (
      await import('@/renderer/hooks/use-area-selection')
    ).default;
    const selection = useAreaSelection(selectionOptions);
    const event = { button: 0, clientX: 100, clientY: 100 };

    selection.startDrag(event as never);
    selection.startDrag(event as never);

    expect(selectionOptions.onSelected).toHaveBeenCalledTimes(2);
    expect(selectionOptions.onSelected).toHaveBeenLastCalledWith(
      { x: 0, y: 0, width: 1000, height: 800 },
      7
    );
  });

  it('keeps a window target final after its first selection', async () => {
    const selectionOptions = options(false);
    const useAreaSelection = (
      await import('@/renderer/hooks/use-area-selection')
    ).default;
    const selection = useAreaSelection(selectionOptions);
    const event = { button: 0, clientX: 100, clientY: 100 };

    selection.startDrag(event as never);
    selection.startDrag(event as never);

    expect(selectionOptions.onSelected).toHaveBeenCalledTimes(1);
  });
});
