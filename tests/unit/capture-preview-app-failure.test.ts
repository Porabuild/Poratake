// @vitest-environment happy-dom
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CapturePreviewParams } from '@/types/capture-preview';

vi.mock('@/renderer/hooks/use-app-theme', () => ({
  useAppTheme: () => {},
}));

const capturePreviewModule = vi.hoisted(() => {
  let rejectModule: (error: Error) => void = () => {};
  const promise = new Promise<{ default: () => unknown }>(
    (_resolve, reject) => {
      rejectModule = reject;
    }
  );
  return { promise, rejectModule };
});

vi.mock(
  '@/renderer/windows/capture-preview-window',
  () => capturePreviewModule.promise
);

const send = vi.fn();
const listeners = new Map<string, (event: unknown, data: unknown) => void>();
let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  window.history.replaceState(null, '', '/?window=capture-preview');
  send.mockClear();
  listeners.clear();
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
  window.ipcRenderer = {
    send,
    invoke: vi.fn(() => Promise.resolve(null)),
    on: vi.fn((channel, listener) => {
      listeners.set(
        channel,
        listener as (event: unknown, data: unknown) => void
      );
      return () => {};
    }),
  } as never;
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

describe('App capture-preview load gating', () => {
  it('reports renderer failure when the preview module fails to load', async () => {
    const { default: App } = await import('@/renderer/App');

    await act(async () => {
      root.render(createElement(App));
    });

    const params: CapturePreviewParams = {
      filePath: 'E:\\shots\\a.png',
      contentType: 'screenshot',
      imageUrl: 'file:///E:/shots/a.png',
    };

    await act(async () => {
      listeners.get('load')?.({}, { type: 'capture-preview', params });
      await Promise.resolve();
    });

    await act(async () => {
      capturePreviewModule.rejectModule(new Error('module failed'));
      await capturePreviewModule.promise.catch(() => {});
    });

    expect(send).toHaveBeenCalledWith('capture-preview:renderer-failed');
    expect(send).not.toHaveBeenCalledWith('capture-preview:content-ready');
  });
});
