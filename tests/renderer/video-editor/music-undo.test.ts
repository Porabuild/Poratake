import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, createElement, useEffect } from 'react';
import type { Root } from 'react-dom/client';
import { Window } from 'happy-dom';
import { useEditorHistory } from '@/renderer/components/video-editor/hooks/use-editor-history';
import { useMusicTracks } from '@/renderer/components/video-editor/hooks/use-music-tracks';
import type { MusicTrack } from '@/types/music';

let root: Root;
let browser: Window;
let history: ReturnType<typeof useEditorHistory>;
let music: ReturnType<typeof useMusicTracks>;
const invoke = vi.fn();

function Harness() {
  const currentHistory = useEditorHistory();
  const currentMusic = useMusicTracks({
    totalTimelineDuration: 10,
    slice: currentHistory.musicTracks,
  });
  useEffect(() => {
    history = currentHistory;
    music = currentMusic;
  });
  return null;
}

beforeEach(async () => {
  browser = new Window();
  vi.stubGlobal('window', browser);
  vi.stubGlobal('document', browser.document);
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  invoke.mockReset();
  invoke.mockResolvedValue({ success: true });
  window.ipcRenderer = { invoke } as never;
  const { createRoot } = await import('react-dom/client');
  root = createRoot(document.createElement('div'));
  await act(() => root.render(createElement(Harness)));
});

afterEach(async () => {
  await act(() => root.unmount());
  await browser.happyDOM.close();
  vi.unstubAllGlobals();
});

describe('music removal and history', () => {
  it('keeps the imported file available through undo and redo', async () => {
    const track: MusicTrack = {
      id: 'music-1',
      groupId: 'music-1',
      name: 'sound',
      source: 'music',
      fileName: 'sound.wav',
      volume: 0.8,
      enabled: true,
      startTime: 0,
      endTime: 10,
      originalDuration: 10,
      trimStart: 0,
      trimEnd: 0,
      speed: 1,
    };
    await act(() => history.initializeDocument({ musicTracks: [track] }));
    await act(() => music.handleRemoveMusicTrackGroup(track.groupId));
    expect(music.musicTracks).toEqual([]);
    expect(invoke).not.toHaveBeenCalledWith('video-editor:music:remove', {
      fileName: track.fileName,
    });
    await act(() => history.undo());
    expect(music.musicTracks).toEqual([track]);
    await act(() => history.redo());
    expect(music.musicTracks).toEqual([]);
    await act(() => history.undo());
    expect(music.musicTracks).toEqual([track]);
  });
});
