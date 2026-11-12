import { act, createElement, createRef } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import SvgAnnotationsOverlay, {
  type SvgAnnotationsOverlayHandle,
} from '@/renderer/components/editor/svg-annotations-overlay';
import type {
  Annotation,
  RectAnnotation,
  RedactAnnotation,
} from '@/types/editor';
import { Window } from 'happy-dom';

const shape: RectAnnotation = {
  id: 'shape',
  type: 'rectangle',
  x: 10,
  y: 10,
  width: 50,
  height: 40,
  fill: '#ff00ff',
  stroke: '#ff00ff',
  strokeWidth: 1,
};

const blackout: RedactAnnotation = {
  id: 'blackout',
  type: 'redact',
  x: 20,
  y: 20,
  width: 30,
  height: 20,
  style: 'blackout',
  intensity: 5,
};

const cases: Array<{
  annotations: Annotation[];
  shapeBeforeBlackout: boolean;
}> = [
  { annotations: [shape, blackout], shapeBeforeBlackout: true },
  { annotations: [blackout, shape], shapeBeforeBlackout: false },
];

let browser: Window;
let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  browser = new Window();
  vi.stubGlobal('window', browser);
  vi.stubGlobal('document', browser.document);
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  container = document.createElement('div');
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(() => root.unmount());
  await browser.happyDOM.close();
  vi.unstubAllGlobals();
});

describe('screenshot blackout annotation order', () => {
  it.each(cases)(
    'matches live and exported SVG stacking for $shapeBeforeBlackout',
    async ({ annotations, shapeBeforeBlackout }) => {
      const ref = createRef<SvgAnnotationsOverlayHandle>();

      await act(() =>
        root.render(
          createElement(SvgAnnotationsOverlay, {
            ref,
            annotations,
            currentAnnotation: null,
            width: 100,
            height: 100,
            selectedAnnotationIds: [],
            onSelect: () => {},
            editingTextId: null,
            activeTool: 'select',
            showBlackoutFill: true,
          })
        )
      );

      const svg = container.querySelector('svg');
      const shapeRect = svg?.querySelector('rect[fill="#ff00ff"]');
      const blackoutRect = svg?.querySelector('rect[fill="#000000"]');
      expect(shapeRect).not.toBeNull();
      expect(blackoutRect).not.toBeNull();
      expect(blackoutRect?.getAttribute('pointer-events')).toBe('none');

      const liveRects = Array.from(svg?.querySelectorAll('rect') ?? []);
      expect(
        liveRects.indexOf(shapeRect!) < liveRects.indexOf(blackoutRect!)
      ).toBe(shapeBeforeBlackout);

      const exportedSvg = ref.current?.getSvgForExport(1) ?? '';
      const shapeIndex = exportedSvg.indexOf('fill="#ff00ff"');
      const blackoutIndex = exportedSvg.indexOf('fill="#000000"');
      expect(shapeIndex).toBeGreaterThanOrEqual(0);
      expect(blackoutIndex).toBeGreaterThanOrEqual(0);
      expect(shapeIndex < blackoutIndex).toBe(shapeBeforeBlackout);
    }
  );

  it('keeps the blackout selection guide interactive', async () => {
    const onSelect = vi.fn();

    await act(() =>
      root.render(
        createElement(SvgAnnotationsOverlay, {
          annotations: [blackout],
          currentAnnotation: null,
          width: 100,
          height: 100,
          selectedAnnotationIds: [],
          onSelect,
          editingTextId: null,
          activeTool: 'select',
          showBlackoutFill: true,
        })
      )
    );

    const guide = container.querySelector('rect[fill="transparent"]');
    expect(guide).not.toBeNull();

    await act(() => {
      guide?.dispatchEvent(
        new browser.MouseEvent('mousedown', { bubbles: true, cancelable: true })
      );
    });

    expect(onSelect).toHaveBeenCalledWith('blackout', false);
  });

  it('keeps the shared overlay blackout-transparent by default', async () => {
    await act(() =>
      root.render(
        createElement(SvgAnnotationsOverlay, {
          annotations: [blackout],
          currentAnnotation: null,
          width: 100,
          height: 100,
          selectedAnnotationIds: [],
          onSelect: () => {},
          editingTextId: null,
          activeTool: 'select',
        })
      )
    );

    expect(container.querySelector('rect[fill="#000000"]')).toBeNull();
    expect(container.querySelector('rect[fill="transparent"]')).not.toBeNull();
  });
});
