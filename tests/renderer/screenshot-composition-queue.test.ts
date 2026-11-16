import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Window } from 'happy-dom';
import type { Annotation } from '@/types/editor';
import { useScreenshotComposition } from '@/renderer/hooks/useScreenshotComposition';

let browser: Window;
let container: HTMLDivElement;
let root: Root;
let drawImage: ReturnType<typeof vi.fn>;
const image = {} as HTMLImageElement;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((ok, fail) => {
    resolve = ok;
    reject = fail;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  browser = new Window();
  vi.stubGlobal('window', browser);
  vi.stubGlobal('document', browser.document);
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  drawImage = vi.fn();
  vi.spyOn(browser.HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    drawImage,
  } as never);
  container = document.createElement('div');
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(() => root.unmount());
  await browser.happyDOM.close();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('screenshot composition queue', () => {
  it('coalesces rapid edits and never paints an outdated result', async () => {
    const first = deferred<HTMLCanvasElement>();
    const last = deferred<HTMLCanvasElement>();
    const next = deferred<HTMLCanvasElement>();
    const renderToCanvas = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(last.promise)
      .mockReturnValueOnce(next.promise);
    const one = [{ id: 'one', type: 'redact' }] as Annotation[];
    const two = [{ id: 'two', type: 'redact' }] as Annotation[];
    const three = [{ id: 'three', type: 'redact' }] as Annotation[];
    const four = [{ id: 'four', type: 'redact' }] as Annotation[];
    function Fixture({ annotations }: { annotations: Annotation[] }) {
      const { canvasRef, hasPreview, ready, failed } = useScreenshotComposition(
        {
          image,
          annotations,
          currentAnnotation: null,
          editingTextId: null,
          renderToCanvas,
        }
      );
      return createElement(
        'div',
        null,
        createElement('canvas', { ref: canvasRef }),
        createElement(
          'output',
          null,
          ready
            ? 'ready'
            : failed
              ? 'failed'
              : hasPreview
                ? 'updating'
                : 'pending'
        )
      );
    }
    await act(() => root.render(createElement(Fixture, { annotations: one })));
    await act(() => root.render(createElement(Fixture, { annotations: two })));
    await act(() =>
      root.render(createElement(Fixture, { annotations: three }))
    );
    expect(renderToCanvas).toHaveBeenCalledTimes(1);
    const stale = document.createElement('canvas');
    const latest = document.createElement('canvas');
    await act(async () => {
      first.resolve(stale);
      await first.promise;
    });
    expect(renderToCanvas).toHaveBeenCalledTimes(2);
    expect(renderToCanvas).toHaveBeenLastCalledWith(three);
    expect(drawImage).not.toHaveBeenCalled();
    await act(async () => {
      last.resolve(latest);
      await last.promise;
    });
    expect(drawImage).toHaveBeenCalledExactlyOnceWith(latest, 0, 0);
    expect(container.querySelector('output')?.textContent).toBe('ready');
    await act(() => root.render(createElement(Fixture, { annotations: four })));
    expect(container.querySelector('output')?.textContent).toBe('updating');
    expect(drawImage).toHaveBeenCalledOnce();
    await act(async () => {
      next.resolve(latest);
      await next.promise;
    });
    expect(container.querySelector('output')?.textContent).toBe('ready');
  });

  it('reports a failed composition instead of marking it ready', async () => {
    const pending = deferred<HTMLCanvasElement>();
    const renderToCanvas = vi.fn().mockReturnValue(pending.promise);
    const log = vi.spyOn(console, 'error').mockImplementation(() => {});
    const noAnnotations: Annotation[] = [];
    function Fixture() {
      const { canvasRef, ready, failed } = useScreenshotComposition({
        image,
        annotations: noAnnotations,
        currentAnnotation: null,
        editingTextId: null,
        renderToCanvas,
      });
      return createElement(
        'div',
        null,
        createElement('canvas', { ref: canvasRef }),
        createElement(
          'output',
          null,
          ready ? 'ready' : failed ? 'failed' : 'pending'
        )
      );
    }
    await act(() => root.render(createElement(Fixture)));
    await act(async () => {
      pending.reject(new Error('failed'));
      await pending.promise.catch(() => {});
    });
    expect(container.querySelector('output')?.textContent).toBe('failed');
    expect(log).toHaveBeenCalledOnce();
  });
});
