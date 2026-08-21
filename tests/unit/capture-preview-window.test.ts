// @vitest-environment happy-dom
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CapturePreviewParams } from '@/types/capture-preview';

vi.mock('@/renderer/hooks/use-video-clipboard-export', () => ({
  useVideoClipboardExport: () => ({
    isCopying: false,
    isDone: false,
    copyProgress: 0,
    startExport: vi.fn(),
    cancelExport: vi.fn(),
  }),
}));

vi.mock('@/renderer/hooks/use-cloud-file-upload', () => ({
  useCloudFileUpload: () => ({
    uploadState: 'idle',
    isUploading: false,
    upload: vi.fn(),
  }),
}));

vi.mock('@/renderer/hooks/use-polish-copy', () => ({
  usePolishCopy: () => ({
    preset: null,
    isPolishing: false,
    polish: vi.fn(),
  }),
}));

const send = vi.fn();
const invoke = vi.fn(() => Promise.resolve([]));
let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  send.mockClear();
  invoke.mockClear();
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
  window.ipcRenderer = {
    send,
    invoke,
    on: vi.fn(() => () => {}),
  } as never;
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

describe('CapturePreviewWindow', () => {
  it('does not reveal a video fallback before a thumbnail is ready', async () => {
    const params: CapturePreviewParams = {
      filePath: 'E:\\recordings\\Take.poratake',
      contentType: 'video',
      imageUrl: 'file:///E:/recordings/Take.poratake/recording.mov',
    };
    const { CapturePreviewFallback } = await import('@/renderer/App');

    await act(async () => {
      root.render(createElement(CapturePreviewFallback, { params }));
    });

    expect(container.querySelector('img')).toBeNull();
    expect(send).not.toHaveBeenCalledWith('capture-preview:content-ready');
  });

  it('waits for video data before revealing the preview', async () => {
    const params: CapturePreviewParams = {
      filePath: 'E:\\recordings\\Take.poratake',
      contentType: 'video',
      imageUrl: 'file:///E:/recordings/Take.poratake/recording.mov',
    };
    const { default: CapturePreviewWindow } =
      await import('@/renderer/windows/capture-preview-window');

    await act(async () => {
      root.render(createElement(CapturePreviewWindow, { params }));
    });

    expect(
      send.mock.calls.some(
        ([channel]) => channel === 'capture-preview:content-ready'
      )
    ).toBe(false);

    const video = container.querySelector('video');
    expect(video).not.toBeNull();
    await act(async () => {
      video?.dispatchEvent(new Event('loadeddata'));
    });

    expect(send).toHaveBeenCalledWith('capture-preview:content-ready');
  });
});
