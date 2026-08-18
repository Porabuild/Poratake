import { useCallback, useEffect, useMemo, useRef } from 'react';
import { useHistory } from '@/renderer/hooks/useHistory';
import type { Segment } from '../types';
import type { ZoomSegment, ZoomSettings } from '@/types/zoom';
import { DEFAULT_ZOOM_SETTINGS } from '@/types/zoom';
import type { MusicTrack } from '@/types/music';
import type { VideoWallpaperSettings } from '@/types/video-wallpaper';
import { DEFAULT_VIDEO_WALLPAPER } from '@/types/video-wallpaper';
import type { FirstFrameSettings } from '@/types/first-frame';
import { DEFAULT_FIRST_FRAME_SETTINGS } from '@/types/first-frame';
import type { CursorStyle } from '@/types/cursor';
import { DEFAULT_CURSOR_STYLE } from '@/types/cursor';
import type { CameraStyle, CameraSegment } from '@/types/camera';
import { DEFAULT_CAMERA_STYLE } from '@/types/camera';
import type { KeyboardStyle } from '@/types/keyboard';
import { DEFAULT_KEYBOARD_STYLE } from '@/types/keyboard';
import type { SubtitleStyle } from '@/types/subtitle';
import { DEFAULT_SUBTITLE_STYLE } from '@/types/subtitle';
import type { AudioStyle } from '@/types/audio';
import { DEFAULT_AUDIO_STYLE } from '@/types/audio';
import type { DrawingSegment } from '@/types/drawing';

export interface EditorDocument {
  segments: Segment[];
  zoomSegments: ZoomSegment[];
  zoomSettings: ZoomSettings;
  cameraSegments: CameraSegment[];
  drawingSegments: DrawingSegment[];
  musicTracks: MusicTrack[];
  wallpaper: VideoWallpaperSettings;
  firstFrame: FirstFrameSettings;
  cursorStyle: CursorStyle;
  cameraStyle: CameraStyle;
  keyboardStyle: KeyboardStyle;
  subtitleStyle: SubtitleStyle;
  audioStyle: AudioStyle;
}

const INITIAL_DOCUMENT: EditorDocument = {
  segments: [],
  zoomSegments: [],
  zoomSettings: DEFAULT_ZOOM_SETTINGS,
  cameraSegments: [],
  drawingSegments: [],
  musicTracks: [],
  wallpaper: DEFAULT_VIDEO_WALLPAPER,
  firstFrame: DEFAULT_FIRST_FRAME_SETTINGS,
  cursorStyle: DEFAULT_CURSOR_STYLE,
  cameraStyle: DEFAULT_CAMERA_STYLE,
  keyboardStyle: DEFAULT_KEYBOARD_STYLE,
  subtitleStyle: DEFAULT_SUBTITLE_STYLE,
  audioStyle: DEFAULT_AUDIO_STYLE,
};

const OBJECT_KEYS_WITH_DEFAULTS: ReadonlySet<keyof EditorDocument> = new Set([
  'zoomSettings',
  'wallpaper',
  'firstFrame',
  'cursorStyle',
  'cameraStyle',
  'keyboardStyle',
  'subtitleStyle',
  'audioStyle',
]);

export const INITIAL_EDITOR_DOCUMENT = INITIAL_DOCUMENT;

function assignKey<K extends keyof EditorDocument>(
  target: EditorDocument,
  key: K,
  value: EditorDocument[K]
): void {
  target[key] = value;
}

export function mergeDocument(
  base: EditorDocument,
  partial: Partial<EditorDocument>
): EditorDocument {
  const next: EditorDocument = { ...base };
  for (const key of Object.keys(partial) as (keyof EditorDocument)[]) {
    const value = partial[key];
    if (value === undefined) continue;
    if (
      OBJECT_KEYS_WITH_DEFAULTS.has(key) &&
      value !== null &&
      typeof value === 'object' &&
      !Array.isArray(value)
    ) {
      assignKey(next, key, {
        ...(INITIAL_DOCUMENT[key] as object),
        ...(value as object),
      } as EditorDocument[typeof key]);
      continue;
    }
    assignKey(next, key, value as EditorDocument[typeof key]);
  }
  return next;
}

type Updater<T> = T | ((prev: T) => T);

export interface SliceController<T> {
  value: T;
  set: (updater: Updater<T>) => void;
  setWithoutHistory: (updater: Updater<T>) => void;
  commit: () => void;
}

