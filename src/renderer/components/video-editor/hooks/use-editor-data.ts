import { useState, useEffect, useCallback } from 'react';
import type { CursorData, CursorStyle } from '@/types/cursor';
import type { CameraData, CameraStyle } from '@/types/camera';
import type { KeyboardData, KeyboardStyle } from '@/types/keyboard';
import {
  DEFAULT_SUBTITLE_STYLE,
  type SubtitleData,
  type SubtitleStyle,
  type SubtitleGenerationOptions,
} from '@/types/subtitle';
import { DEFAULT_CURSOR_STYLE } from '@/types/cursor';
import { DEFAULT_CAMERA_STYLE } from '@/types/camera';
import { DEFAULT_KEYBOARD_STYLE } from '@/types/keyboard';
import { DEFAULT_AUDIO_STYLE, type AudioStyle } from '@/types/audio';
import type { VideoMetadata } from '@/types/video';
import type { SliceController } from './use-editor-history';
import { toFileUrl } from '../utils';

export type VideoMetadataStatus = 'loading' | 'ready' | 'unavailable';

interface UseEditorDataProps {
  cursorStyleSlice: SliceController<CursorStyle>;
  cameraStyleSlice: SliceController<CameraStyle>;
  keyboardStyleSlice: SliceController<KeyboardStyle>;
  subtitleStyleSlice: SliceController<SubtitleStyle>;
  audioStyleSlice: SliceController<AudioStyle>;
}

interface UseEditorDataReturn {
  cursorData: CursorData | null;
  cursorStyle: CursorStyle;
  setCursorStyle: (style: CursorStyle) => void;
  setCursorData: (data: CursorData | null) => void;
  handleCursorDataSave: (
    data: CursorData
  ) => Promise<{ success: boolean; error?: string }>;
  handleCursorDataImport: () => Promise<{ success: boolean; error?: string }>;
  cameraData: CameraData | null;
  cameraSrc: string | null;
  cameraVideoPath: string | null;
  cameraStyle: CameraStyle;
  setCameraStyle: (style: CameraStyle) => void;
  keyboardData: KeyboardData | null;
  keyboardStyle: KeyboardStyle;
  setKeyboardStyle: (style: KeyboardStyle) => void;
  subtitleData: SubtitleData | null;
  subtitleStyle: SubtitleStyle;
  setSubtitleStyle: (style: SubtitleStyle) => void;
  setSubtitleData: (data: SubtitleData | null) => void;
  handleSubtitleGenerate: (options: SubtitleGenerationOptions) => Promise<void>;
  handleSubtitleDelete: () => Promise<void>;
  handleSubtitleDataSave: (
    data: SubtitleData
  ) => Promise<{ success: boolean; error?: string }>;
  handleSubtitleDataImport: () => Promise<{ success: boolean; error?: string }>;
  audioStyle: AudioStyle;
  setAudioStyle: (style: AudioStyle) => void;
  systemAudioPath: string | null;
  micAudioPath: string | null;
  hasEmbeddedAudio: boolean;
  audioPathsLoaded: boolean;
  videoMetadata: VideoMetadata | null;
  videoMetadataStatus: VideoMetadataStatus;
  restoreState: (state: {
    cursorStyle?: CursorStyle;
    cameraStyle?: CameraStyle;
    keyboardStyle?: KeyboardStyle;
    subtitleStyle?: SubtitleStyle;
    audioStyle?: AudioStyle;
  }) => void;
}

