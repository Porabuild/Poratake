// @vitest-environment happy-dom
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AreaOverlayParams } from '@/types/area-overlay';

vi.mock('@/renderer/components/area-overlay/all-in-one-toolbar', async () => {
  const { createElement: createMockElement } = await import('react');
  return {
    default: () =>
      createMockElement('div', { 'data-testid': 'toolbar' }, 'Toolbar'),
  };
});

vi.mock('@/renderer/components/area-overlay/crosshair-guides', () => ({
  default: () => null,
}));

vi.mock('@/renderer/components/area-overlay/selection-frame', async () => {
  const { createElement: createMockElement } = await import('react');
  return {
    default: () =>
      createMockElement('div', { 'data-testid': 'selection' }, 'Selection'),
  };
});

vi.mock('@/renderer/components/area-overlay/selection-scrim', () => ({
  default: () => null,
}));

type IpcHandler = (...args: never[]) => void;

const handlers = new Map<string, Set<IpcHandler>>();
const send = vi.fn();
const invoke = vi.fn();

function emit(channel: string, ...args: unknown[]): void {
  for (const handler of handlers.get(channel) ?? []) {
    handler(...(args as never[]));
  }
}

function createParams(sessionId: number): AreaOverlayParams {
  return {
    sessionId,
    displayId: 1,
    imageUrl: null,
    interactive: true,
    autoConfirm: false,
    repeatablePicks: false,
    showPrompt: true,
    rect: sessionId === 1 ? { x: 10, y: 10, width: 100, height: 80 } : null,
    aspectRatio: null,
    toolbar: {
      kind: 'all-in-one',
      recordingEnabled: true,
      ocrEnabled: true,
      activeMode: 'screenshot',
      activeTarget: 'area',
    },
    pickTargets:
      sessionId === 1 ? null : [{ id: 2, x: 0, y: 0, width: 200, height: 200 }],
    prompt: sessionId === 1 ? 'Session one' : 'Session two',
  };
}

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  handlers.clear();
  send.mockClear();
  invoke.mockReset();
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
  window.appPlatform = 'win32';
  window.ipcRenderer = {
    send,
    invoke,
    on: vi.fn((channel: string, handler: IpcHandler) => {
      const channelHandlers = handlers.get(channel) ?? new Set<IpcHandler>();
      channelHandlers.add(handler);
      handlers.set(channel, channelHandlers);
    }),
    off: vi.fn((channel: string, handler: IpcHandler) => {
      handlers.get(channel)?.delete(handler);
    }),
  } as never;
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('AreaOverlayWindow sessions', () => {
  it('resets session-owned state and announces each keyed frame once', async () => {
    const { default: AreaOverlayWindow } =
      await import('@/renderer/windows/area-overlay-window');

    act(() => {
      root.render(
        createElement(AreaOverlayWindow, { params: createParams(1) })
      );
    });

    expect(container.querySelector('[data-testid="selection"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="toolbar"]')).not.toBeNull();

    act(() => emit('area-overlay:handoff'));
    expect(container.querySelector('[data-testid="selection"]')).toBeNull();
    expect(container.querySelector('[data-testid="toolbar"]')).toBeNull();

    act(() => {
      root.render(
        createElement(AreaOverlayWindow, { params: createParams(2) })
      );
    });

    expect(container.querySelector('[data-testid="selection"]')).toBeNull();
    expect(container.querySelector('[data-testid="toolbar"]')).not.toBeNull();
    expect(container.textContent).toContain('Session two');
    expect(
      send.mock.calls.filter(([channel]) => channel === 'area-overlay:ready')
    ).toEqual([
      ['area-overlay:ready', 1],
      ['area-overlay:ready', 2],
    ]);
  });

  it('renders the last pointer position when the color frame becomes ready', async () => {
    const drawImage = vi.fn();
    const context = {
      clearRect: vi.fn(),
      createImageData: (width: number, height: number) => ({
        data: new Uint8ClampedArray(width * height * 4),
      }),
      drawImage,
      getImageData: () => ({ data: new Uint8ClampedArray(20 * 20 * 4) }),
      imageSmoothingEnabled: true,
      putImageData: vi.fn(),
    };
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(
      context as never
    );
    vi.stubGlobal(
      'Image',
      class {
        naturalWidth = 20;
        naturalHeight = 20;
        src = '';
        decode = () => Promise.resolve();
      }
    );
    let renderFrame!: FrameRequestCallback;
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      renderFrame = callback;
      return 1;
    });
    vi.stubGlobal('cancelAnimationFrame', vi.fn());

    let resolveFrame!: (frame: { url: string }) => void;
    invoke.mockReturnValue(
      new Promise(resolve => {
        resolveFrame = resolve;
      })
    );
    const { default: ColorPicker } =
      await import('@/renderer/components/area-overlay/color-picker');

    act(() => {
      root.render(
        createElement(ColorPicker, {
          onPick: vi.fn(),
          onCancel: vi.fn(),
        })
      );
    });
    act(() => {
      window.dispatchEvent(
        new MouseEvent('mousemove', { clientX: 8, clientY: 9 })
      );
      renderFrame(0);
    });

    await act(async () => {
      resolveFrame({ url: 'file:///tmp/frame.png' });
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(drawImage).toHaveBeenCalledTimes(2);
  });
});