export interface UseEditorHistoryReturn {
  document: EditorDocument;
  initializeDocument: (partial: Partial<EditorDocument>) => void;
  replaceDocument: (partial: Partial<EditorDocument>) => void;
  undo: () => void;
  redo: () => void;
  canUndo: boolean;
  canRedo: boolean;
  commit: () => void;
  segments: SliceController<Segment[]>;
  zoomSegments: SliceController<ZoomSegment[]>;
  zoomSettings: SliceController<ZoomSettings>;
  cameraSegments: SliceController<CameraSegment[]>;
  drawingSegments: SliceController<DrawingSegment[]>;
  musicTracks: SliceController<MusicTrack[]>;
  wallpaper: SliceController<VideoWallpaperSettings>;
  firstFrame: SliceController<FirstFrameSettings>;
  cursorStyle: SliceController<CursorStyle>;
  cameraStyle: SliceController<CameraStyle>;
  keyboardStyle: SliceController<KeyboardStyle>;
  subtitleStyle: SliceController<SubtitleStyle>;
  audioStyle: SliceController<AudioStyle>;
}

function resolveUpdater<T>(updater: Updater<T>, prev: T): T {
  return typeof updater === 'function'
    ? (updater as (prev: T) => T)(prev)
    : updater;
}

export function useEditorHistory(): UseEditorHistoryReturn {
  const {
    state: document,
    set,
    setWithoutHistory,
    commitToHistory,
    undo,
    redo,
    canUndo,
    canRedo,
    reset,
  } = useHistory<EditorDocument>(INITIAL_DOCUMENT);

  const documentRef = useRef<EditorDocument>(document);
  useEffect(() => {
    documentRef.current = document;
  }, [document]);

  const initializeDocument = useCallback(
    (partial: Partial<EditorDocument>) => {
      reset(mergeDocument(INITIAL_DOCUMENT, partial));
    },
    [reset]
  );

  const replaceDocument = useCallback(
    (partial: Partial<EditorDocument>) => {
      set(mergeDocument(documentRef.current, partial));
    },
    [set]
  );

  const sliceSetters = useMemo(() => {
    const build = <K extends keyof EditorDocument>(key: K) => ({
      set: (updater: Updater<EditorDocument[K]>) => {
        const current = documentRef.current;
        const next = resolveUpdater(updater, current[key]);
        if (Object.is(next, current[key])) return;
        set({ ...current, [key]: next });
      },
      setWithoutHistory: (updater: Updater<EditorDocument[K]>) => {
        const current = documentRef.current;
        const next = resolveUpdater(updater, current[key]);
        if (Object.is(next, current[key])) return;
        setWithoutHistory({ ...current, [key]: next });
      },
      commit: commitToHistory,
    });

    return {
      segments: build('segments'),
      zoomSegments: build('zoomSegments'),
      zoomSettings: build('zoomSettings'),
      cameraSegments: build('cameraSegments'),
      drawingSegments: build('drawingSegments'),
      musicTracks: build('musicTracks'),
      wallpaper: build('wallpaper'),
      firstFrame: build('firstFrame'),
      cursorStyle: build('cursorStyle'),
      cameraStyle: build('cameraStyle'),
      keyboardStyle: build('keyboardStyle'),
      subtitleStyle: build('subtitleStyle'),
      audioStyle: build('audioStyle'),
    };
  }, [set, setWithoutHistory, commitToHistory]);

  return {
    document,
    initializeDocument,
    replaceDocument,
    undo,
    redo,
    canUndo,
    canRedo,
    commit: commitToHistory,
    segments: { value: document.segments, ...sliceSetters.segments },
    zoomSegments: {
      value: document.zoomSegments,
      ...sliceSetters.zoomSegments,
    },
    zoomSettings: {
      value: document.zoomSettings,
      ...sliceSetters.zoomSettings,
    },
    cameraSegments: {
      value: document.cameraSegments,
      ...sliceSetters.cameraSegments,
    },
    drawingSegments: {
      value: document.drawingSegments,
      ...sliceSetters.drawingSegments,
    },
    musicTracks: { value: document.musicTracks, ...sliceSetters.musicTracks },
    wallpaper: { value: document.wallpaper, ...sliceSetters.wallpaper },
    firstFrame: { value: document.firstFrame, ...sliceSetters.firstFrame },
    cursorStyle: { value: document.cursorStyle, ...sliceSetters.cursorStyle },
    cameraStyle: { value: document.cameraStyle, ...sliceSetters.cameraStyle },
    keyboardStyle: {
      value: document.keyboardStyle,
      ...sliceSetters.keyboardStyle,
    },
    subtitleStyle: {
      value: document.subtitleStyle,
      ...sliceSetters.subtitleStyle,
    },
    audioStyle: { value: document.audioStyle, ...sliceSetters.audioStyle },
  };
}
