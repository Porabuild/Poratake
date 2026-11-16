import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { Window } from 'happy-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import RedactOverlay from '@/renderer/components/editor/redact/redact-overlay';
import type { RedactAnnotation } from '@/types/editor';

let browser: Window;
let container: HTMLDivElement;
let root: Root;
let fillRect: ReturnType<typeof vi.fn>;

beforeEach(() => {
  browser = new Window();
  vi.stubGlobal('window', browser);
  vi.stubGlobal('document', browser.document);
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  fillRect = vi.fn();
  const context = { clearRect: vi.fn(), drawImage: vi.fn(), fillRect };
  vi.spyOn(browser.HTMLCanvasElement.prototype, 'getContext').mockReturnValue(
    context as unknown as CanvasRenderingContext2D
  );
  container = browser.document.createElement('div');
  browser.document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(() => root.unmount());
  await browser.happyDOM.close();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('screenshot blackout preview', () => {
  it.each([
    [1, 8.8, 3.8, 12.3, 12.3, 0, 0, 8, 3, 14, 14],
    [2, 8.8, 3.8, 12.3, 12.3, 0, 0, 17, 7, 26, 26],
    [1, 21.1, 16.1, -12.3, -12.3, 0, 0, 8, 3, 14, 14],
    [1, -2.5, 3.8, 12.3, 12.3, 0, 0, 0, 3, 10, 14],
    [1, 8.8, 3.8, 12.3, 12.3, 0.25, 0.25, 9, 4, 13, 13],
  ])(
    'covers the far edge at device pixel ratio %s',
    async (
      ratio,
      x,
      y,
      rectWidth,
      rectHeight,
      offsetX,
      offsetY,
      left,
      top,
      width,
      height
    ) => {
      Object.defineProperty(browser, 'devicePixelRatio', { value: ratio });
      const blackout: RedactAnnotation = {
        id: 'blackout',
        type: 'redact',
        x,
        y,
        width: rectWidth,
        height: rectHeight,
        style: 'blackout',
        intensity: 5,
      };
      const image = {
        naturalWidth: 100,
        naturalHeight: 80,
      } as HTMLImageElement;

      await act(() =>
        root.render(
          createElement(RedactOverlay, {
            image,
            imageWidth: 100,
            imageHeight: 80,
            canvasWidth: 100,
            canvasHeight: 80,
            offsetX,
            offsetY,
            annotations: [blackout],
            currentAnnotation: null,
          })
        )
      );

      expect(fillRect).toHaveBeenCalledExactlyOnceWith(
        left,
        top,
        width,
        height
      );
    }
  );
});
