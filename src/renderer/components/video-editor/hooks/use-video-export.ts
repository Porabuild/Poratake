import { useState, useCallback, useRef, useEffect } from 'react';
import type { VideoExportOptions, VideoMetadata } from '@/types/video';
import type { ExportSettings } from '@/types/video-editor-state';
import type { CloudUploadState } from '@/types/cloud';
import type { Segment } from '../types';
import type { ZoomSegment, ZoomSettings } from '@/types/zoom';
import type { CursorData, CursorStyle } from '@/types/cursor';
import type { CameraStyle } from '@/types/camera';
import type { KeyboardData, KeyboardStyle } from '@/types/keyboard';
import type { SubtitleData, SubtitleStyle } from '@/types/subtitle';
import type { AudioStyle } from '@/types/audio';
import type { MusicTrack } from '@/types/music';
import type { VideoWallpaperSettings as VideoWallpaper } from '@/types/video-wallpaper';
import type { FirstFrameSettings } from '@/types/first-frame';
import type { DrawingSegment } from '@/types/drawing';
import { clampExportOptionsToFree } from '@/types/entitlements';
import { WebCodecsExporter } from '../export';
import { videoToTimeline, getTotalTimelineDuration } from '../utils';

const DEFAULT_EXPORT_SETTINGS: ExportSettings = {
  format: 'mp4',
  resolution: 'original',
  qualityPreset: 'studio',
  frameRate: '60',
  openInFinder: true,
};

interface ExportConfig {
  filePath: string;
  fileName: string;
  videoMetadata: VideoMetadata | null;
  segments: Segment[];
  wallpaper: VideoWallpaper;
  zoomSegments: ZoomSegment[];
  zoomSettings: ZoomSettings;
  drawingSegments: DrawingSegment[];
  cursorData: CursorData | null;
  cursorStyle: CursorStyle;
  cameraStyle: CameraStyle;
  cameraVideoPath: string | null;
  systemAudioPath: string | null;
  micAudioPath: string | null;
  audioStyle: AudioStyle;
  hasEmbeddedAudio: boolean;
  keyboardData: KeyboardData | null;
  keyboardStyle: KeyboardStyle;
  subtitleData: SubtitleData | null;
  subtitleStyle: SubtitleStyle;
  firstFrame: FirstFrameSettings;
  musicTracks: MusicTrack[];
  uploadToCloud: boolean;
}

interface UseVideoExportReturn {
  isExporting: boolean;
  exportProgress: number;
  exportSettings: ExportSettings;
  setExportSettings: React.Dispatch<React.SetStateAction<ExportSettings>>;
  cloudUploadState: CloudUploadState;
  uploadedUrl: string | null;
  copyUploadedUrl: () => void;
  cancelCloudUpload: () => void;
  handleExport: (
    options: VideoExportOptions,
    config: ExportConfig
  ) => Promise<void>;
  handleCancelExport: () => void;
  restoreExportSettings: (settings: ExportSettings) => void;
}

