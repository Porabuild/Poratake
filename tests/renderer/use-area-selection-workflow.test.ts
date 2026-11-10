import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, createElement, useEffect } from 'react';
import type { Root } from 'react-dom/client';
import { Window } from 'happy-dom';
import useAreaSelection from '@/renderer/hooks/use-area-selection';
import type { AreaSelectionOptions } from '@/renderer/hooks/use-area-selection';

let root: Root;
let browser: Window;
let selection: ReturnType<typeof useAreaSelection>;

function Harness({ options }: { options: AreaSelectionOptions }) {
  const current = useAreaSelection(options);
  useEffect(() => {
    selection = current;
  });
  return null;
}

beforeEach(async () => {
  browser = new Window();
  vi.stubGlobal('window', browser);
  vi.stubGlobal('document', browser.document);
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  window.ipcRenderer = { on: vi.fn(() => () => {}) } as never;
  const { createRoot } = await import('react-dom/client');
  root = createRoot(document.createElement('div'));
});

afterEach(async () => {
  await act(() => root.unmount());
  await browser.happyDOM.close();
  vi.unstubAllGlobals();
});

describe('area selection release coordinates', () => {
  it.each([
    {
      name: 'creating',
      initialRect: null,
      start: [100, 100],
      moved: [180, 180],
      released: [220, 200],
      expected: { x: 100, y: 100, width: 120, height: 100 },
    },
    {
      name: 'moving',
      initialRect: { x: 100, y: 100, width: 200, height: 150 },
      start: [150, 150],
      moved: [160, 160],
      released: [175, 180],
      expected: { x: 125, y: 130, width: 200, height: 150 },
    },
    {
      name: 'resizing',
      initialRect: { x: 100, y: 100, width: 200, height: 150 },
      start: [300, 250],
      moved: [310, 260],
      released: [330, 280],
      expected: { x: 100, y: 100, width: 230, height: 180 },
    },
  ])(
    'uses the final mouse-up position when $name',
    async ({ initialRect, start, moved, released, expected }) => {
      const options: AreaSelectionOptions = {
        interactive: true,
        initialRect,
        initialAspectRatio: null,
        pickTargets: null,
        repeatablePicks: false,
        onSelected: vi.fn(),
        onUpdated: vi.fn(),
        onDiscarded: vi.fn(),
      };
      await act(() => root.render(createElement(Harness, { options })));
      await act(() =>
        selection.startDrag({
          button: 0,
          clientX: start[0],
          clientY: start[1],
          preventDefault: vi.fn(),
        } as never)
      );
      await act(() =>
        window.dispatchEvent(
          new browser.MouseEvent('mousemove', {
            clientX: moved[0],
            clientY: moved[1],
          }) as never
        )
      );
      await act(() =>
        window.dispatchEvent(
          new browser.MouseEvent('mouseup', {
            clientX: released[0],
            clientY: released[1],
          }) as never
        )
      );
      expect(selection.rect).toEqual(expected);
      expect(
        initialRect ? options.onUpdated : options.onSelected
      ).toHaveBeenLastCalledWith(expected);
    }
  );
});
