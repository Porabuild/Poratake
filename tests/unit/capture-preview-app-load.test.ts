// @vitest-environment happy-dom
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CapturePreviewParams } from '@/types/capture-preview';

vi.mock('@/renderer/hooks/use-app-theme', () => ({
  useAppTheme: () => {},
}));

const capturePreviewModule = vi.hoisted(() => {
  let resolveModule: (mod: { default: () => unknown }) => void = () => {};
  const promise = new Promise<{ default: () => unknown }>(resolve => {
    resolveModule = resolve;
  });
  return { promise, resolveModule };
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

function readyProbe() {
  return document.querySelector('[data-testid="capture-preview-ready"]');
}

describe('App capture-preview load gating', () => {
  it('applies the load payload only after the preview module loads', async () => {
    const { default: App } = await import('@/renderer/App');

    await act(async () => {
      root.render(createElement(App));
    });

    expect(readyProbe()).toBeNull();

    const params: CapturePreviewParams = {
      filePath: 'E:\\shots\\a.png',
      contentType: 'screenshot',
      imageUrl: 'file:///E:/shots/a.png',
    };

    await act(async () => {
      listeners.get('load')?.({}, { type: 'capture-preview', params });
      await Promise.resolve();
    });

    expect(readyProbe()).toBeNull();
    expect(container.querySelector('img')).toBeNull();
    expect(send).not.toHaveBeenCalledWith('capture-preview:content-ready');

    await act(async () => {
      capturePreviewModule.resolveModule({
        default: () =>
          createElement('div', { 'data-testid': 'capture-preview-ready' }),
      });
      await capturePreviewModule.promise;
    });

    expect(readyProbe()).not.toBeNull();
    expect(container.querySelector('img')).toBeNull();
    expect(send).not.toHaveBeenCalledWith('capture-preview:content-ready');
  });
});
