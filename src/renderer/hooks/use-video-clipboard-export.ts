import { useState, useCallback, useRef } from 'react';
import type { WebCodecsExporter } from '@/renderer/components/video-editor/export';
import { DEFAULT_CURSOR_STYLE } from '@/types/cursor';
import { DEFAULT_CAMERA_STYLE } from '@/types/camera';
import { DEFAULT_AUDIO_STYLE } from '@/types/audio';
import { DEFAULT_ZOOM_SETTINGS } from '@/types/zoom';
import type { CursorData, CursorStyle } from '@/types/cursor';
import type { CameraStyle, CameraSegment } from '@/types/camera';
import type { AudioStyle } from '@/types/audio';
import type { VideoMetadata } from '@/types/video';
import type { Segment } from '@/renderer/components/video-editor/types';
import type { ZoomSegment, ZoomSettings } from '@/types/zoom';
import type { DrawingSegment } from '@/types/drawing';
import type { MusicTrack } from '@/types/music';

interface ExportData {
  videoPath: string;
  videoMetadata: VideoMetadata;
  segments: Segment[] | null;
  zoomSegments: ZoomSegment[] | null;
  zoomSettings: ZoomSettings | null;
  drawingSegments: DrawingSegment[] | null;
  cursorData: CursorData | null;
  cursorStyle: CursorStyle | null;
  cameraVideoPath: string | null;
  cameraStyle: CameraStyle | null;
  cameraVisibleRanges?: CameraSegment[] | null;
  systemAudioPath: string | null;
  micAudioPath: string | null;
  hasEmbeddedAudio: boolean;
  audioStyle: AudioStyle | null;
  musicTracks: MusicTrack[] | null;
}

interface UseVideoClipboardExportReturn {
  isCopying: boolean;
  isDone: boolean;
  copyProgress: number;
  startExport: () => void;
  cancelExport: () => void;
}

const DONE_DISPLAY_MS = 800;
const DONE_SAFETY_TIMEOUT_MS = 5000;

function createDefaultSegment(duration: number): Segment {
  return {
    id: crypto.randomUUID(),
    originalStart: 0,
    originalEnd: duration,
    trimMinStart: 0,
    trimMaxEnd: duration,
  };
}

export function useVideoClipboardExport(): UseVideoClipboardExportReturn {
  const [isCopying, setIsCopying] = useState(false);
  const [isDone, setIsDone] = useState(false);
  const [copyProgress, setCopyProgress] = useState(0);
  const exporterRef = useRef<WebCodecsExporter | null>(null);
  const isCopyingRef = useRef(false);
  const cancelRequestedRef = useRef(false);
  const exportSequenceRef = useRef(0);

  const resetState = useCallback(() => {
    setIsCopying(false);
    setIsDone(false);
    setCopyProgress(0);
  }, []);

  const cancelExport = useCallback(() => {
    cancelRequestedRef.current = true;
    exporterRef.current?.cancel();
    setCopyProgress(0);
  }, []);

  const startExport = useCallback(async () => {
    if (isCopyingRef.current) return;

    isCopyingRef.current = true;
    cancelRequestedRef.current = false;
    const exportSequence = ++exportSequenceRef.current;
    setIsCopying(true);
    setIsDone(false);
    setCopyProgress(0);

    let exporter: WebCodecsExporter | null = null;
    let outputPath: string | null = null;
    let completed = false;

    try {
      const data = (await window.ipcRenderer.invoke(
        'capture-preview:load-export-data'
      )) as ExportData | null;

      if (!data) {
        return;
      }

      outputPath = (await window.ipcRenderer.invoke(
        'capture-preview:get-export-output-path'
      )) as string;

      const { videoMetadata } = data;
      const cursorStyle = data.cursorStyle ?? DEFAULT_CURSOR_STYLE;
      const cameraStyle = data.cameraStyle ?? DEFAULT_CAMERA_STYLE;
      const audioStyle = data.audioStyle ?? DEFAULT_AUDIO_STYLE;
      const segments =
        data.segments && data.segments.length > 0
          ? data.segments
          : [createDefaultSegment(videoMetadata.duration)];
      const zoomSegments = data.zoomSegments ?? [];
      const zoomSettings = data.zoomSettings ?? DEFAULT_ZOOM_SETTINGS;
      const drawingSegments = data.drawingSegments ?? [];

      const { WebCodecsExporter } =
        await import('@/renderer/components/video-editor/export');
      exporter = new WebCodecsExporter();
      exporterRef.current = exporter;
      await exporter.begin();

      if (cancelRequestedRef.current) {
        exporter.cancel();
      }

      const result = await exporter.export({
        sourceVideoPath: data.videoPath,
        systemAudioPath: data.systemAudioPath,
        micAudioPath: data.micAudioPath,
        systemAudioEnabled: audioStyle.systemAudioEnabled,
        micAudioEnabled: audioStyle.micAudioEnabled,
        systemAudioVolume: audioStyle.systemAudioVolume,
        micAudioVolume: audioStyle.micAudioVolume,
        hasEmbeddedAudio: data.hasEmbeddedAudio,
        musicTracks: data.musicTracks ?? undefined,
        cameraVideoPath: data.cameraVideoPath,
        outputPath,
        config: {
          videoWidth: videoMetadata.width,
          videoHeight: videoMetadata.height,
          segments,
          wallpaper: null,
          zoomSegments,
          zoomSettings,
          drawingSegments,
          cursorData: data.cursorData,
          cursorStyle,
          cameraStyle,
          cameraVisibleRanges: data.cameraVisibleRanges ?? null,
          fps: 60,
        },
        frameRate: 60,
        qualityPreset: 'studio',
        resolution: 'original',
        onProgress: progress => {
          setCopyProgress(progress);
        },
      });

      if (exporter.isCancelled()) return;

      if (!result.success) {
        console.error('Preview export failed:', result.error);
        return;
      }

      setCopyProgress(100);

      const copied = (await window.ipcRenderer.invoke(
        'capture-preview:copy-video-to-clipboard',
        outputPath
      )) as boolean;

      if (exporter.isCancelled()) return;

      if (copied) {
        completed = true;
        setIsDone(true);
        setTimeout(() => {
          if (exportSequenceRef.current === exportSequence) {
            window.ipcRenderer.send('capture-preview:close');
          }
        }, DONE_DISPLAY_MS);
        setTimeout(() => {
          if (exportSequenceRef.current === exportSequence) {
            resetState();
          }
        }, DONE_SAFETY_TIMEOUT_MS);
      }
    } catch (error) {
      if (!exporter?.isCancelled()) {
        console.error('Preview export error:', error);
      }
    } finally {
      if (!completed && outputPath) {
        await window.ipcRenderer
          .invoke('video-editor:delete-temp-file', { filePath: outputPath })
          .catch(() => {});
      }
      await exporter?.finish().catch(() => {});
      if (exporterRef.current === exporter) {
        exporterRef.current = null;
      }
      isCopyingRef.current = false;
      cancelRequestedRef.current = false;
      setIsCopying(false);
      if (!completed) {
        setCopyProgress(0);
      }
    }
  }, [resetState]);

  return { isCopying, isDone, copyProgress, startExport, cancelExport };
}