export function useVideoExport(): UseVideoExportReturn {
  const [isExporting, setIsExporting] = useState(false);
  const [exportProgress, setExportProgress] = useState(0);
  const [exportSettings, setExportSettings] = useState<ExportSettings>(
    DEFAULT_EXPORT_SETTINGS
  );
  const [cloudUploadState, setCloudUploadState] =
    useState<CloudUploadState>('idle');
  const [uploadedUrl, setUploadedUrl] = useState<string | null>(null);
  const exporterRef = useRef<WebCodecsExporter | null>(null);

  const copyUploadedUrl = useCallback(() => {
    if (!uploadedUrl) return;
    navigator.clipboard.writeText(uploadedUrl).catch(() => {});
  }, [uploadedUrl]);

  const cancelCloudUpload = useCallback(() => {
    window.ipcRenderer.send('cloud:cancelUpload');
  }, []);

  useEffect(() => {
    const handleExportStarted = () => {
      setIsExporting(true);
      setExportProgress(0);
    };

    const handleSaved = () => {
      setIsExporting(false);
      setExportProgress(100);
      setTimeout(() => {
        setExportProgress(0);
      }, 2000);
    };

    const handleSaveError = () => {
      setIsExporting(false);
      setExportProgress(0);
    };

    const handleSaveProgress = (
      _event: unknown,
      data: { progress: number }
    ) => {
      setExportProgress(data.progress);
    };

    const handleSaveCancelled = () => {
      setIsExporting(false);
      setExportProgress(0);
    };

    window.ipcRenderer.on('video-editor:export-started', handleExportStarted);
    window.ipcRenderer.on('video-editor:saved', handleSaved);
    window.ipcRenderer.on('video-editor:save-error', handleSaveError);
    window.ipcRenderer.on('video-editor:save-progress', handleSaveProgress);
    window.ipcRenderer.on('video-editor:save-cancelled', handleSaveCancelled);

    return () => {
      window.ipcRenderer.off(
        'video-editor:export-started',
        handleExportStarted
      );
      window.ipcRenderer.off('video-editor:saved', handleSaved);
      window.ipcRenderer.off('video-editor:save-error', handleSaveError);
      window.ipcRenderer.off('video-editor:save-progress', handleSaveProgress);
      window.ipcRenderer.off(
        'video-editor:save-cancelled',
        handleSaveCancelled
      );
    };
  }, []);

  const handleExport = useCallback(
    async (rawOptions: VideoExportOptions, config: ExportConfig) => {
      if (isExporting || !config.filePath || !config.videoMetadata) return;

      const isPro = (await window.ipcRenderer.invoke(
        'license:isPro'
      )) as boolean;
      const options = isPro ? rawOptions : clampExportOptionsToFree(rawOptions);

      const isGif = options.format === 'gif';
      const extension = isGif ? 'gif' : 'mp4';
      const defaultName = `${config.fileName}-exported.${extension}`;

      const dialogResult = (await window.ipcRenderer.invoke(
        'video-editor:show-save-dialog',
        { defaultName, format: options.format }
      )) as { canceled: boolean; filePath?: string };

      if (dialogResult.canceled || !dialogResult.filePath) return;

      const rawOutputPath = dialogResult.filePath;
      const finalOutputPath = isGif
        ? rawOutputPath.match(/\.gif$/i)
          ? rawOutputPath
          : `${rawOutputPath.replace(/\.[^/.]+$/, '')}.gif`
        : rawOutputPath;
      const frameRate = parseInt(options.frameRate, 10);

      setIsExporting(true);
      setExportProgress(0);
      setCloudUploadState('idle');
      setUploadedUrl(null);

      const exporter = new WebCodecsExporter();
      exporterRef.current = exporter;

      const exportStartTime = Date.now();
      let keyboardSoundTempPath: string | null = null;

      try {
        const mp4OutputPath = isGif
          ? finalOutputPath.match(/\.gif$/i)
            ? finalOutputPath.replace(/\.gif$/i, '-temp.mp4')
            : `${finalOutputPath}-temp.mp4`
          : finalOutputPath;

        if (
          config.audioStyle.keyboardSoundEnabled &&
          config.keyboardData?.events?.length
        ) {
          const downEvents = config.keyboardData.events.filter(
            e => e.type === 'down'
          );
          const keyPresses = downEvents
            .map(e => {
              const timelineTime = videoToTimeline(
                config.segments,
                e.timestamp
              );
              return { timestamp: timelineTime };
            })
            .filter(
              e =>
                e.timestamp >= 0 &&
                e.timestamp < getTotalTimelineDuration(config.segments)
            );

          if (keyPresses.length > 0) {
            const duration = getTotalTimelineDuration(config.segments);
            const tempPath = `${mp4OutputPath}-keyboard-sound.m4a`;
            const genResult = (await window.ipcRenderer.invoke(
              'video-editor:generate-keyboard-audio',
              {
                keyPresses,
                soundType: config.audioStyle.keyboardSoundType,
                duration,
                outputPath: tempPath,
              }
            )) as { success: boolean; error?: string };

            if (genResult.success) {
              keyboardSoundTempPath = tempPath;
            }
          }
        }

        const result = await exporter.export({
          sourceVideoPath: config.filePath,
          systemAudioPath: config.systemAudioPath,
          micAudioPath: config.micAudioPath,
          systemAudioEnabled: config.audioStyle.systemAudioEnabled,
          micAudioEnabled: config.audioStyle.micAudioEnabled,
          systemAudioVolume: config.audioStyle.systemAudioVolume,
          micAudioVolume: config.audioStyle.micAudioVolume,
          hasEmbeddedAudio: config.hasEmbeddedAudio,
          keyboardSoundPath: keyboardSoundTempPath,
          keyboardSoundVolume: config.audioStyle.keyboardSoundVolume,
          cameraVideoPath: config.cameraVideoPath,
          musicTracks: config.musicTracks,
          outputPath: mp4OutputPath,
          config: {
            videoWidth: config.videoMetadata.width,
            videoHeight: config.videoMetadata.height,
            segments: config.segments,
            wallpaper: config.wallpaper,
            zoomSegments: config.zoomSegments,
            zoomSettings: config.zoomSettings,
            drawingSegments: config.drawingSegments,
            cursorData: config.cursorData,
            cursorStyle: config.cursorStyle,
            cameraStyle: config.cameraStyle,
            keyboardData: config.keyboardData,
            keyboardStyle: config.keyboardStyle,
            subtitleData: config.subtitleData,
            subtitleStyle: config.subtitleStyle,
            firstFrame: config.firstFrame,
            fps: frameRate,
          },
          frameRate,
          qualityPreset: options.qualityPreset,
          resolution: options.resolution,
          exportOptions: options,
          onProgress: progress => {
            const adjustedProgress = isGif
              ? Math.round(progress * 0.7)
              : progress;
            setExportProgress(adjustedProgress);
          },
        });

        if (!result.success) {
          console.error('Export failed:', result.error);
          return;
        }

        if (isGif) {
          setExportProgress(70);

          const gifResult = (await window.ipcRenderer.invoke(
            'video-editor:convert-to-gif',
            {
              inputPath: mp4OutputPath,
              outputPath: finalOutputPath,
              resolution: options.resolution,
              frameRate: options.frameRate,
            }
          )) as { success: boolean; error?: string };

          try {
            await window.ipcRenderer.invoke('video-editor:delete-temp-file', {
              filePath: mp4OutputPath,
            });
          } catch {
            console.warn('Failed to delete temp MP4 file');
          }

          if (!gifResult.success) {
            console.error('GIF conversion failed:', gifResult.error);
            return;
          }
        }

        setExportProgress(100);
        const durationSeconds = (Date.now() - exportStartTime) / 1000;
        await window.ipcRenderer.invoke('video-export:show-completion', {
          durationSeconds,
          filePath: finalOutputPath,
          openInFinder: exportSettings.openInFinder,
        });

        if (config.uploadToCloud) {
          setCloudUploadState('uploading');
          try {
            const uploadResult = (await window.ipcRenderer.invoke(
              'cloud:uploadFile',
              finalOutputPath
            )) as { success: boolean; url?: string; error?: string };

            if (uploadResult.success && uploadResult.url) {
              setUploadedUrl(uploadResult.url);
              setCloudUploadState('success');
            } else if (uploadResult.error === 'Upload cancelled') {
              setCloudUploadState('idle');
            } else {
              setCloudUploadState('error');
            }
          } catch (uploadError) {
            console.error('Cloud upload failed:', uploadError);
            setCloudUploadState('error');
          }
        }
      } catch (error) {
        console.error('Export error:', error);
      } finally {
        if (keyboardSoundTempPath) {
          window.ipcRenderer
            .invoke('video-editor:delete-temp-file', {
              filePath: keyboardSoundTempPath,
            })
            .catch(() => {});
        }
        setIsExporting(false);
        exporterRef.current = null;
      }
    },
    [isExporting, exportSettings.openInFinder]
  );

  const handleCancelExport = useCallback(() => {
    if (exporterRef.current) {
      exporterRef.current.cancel();
      exporterRef.current = null;
    }
    setIsExporting(false);
    setExportProgress(0);
  }, []);

  const restoreExportSettings = useCallback((settings: ExportSettings) => {
    setExportSettings(settings);
  }, []);

  return {
    isExporting,
    exportProgress,
    exportSettings,
    setExportSettings,
    cloudUploadState,
    uploadedUrl,
    copyUploadedUrl,
    cancelCloudUpload,
    handleExport,
    handleCancelExport,
    restoreExportSettings,
  };
}