export function useEditorData({
  cursorStyleSlice,
  cameraStyleSlice,
  keyboardStyleSlice,
  subtitleStyleSlice,
  audioStyleSlice,
}: UseEditorDataProps): UseEditorDataReturn {
  const [cursorData, setCursorData] = useState<CursorData | null>(null);
  const [cameraData, setCameraData] = useState<CameraData | null>(null);
  const [cameraSrc, setCameraSrc] = useState<string | null>(null);
  const [cameraVideoPath, setCameraVideoPath] = useState<string | null>(null);
  const [keyboardData, setKeyboardData] = useState<KeyboardData | null>(null);
  const [subtitleData, setSubtitleData] = useState<SubtitleData | null>(null);

  const [systemAudioPath, setSystemAudioPath] = useState<string | null>(null);
  const [micAudioPath, setMicAudioPath] = useState<string | null>(null);
  const [hasEmbeddedAudio, setHasEmbeddedAudio] = useState(false);
  const [audioPathsLoaded, setAudioPathsLoaded] = useState(false);

  const [videoMetadata, setVideoMetadata] = useState<VideoMetadata | null>(
    null
  );
  const [videoMetadataStatus, setVideoMetadataStatus] =
    useState<VideoMetadataStatus>('loading');

  useEffect(() => {
    window.ipcRenderer
      .invoke('video-editor:getCursorData')
      .then((data: CursorData | null) => {
        if (data && data.events.length > 0) {
          setCursorData(data);
        }
      })
      .catch((err: Error) => {
        console.error('Failed to load cursor data:', err);
      });
  }, []);

  useEffect(() => {
    window.ipcRenderer
      .invoke('video-editor:getCameraData')
      .then(
        (
          result: { cameraData: CameraData; cameraVideoPath: string } | null
        ) => {
          if (result) {
            setCameraData(result.cameraData);
            setCameraSrc(toFileUrl(result.cameraVideoPath));
            setCameraVideoPath(result.cameraVideoPath);
          }
        }
      )
      .catch((err: Error) => {
        console.error('Failed to load camera data:', err);
      });
  }, []);

  useEffect(() => {
    window.ipcRenderer
      .invoke('video-editor:getKeyboardData')
      .then((data: KeyboardData | null) => {
        if (data && data.events.length > 0) {
          setKeyboardData(data);
        }
      })
      .catch((err: Error) => {
        console.error('Failed to load keyboard data:', err);
      });
  }, []);

  useEffect(() => {
    window.ipcRenderer
      .invoke('video-editor:getAudioPaths')
      .then(
        (result: {
          systemAudioPath: string | null;
          micAudioPath: string | null;
          hasEmbeddedAudio: boolean;
        }) => {
          if (result.systemAudioPath) {
            setSystemAudioPath(result.systemAudioPath);
          }
          if (result.micAudioPath) {
            setMicAudioPath(result.micAudioPath);
          }
          setHasEmbeddedAudio(result.hasEmbeddedAudio);
        }
      )
      .catch((err: Error) => {
        console.error('Failed to load audio paths:', err);
      })
      .finally(() => {
        setAudioPathsLoaded(true);
      });
  }, []);

  useEffect(() => {
    window.ipcRenderer
      .invoke('video-editor:getSubtitleData')
      .then((data: SubtitleData | null) => {
        if (data && data.segments.length > 0) {
          setSubtitleData(data);
        }
      })
      .catch((err: Error) => {
        console.error('Failed to load subtitle data:', err);
      });
  }, []);

  useEffect(() => {
    window.ipcRenderer
      .invoke('video-editor:getVideoMetadata')
      .then((metadata: VideoMetadata | null) => {
        if (metadata && metadata.duration > 0) {
          setVideoMetadata(metadata);
          setVideoMetadataStatus('ready');
          return;
        }
        console.error('Video metadata unavailable for this recording');
        setVideoMetadataStatus('unavailable');
      })
      .catch((err: Error) => {
        console.error('Failed to get video metadata:', err);
        setVideoMetadataStatus('unavailable');
      });
  }, []);

  const handleSubtitleGenerate = useCallback(
    async (options: SubtitleGenerationOptions) => {
      const result = await window.ipcRenderer.invoke(
        'video-editor:generateSubtitles',
        options
      );
      if (result.success && result.data && result.data.segments.length > 0) {
        setSubtitleData(result.data);
      } else if (!result.success && result.error) {
        throw new Error(result.error);
      }
    },
    []
  );

  const handleSubtitleDelete = useCallback(async () => {
    await window.ipcRenderer.invoke('video-editor:deleteSubtitleData');
    setSubtitleData(null);
  }, []);

  const handleSubtitleDataSave = useCallback(
    async (
      data: SubtitleData
    ): Promise<{ success: boolean; error?: string }> => {
      const result = await window.ipcRenderer.invoke(
        'video-editor:saveSubtitleData',
        data
      );
      if (result.success) {
        setSubtitleData(data);
      }
      return result;
    },
    []
  );

  const handleSubtitleDataImport = useCallback(async (): Promise<{
    success: boolean;
    error?: string;
  }> => {
    const result = await window.ipcRenderer.invoke(
      'video-editor:importSubtitleData'
    );
    if (result.success && result.data) {
      setSubtitleData(result.data);
    }
    return result;
  }, []);

  const handleCursorDataSave = useCallback(
    async (data: CursorData): Promise<{ success: boolean; error?: string }> => {
      const result = await window.ipcRenderer.invoke(
        'video-editor:saveCursorData',
        data
      );
      if (result.success) {
        setCursorData(data);
      }
      return result;
    },
    []
  );

  const handleCursorDataImport = useCallback(async (): Promise<{
    success: boolean;
    error?: string;
  }> => {
    const result = await window.ipcRenderer.invoke(
      'video-editor:importCursorData'
    );
    if (result.success && result.data) {
      setCursorData(result.data);
    }
    return result;
  }, []);

  const setCursorStyle = useCallback(
    (style: CursorStyle) => cursorStyleSlice.set(style),
    [cursorStyleSlice]
  );
  const setCameraStyle = useCallback(
    (style: CameraStyle) => cameraStyleSlice.set(style),
    [cameraStyleSlice]
  );
  const setKeyboardStyle = useCallback(
    (style: KeyboardStyle) => keyboardStyleSlice.set(style),
    [keyboardStyleSlice]
  );
  const setSubtitleStyle = useCallback(
    (style: SubtitleStyle) => subtitleStyleSlice.set(style),
    [subtitleStyleSlice]
  );
  const setAudioStyle = useCallback(
    (style: AudioStyle) => audioStyleSlice.set(style),
    [audioStyleSlice]
  );

  const restoreState = useCallback(
    (state: {
      cursorStyle?: CursorStyle;
      cameraStyle?: CameraStyle;
      keyboardStyle?: KeyboardStyle;
      subtitleStyle?: SubtitleStyle;
      audioStyle?: AudioStyle;
    }) => {
      if (state.cursorStyle) {
        cursorStyleSlice.set({
          ...DEFAULT_CURSOR_STYLE,
          ...state.cursorStyle,
        });
      }
      if (state.cameraStyle) {
        cameraStyleSlice.set({
          ...DEFAULT_CAMERA_STYLE,
          ...state.cameraStyle,
        });
      }
      if (state.keyboardStyle) {
        keyboardStyleSlice.set({
          ...DEFAULT_KEYBOARD_STYLE,
          ...state.keyboardStyle,
        });
      }
      if (state.subtitleStyle) {
        subtitleStyleSlice.set({
          ...DEFAULT_SUBTITLE_STYLE,
          ...state.subtitleStyle,
        });
      }
      if (state.audioStyle) {
        audioStyleSlice.set({
          ...DEFAULT_AUDIO_STYLE,
          ...state.audioStyle,
        });
      }
    },
    [
      cursorStyleSlice,
      cameraStyleSlice,
      keyboardStyleSlice,
      subtitleStyleSlice,
      audioStyleSlice,
    ]
  );

  return {
    cursorData,
    cursorStyle: cursorStyleSlice.value,
    setCursorStyle,
    setCursorData,
    handleCursorDataSave,
    handleCursorDataImport,
    cameraData,
    cameraSrc,
    cameraVideoPath,
    cameraStyle: cameraStyleSlice.value,
    setCameraStyle,
    keyboardData,
    keyboardStyle: keyboardStyleSlice.value,
    setKeyboardStyle,
    subtitleData,
    subtitleStyle: subtitleStyleSlice.value,
    setSubtitleStyle,
    setSubtitleData,
    handleSubtitleGenerate,
    handleSubtitleDelete,
    handleSubtitleDataSave,
    handleSubtitleDataImport,
    audioStyle: audioStyleSlice.value,
    setAudioStyle,
    systemAudioPath,
    micAudioPath,
    hasEmbeddedAudio,
    audioPathsLoaded,
    videoMetadata,
    videoMetadataStatus,
    restoreState,
  };
}
