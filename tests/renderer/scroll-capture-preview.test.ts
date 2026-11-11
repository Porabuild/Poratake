import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, createElement } from 'react';
import type { Root } from 'react-dom/client';
import { Window } from 'happy-dom';
import { EMPTY_SCROLL_CAPTURE_STATE } from '@/types/scroll-capture';

vi.mock('@/renderer/windows/area-overlay-window', () => ({
  default: () => null,
}));

let browser: Window;
let root: Root;
let container: HTMLElement;
let handlers: Record<string, (...args: unknown[]) => void>;

beforeEach(async () => {
  browser = new Window();
  vi.stubGlobal('window', browser);
  vi.stubGlobal('document', browser.document);
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  handlers = {};
  window.ipcRenderer = {
    on: (channel: string, handler: (...args: unknown[]) => void) => {
      handlers[channel] = handler;
      return () => delete handlers[channel];
    },
  } as never;
  container = document.createElement('div');
  document.body.append(container);
  const { createRoot } = await import('react-dom/client');
  root = createRoot(container);
  const { default: ScrollCaptureOverlayWindow } =
    await import('@/renderer/windows/scroll-capture-overlay-window');
  await act(() =>
    root.render(
      createElement(ScrollCaptureOverlayWindow, { params: {} as never })
    )
  );
});

afterEach(async () => {
  await act(() => root.unmount());
  await browser.happyDOM.close();
  vi.unstubAllGlobals();
});

describe('scroll capture preview', () => {
  it.each([
    {
      displayY: 0,
      displayHeight: 1080,
      areaY: 100,
      expectedTop: 100,
      expectedHeight: 360,
    },
    {
      displayY: 0,
      displayHeight: 1080,
      areaY: 950,
      expectedTop: 720,
      expectedHeight: 360,
    },
    {
      displayY: 200,
      displayHeight: 900,
      areaY: 1010,
      expectedTop: 540,
      expectedHeight: 360,
    },
    {
      displayY: 0,
      displayHeight: 200,
      areaY: 150,
      expectedTop: 0,
      expectedHeight: 200,
    },
  ])(
    'keeps the preview visible on the selected display at $areaY',
    async ({ displayY, displayHeight, areaY, expectedTop, expectedHeight }) => {
      await act(() => {
        handlers['scroll-capture:begin'](undefined, {
          displayId: 7,
          displayBounds: {
            x: 0,
            y: displayY,
            width: 1920,
            height: displayHeight,
          },
          area: { x: 100, y: areaY, width: 200, height: 20, displayId: 7 },
        });
        handlers['scroll-capture:update'](undefined, {
          ...EMPTY_SCROLL_CAPTURE_STATE,
          cursorOutside: true,
          preview: 'fixture',
          previewWidth: 240,
          previewHeight: 720,
        });
      });

      const preview = container.querySelector('img')?.parentElement;
      expect(preview?.style.top).toBe(`${expectedTop}px`);
      expect(preview?.style.height).toBe(`${expectedHeight}px`);
      expect(container.querySelector('span')?.parentElement?.style.top).toBe(
        `${areaY - displayY}px`
      );
    }
  );

  it('moves the preview only when its growing image needs more space', async () => {
    await act(() => {
      handlers['scroll-capture:begin'](undefined, {
        displayId: 7,
        displayBounds: { x: 0, y: 0, width: 1920, height: 1080 },
        area: { x: 100, y: 950, width: 200, height: 20, displayId: 7 },
      });
      handlers['scroll-capture:update'](undefined, {
        ...EMPTY_SCROLL_CAPTURE_STATE,
        preview: 'fixture',
        previewWidth: 240,
        previewHeight: 100,
      });
    });
    expect(container.querySelector('img')?.parentElement?.style.top).toBe(
      '950px'
    );

    await act(() => {
      handlers['scroll-capture:update'](undefined, {
        ...EMPTY_SCROLL_CAPTURE_STATE,
        preview: 'fixture',
        previewWidth: 240,
        previewHeight: 720,
      });
    });
    expect(container.querySelector('img')?.parentElement?.style.top).toBe(
      '720px'
    );
  });

  it('fits the preview to a display narrower than the usual preview width', async () => {
    await act(() => {
      handlers['scroll-capture:begin'](undefined, {
        displayId: 7,
        displayBounds: { x: 0, y: 0, width: 200, height: 180 },
        area: { x: 100, y: 80, width: 80, height: 20, displayId: 7 },
      });
      handlers['scroll-capture:update'](undefined, {
        ...EMPTY_SCROLL_CAPTURE_STATE,
        preview: 'fixture',
        previewWidth: 400,
        previewHeight: 200,
      });
    });
    const preview = container.querySelector('img')?.parentElement;
    expect(preview?.style.left).toBe('0px');
    expect(preview?.style.top).toBe('80px');
    expect(preview?.style.width).toBe('200px');
    expect(preview?.style.height).toBe('100px');

    await act(() => {
      handlers['scroll-capture:update'](undefined, {
        ...EMPTY_SCROLL_CAPTURE_STATE,
        preview: 'fixture',
        previewWidth: 200,
        previewHeight: 800,
      });
    });
    expect(preview?.style.top).toBe('0px');
    expect(preview?.style.height).toBe('180px');
    expect(container.querySelector('img')?.className).toContain(
      'object-bottom'
    );
  });
});
