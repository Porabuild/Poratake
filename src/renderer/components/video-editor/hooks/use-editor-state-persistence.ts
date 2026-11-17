import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import type {
  VideoEditorState,
  ExportSettings,
} from '@/types/video-editor-state';
import { EDITOR_STATE_VERSION } from '@/types/video-editor-state';
import type { RecordingType } from '@/types/video';
import type { CursorStyle } from '@/types/cursor';
import type { CameraStyle, CameraSegment } from '@/types/camera';
import type { KeyboardStyle } from '@/types/keyboard';
import type { SubtitleStyle } from '@/types/subtitle';
import type { AudioStyle } from '@/types/audio';
import type { ZoomSegment, ZoomSettings } from '@/types/zoom';
import type { VideoWallpaperSettings } from '@/types/video-wallpaper';
import type { FirstFrameSettings } from '@/types/first-frame';
import type { MusicTrack } from '@/types/music';
import type { DrawingSegment } from '@/types/drawing';
import type { Segment } from '../types';
import type { SidebarTab } from '../editor-sidebar';

const SAVE_DEBOUNCE_MS = 500;

interface EditorStateValues {
  segments: Segment[];
  cursorStyle: CursorStyle;
  cameraStyle: CameraStyle;
  keyboardStyle: KeyboardStyle;
  subtitleStyle: SubtitleStyle;
  audioStyle: AudioStyle;
  zoomSegments: ZoomSegment[];
  zoomSettings: ZoomSettings;
  cameraSegments: CameraSegment[];
  drawingSegments: DrawingSegment[];
  wallpaper: VideoWallpaperSettings;
  firstFrame: FirstFrameSettings;
  musicTracks: MusicTrack[];
  exportSettings: ExportSettings;
  timelineZoom: number;
  sidebarOpen: boolean;
  sidebarTab: SidebarTab;
  scrubAudioEnabled: boolean;
}

interface UseEditorStatePersistenceProps {
  isReady: boolean;
  values: EditorStateValues;
}

interface UseEditorStatePersistenceReturn {
  loadedState: VideoEditorState | null;
  isStateLoaded: boolean;
  recordingType: RecordingType | undefined;
  resetState: () => Promise<boolean>;
}

export function useEditorStatePersistence({
  isReady,
  values,
}: UseEditorStatePersistenceProps): UseEditorStatePersistenceReturn {
  const [loadedState, setLoadedState] = useState<VideoEditorState | null>(null);
  const [isStateLoaded, setIsStateLoaded] = useState(false);
  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hasAppliedStateRef = useRef(false);
  const isMountedRef = useRef(true);
  const saveQueueRef = useRef<Promise<boolean>>(Promise.resolve(true));
  const latestRef = useRef({ isReady, isStateLoaded, loadedState, values });
  useLayoutEffect(() => {
    latestRef.current = { isReady, isStateLoaded, loadedState, values };
  }, [isReady, isStateLoaded, loadedState, values]);

  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
      if (saveTimeoutRef.current) {
        clearTimeout(saveTimeoutRef.current);
        saveTimeoutRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    if (isStateLoaded) return;

    window.ipcRenderer
      .invoke('video-editor:getState')
      .then((state: VideoEditorState | null) => {
        if (!isMountedRef.current) return;
        setLoadedState(state);
        setIsStateLoaded(true);
      })
      .catch((err: Error) => {
        console.error('Failed to load editor state:', err);
        if (isMountedRef.current) {
          setIsStateLoaded(true);
        }
      });
  }, [isStateLoaded]);

  useEffect(() => {
    if (isReady && values.segments.length > 0 && !hasAppliedStateRef.current) {
      hasAppliedStateRef.current = true;
    }
  }, [isReady, values.segments.length]);

  const saveState = useCallback((): Promise<boolean> => {
    const save = saveQueueRef.current
      .catch(() => false)
      .then(async () => {
        const snapshot = latestRef.current;
        const currentValues = snapshot.values;
        if (
          !isMountedRef.current ||
          !snapshot.isStateLoaded ||
          !snapshot.isReady ||
          !hasAppliedStateRef.current ||
          currentValues.segments.length === 0
        ) {
          return true;
        }
        const state: VideoEditorState = {
          version: EDITOR_STATE_VERSION,
          savedAt: new Date().toISOString(),
          recordingType: snapshot.loadedState?.recordingType,
          segments: currentValues.segments,
          cursorStyle: currentValues.cursorStyle,
          cameraStyle: currentValues.cameraStyle,
          keyboardStyle: currentValues.keyboardStyle,
          subtitleStyle: currentValues.subtitleStyle,
          audioStyle: currentValues.audioStyle,
          zoomSegments: currentValues.zoomSegments,
          zoomSettings: currentValues.zoomSettings,
          cameraSegments: currentValues.cameraSegments,
          drawingSegments: currentValues.drawingSegments,
          wallpaper: currentValues.wallpaper,
          firstFrame: currentValues.firstFrame,
          musicTracks: currentValues.musicTracks,
          exportSettings: currentValues.exportSettings,
          timelineZoom: currentValues.timelineZoom,
          ui: {
            sidebarOpen: currentValues.sidebarOpen,
            sidebarTab: currentValues.sidebarTab,
            scrubAudioEnabled: currentValues.scrubAudioEnabled,
          },
        };
        try {
          const saved = await window.ipcRenderer.invoke(
            'video-editor:saveState',
            state
          );
          if (!saved) console.error('Failed to save editor state');
          return saved === true;
        } catch (err) {
          console.error('Failed to save editor state:', err);
          return false;
        }
      });
    saveQueueRef.current = save;
    return save;
  }, []);

  useEffect(() => {
    const remove = window.ipcRenderer.on('video-editor:prepare-close', () => {
      if (saveTimeoutRef.current) {
        clearTimeout(saveTimeoutRef.current);
        saveTimeoutRef.current = null;
      }
      void saveState().then(saved => {
        window.ipcRenderer.send(
          saved
            ? 'video-editor:close-confirmed'
            : 'video-editor:close-save-failed'
        );
      });
    });
    window.ipcRenderer.send('video-editor:close-handler-ready');
    return remove;
  }, [saveState]);

  useEffect(() => {
    if (
      !isStateLoaded ||
      !isReady ||
      !hasAppliedStateRef.current ||
      values.segments.length === 0
    ) {
      return;
    }

    if (saveTimeoutRef.current) {
      clearTimeout(saveTimeoutRef.current);
    }

    saveTimeoutRef.current = setTimeout(() => {
      saveTimeoutRef.current = null;
      void saveState();
    }, SAVE_DEBOUNCE_MS);

    return () => {
      if (saveTimeoutRef.current) {
        clearTimeout(saveTimeoutRef.current);
        saveTimeoutRef.current = null;
      }
    };
  }, [isStateLoaded, isReady, values, saveState]);

  const resetState = useCallback(async (): Promise<boolean> => {
    if (saveTimeoutRef.current) {
      clearTimeout(saveTimeoutRef.current);
      saveTimeoutRef.current = null;
    }

    try {
      await saveQueueRef.current;
      return await window.ipcRenderer.invoke('video-editor:resetState');
    } catch (err) {
      console.error('Failed to reset editor state:', err);
      return false;
    }
  }, []);

  return {
    loadedState,
    isStateLoaded,
    recordingType: loadedState?.recordingType,
    resetState,
  };
}
