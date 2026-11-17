import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, createElement } from 'react';
import type { Root } from 'react-dom/client';
import { Window } from 'happy-dom';
import { useEditorStatePersistence } from '@/renderer/components/video-editor/hooks/use-editor-state-persistence';

type Values = Parameters<typeof useEditorStatePersistence>[0]['values'];
let root: Root;
let browser: Window;
let events: Map<string, (...args: unknown[]) => void>;
const invoke = vi.fn();
const send = vi.fn();

function Harness({ tracks }: { tracks: string[] }) {
  useEditorStatePersistence({
    isReady: true,
    values: {
      segments: [{ id: 'video' }],
      musicTracks: tracks.map(fileName => ({ fileName })),
      zoomSegments: [],
      cameraSegments: [],
      drawingSegments: [],
    } as unknown as Values,
  });
  return null;
}

beforeEach(async () => {
  browser = new Window();
  vi.stubGlobal('window', browser);
  vi.stubGlobal('document', browser.document);
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  events = new Map();
  invoke.mockReset();
  send.mockReset();
  invoke.mockImplementation(async channel =>
    channel === 'video-editor:getState' ? null : true
  );
  window.ipcRenderer = {
    invoke,
    send,
    on: (channel: string, callback: (...args: unknown[]) => void) => {
      events.set(channel, callback);
      return () => events.delete(channel);
    },
  } as never;
  const { createRoot } = await import('react-dom/client');
  root = createRoot(document.createElement('div'));
  await act(async () =>
    root.render(createElement(Harness, { tracks: ['original.wav'] }))
  );
});

afterEach(async () => {
  await act(() => root.unmount());
  await browser.happyDOM.close();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('video editor close persistence', () => {
  it('saves the latest music before confirming close', async () => {
    await act(() =>
      root.render(createElement(Harness, { tracks: ['restored.wav'] }))
    );
    await act(async () => events.get('video-editor:prepare-close')?.());
    expect(invoke).toHaveBeenCalledWith(
      'video-editor:saveState',
      expect.objectContaining({
        musicTracks: [{ fileName: 'restored.wav' }],
      })
    );
    expect(send).toHaveBeenCalledWith('video-editor:close-confirmed');
  });

  it('waits for an in-flight save and then saves the latest edit', async () => {
    vi.useFakeTimers();
    let finishFirst: ((value: boolean) => void) | undefined;
    invoke.mockImplementation(channel => {
      if (channel !== 'video-editor:saveState') return Promise.resolve(null);
      if (!finishFirst)
        return new Promise<boolean>(resolve => {
          finishFirst = resolve;
        });
      return Promise.resolve(true);
    });
    await act(() =>
      root.render(createElement(Harness, { tracks: ['first.wav'] }))
    );
    await act(async () => vi.advanceTimersByTimeAsync(500));
    expect(finishFirst).toBeDefined();
    await act(() =>
      root.render(createElement(Harness, { tracks: ['latest.wav'] }))
    );
    await act(async () => events.get('video-editor:prepare-close')?.());
    expect(send).not.toHaveBeenCalledWith('video-editor:close-confirmed');
    await act(async () => finishFirst?.(true));
    expect(invoke).toHaveBeenLastCalledWith(
      'video-editor:saveState',
      expect.objectContaining({
        musicTracks: [{ fileName: 'latest.wav' }],
      })
    );
    expect(send).toHaveBeenCalledWith('video-editor:close-confirmed');
  });

  it('keeps the window open when the save fails', async () => {
    invoke.mockResolvedValue(false);
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    await act(async () => events.get('video-editor:prepare-close')?.());
    expect(send).toHaveBeenCalledWith('video-editor:close-save-failed');
    expect(send).not.toHaveBeenCalledWith('video-editor:close-confirmed');
    error.mockRestore();
  });
});
