import { lazy, Suspense } from 'react';
import type { CursorData, CursorStyle } from '@/types/cursor';
import type { ZoomSegment, ZoomSettings } from '@/types/zoom';
import type { CameraStyle } from '@/types/camera';
import type { AudioStyle } from '@/types/audio';
import type { MusicTrack } from '@/types/music';
import type { KeyboardStyle } from '@/types/keyboard';
import type {
  SubtitleStyle,
  SubtitleData,
  SubtitleGenerationOptions,
} from '@/types/subtitle';
import type { VideoExportOptions } from '@/types/video';
import type { AspectRatio } from '@/types/aspect-ratio';
import type { VideoWallpaperSettings } from '@/types/video-wallpaper';
import type { Annotation, GradientOption } from '@/types/editor';
import type { ExportSettings } from '@/types/video-editor-state';
import type { CloudUploadState } from '@/types/cloud';
import type { RecordingType } from '@/types/video';
import type { FirstFrameSettings, FirstFrameFit } from '@/types/first-frame';
import type { DrawingSegment, DrawingToolSettings } from '@/types/drawing';
import {
  loadAudioSettingsPanel,
  loadCameraSettingsPanel,
  loadCursorSettingsPanel,
  loadDrawingSettingsPanel,
  loadExportSettingsPanel,
  loadFirstFrameSettingsPanel,
  loadKeyboardSettingsPanel,
  loadSubtitleSettingsPanel,
  loadWallpaperSettingsPanel,
  loadZoomSettingsPanel,
  type SidebarTab,
} from './editor-sidebar-panel-loaders';

export type { SidebarTab } from './editor-sidebar-panel-loaders';

const CursorSettingsPanel = lazy(loadCursorSettingsPanel);
const ZoomSettingsPanel = lazy(loadZoomSettingsPanel);
const DrawingSettingsPanel = lazy(loadDrawingSettingsPanel);
const CameraSettingsPanel = lazy(loadCameraSettingsPanel);
const AudioSettingsPanel = lazy(loadAudioSettingsPanel);
const WallpaperSettingsPanel = lazy(loadWallpaperSettingsPanel);
const KeyboardSettingsPanel = lazy(loadKeyboardSettingsPanel);
const SubtitleSettingsPanel = lazy(loadSubtitleSettingsPanel);
const FirstFrameSettingsPanel = lazy(loadFirstFrameSettingsPanel);
const ExportSettingsPanel = lazy(loadExportSettingsPanel);

interface EditorSidebarProps {
  isOpen: boolean;
  width: number;
  isResizing: boolean;
  onStartResize: (event: React.MouseEvent) => void;
  activeTab: SidebarTab;
  cursorStyle: CursorStyle;
  onCursorStyleChange: (style: CursorStyle) => void;
  hasCursorData: boolean;
  cursorData: CursorData | null;
  videoDuration: number;
  videoWidth: number;
  videoHeight: number;
  onCursorDataSave: (
    data: CursorData
  ) => Promise<{ success: boolean; error?: string }>;
  onCursorDataImport: () => Promise<{ success: boolean; error?: string }>;
  selectedZoomId: string | null;
  zoomSegments: ZoomSegment[];
  zoomSettings: ZoomSettings;
  onUpdateZoomSegment: (id: string, updates: Partial<ZoomSegment>) => void;
  onUpdateZoomSettings: (settings: ZoomSettings) => void;
  onGenerateAutoZoom: () => void;
  drawingSegments: DrawingSegment[];
  selectedDrawingId: string | null;
  drawingToolSettings: DrawingToolSettings;
  textFocusNonce: number;
  onDrawingToolSettingsChange: (settings: DrawingToolSettings) => void;
  onUpdateDrawingAnnotation: (id: string, updates: Partial<Annotation>) => void;
  onDeleteDrawingSegment: (id: string) => void;
  videoSrc: string;
  timelinePosition: number;
  cameraStyle: CameraStyle;
  onCameraStyleChange: (style: CameraStyle) => void;
  hasCameraData: boolean;
  audioStyle: AudioStyle;
  onAudioStyleChange: (style: AudioStyle) => void;
  hasMicAudio: boolean;
  hasKeyboardData: boolean;
  musicTrackGroups: MusicTrack[][];
  onAddMusicTrack: () => void;
  onRemoveMusicTrackGroup: (groupId: string) => void;
  onUpdateMusicTrackGroup: (
    groupId: string,
    updates: Partial<MusicTrack>
  ) => void;
  onPlayDemo: () => void;
  onStopDemo: () => void;
  isDemoPlaying: boolean;
  keyboardStyle: KeyboardStyle;
  onKeyboardStyleChange: (style: KeyboardStyle) => void;
  subtitleStyle: SubtitleStyle;
  onSubtitleStyleChange: (style: SubtitleStyle) => void;
  subtitleData: SubtitleData | null;
  onSubtitleGenerate: (options: SubtitleGenerationOptions) => Promise<void>;
  onSubtitleDelete: () => Promise<void>;
  onSubtitleDataSave: (
    data: SubtitleData
  ) => Promise<{ success: boolean; error?: string }>;
  onSubtitleDataImport: () => Promise<{ success: boolean; error?: string }>;
  wallpaper: VideoWallpaperSettings;
  onWallpaperEnabledChange: (enabled: boolean) => void;
  onWallpaperGradientChange: (gradient: GradientOption | null) => void;
  onWallpaperBackgroundImageChange: (image: string | null) => void;
  onWallpaperPaddingChange: (padding: number) => void;
  onWallpaperCornersChange: (corners: number) => void;
  onWallpaperShadowChange: (shadow: number) => void;
  onWallpaperAspectRatioChange: (aspectRatio: AspectRatio | null) => void;
  onWallpaperDeviceFrameChange: (deviceFrame: boolean) => void;
  recordingType?: RecordingType;
  firstFrame: FirstFrameSettings;
  onFirstFrameImageChange: (imageData: string | null) => void;
  onFirstFrameFitChange: (fit: FirstFrameFit) => void;
  exportSettings: ExportSettings;
  onExportSettingsChange: (settings: ExportSettings) => void;
  onExport: (options: VideoExportOptions) => void;
  isExporting: boolean;
  exportProgress: number;
  onCancelExport: () => void;
  exportError: string | null;
  videoDurationSeconds: number;
  hasWallpaper: boolean;
  uploadToCloud: boolean;
  onUploadToCloudChange: (value: boolean) => void;
  cloudConfigured: boolean;
  cloudUploadState: CloudUploadState;
  uploadedUrl: string | null;
  onCopyUploadedUrl: () => void;
  onCancelCloudUpload: () => void;
}

export default function EditorSidebar({
  isOpen,
  width,
  isResizing,
  onStartResize,
  activeTab,
  cursorStyle,
  onCursorStyleChange,
  hasCursorData,
  cursorData,
  videoDuration,
  videoWidth,
  videoHeight,
  onCursorDataSave,
  onCursorDataImport,
  selectedZoomId,
  zoomSegments,
  zoomSettings,
  onUpdateZoomSegment,
  onUpdateZoomSettings,
  onGenerateAutoZoom,
  drawingSegments,
  selectedDrawingId,
  drawingToolSettings,
  textFocusNonce,
  onDrawingToolSettingsChange,
  onUpdateDrawingAnnotation,
  onDeleteDrawingSegment,
  videoSrc,
  timelinePosition,
  cameraStyle,
  onCameraStyleChange,
  hasCameraData,
  audioStyle,
  onAudioStyleChange,
  hasMicAudio,
  hasKeyboardData,
  musicTrackGroups,
  onAddMusicTrack,
  onRemoveMusicTrackGroup,
  onUpdateMusicTrackGroup,
  onPlayDemo,
  onStopDemo,
  isDemoPlaying,
  keyboardStyle,
  onKeyboardStyleChange,
  subtitleStyle,
  onSubtitleStyleChange,
  subtitleData,
  onSubtitleGenerate,
  onSubtitleDelete,
  onSubtitleDataSave,
  onSubtitleDataImport,
  wallpaper,
  onWallpaperEnabledChange,
  onWallpaperGradientChange,
  onWallpaperBackgroundImageChange,
  onWallpaperPaddingChange,
  onWallpaperCornersChange,
  onWallpaperShadowChange,
  onWallpaperAspectRatioChange,
  onWallpaperDeviceFrameChange,
  recordingType,
  firstFrame,
  onFirstFrameImageChange,
  onFirstFrameFitChange,
  exportSettings,
  onExportSettingsChange,
  onExport,
  isExporting,
  exportProgress,
  onCancelExport,
  exportError,
  videoDurationSeconds,
  hasWallpaper,
  uploadToCloud,
  onUploadToCloudChange,
  cloudConfigured,
  cloudUploadState,
  uploadedUrl,
  onCopyUploadedUrl,
  onCancelCloudUpload,
}: EditorSidebarProps) {
  if (!isOpen) {
    return null;
  }

  const renderContent = () => {
    switch (activeTab) {
      case 'cursor':
        return (
          <CursorSettingsPanel
            cursorStyle={cursorStyle}
            onStyleChange={onCursorStyleChange}
            hasCursorData={hasCursorData}
            cursorData={cursorData}
            videoDuration={videoDuration}
            videoWidth={videoWidth}
            videoHeight={videoHeight}
            onCursorDataSave={onCursorDataSave}
            onCursorDataImport={onCursorDataImport}
          />
        );
      case 'zoom':
        return (
          <ZoomSettingsPanel
            selectedZoomId={selectedZoomId}
            zoomSegments={zoomSegments}
            zoomSettings={zoomSettings}
            onUpdateZoomSegment={onUpdateZoomSegment}
            onUpdateZoomSettings={onUpdateZoomSettings}
            videoSrc={videoSrc}
            timelinePosition={timelinePosition}
            hasCursorData={hasCursorData}
            onGenerateAutoZoom={onGenerateAutoZoom}
          />
        );
      case 'drawing':
        return (
          <DrawingSettingsPanel
            drawingSegments={drawingSegments}
            selectedDrawingId={selectedDrawingId}
            toolSettings={drawingToolSettings}
            textFocusNonce={textFocusNonce}
            onToolSettingsChange={onDrawingToolSettingsChange}
            onUpdateDrawingAnnotation={onUpdateDrawingAnnotation}
            onDeleteDrawingSegment={onDeleteDrawingSegment}
          />
        );
      case 'camera':
        return (
          <CameraSettingsPanel
            cameraStyle={cameraStyle}
            onStyleChange={onCameraStyleChange}
            hasCameraData={hasCameraData}
          />
        );
      case 'audio':
        return (
          <AudioSettingsPanel
            audioStyle={audioStyle}
            onStyleChange={onAudioStyleChange}
            hasKeyboardData={hasKeyboardData}
            onPlayDemo={onPlayDemo}
            onStopDemo={onStopDemo}
            isDemoPlaying={isDemoPlaying}
            musicTrackGroups={musicTrackGroups}
            onAddMusicTrack={onAddMusicTrack}
            onRemoveMusicTrackGroup={onRemoveMusicTrackGroup}
            onUpdateMusicTrackGroup={onUpdateMusicTrackGroup}
          />
        );
      case 'wallpaper':
        return (
          <WallpaperSettingsPanel
            wallpaper={wallpaper}
            onEnabledChange={onWallpaperEnabledChange}
            onGradientChange={onWallpaperGradientChange}
            onBackgroundImageChange={onWallpaperBackgroundImageChange}
            onPaddingChange={onWallpaperPaddingChange}
            onCornersChange={onWallpaperCornersChange}
            onShadowChange={onWallpaperShadowChange}
            onAspectRatioChange={onWallpaperAspectRatioChange}
            onDeviceFrameChange={onWallpaperDeviceFrameChange}
            recordingType={recordingType}
          />
        );
      case 'keyboard':
        return (
          <KeyboardSettingsPanel
            keyboardStyle={keyboardStyle}
            onStyleChange={onKeyboardStyleChange}
            hasKeyboardData={hasKeyboardData}
          />
        );
      case 'subtitle':
        return (
          <SubtitleSettingsPanel
            subtitleStyle={subtitleStyle}
            onStyleChange={onSubtitleStyleChange}
            subtitleData={subtitleData}
            hasMicAudio={hasMicAudio}
            videoDuration={videoDuration}
            onGenerate={onSubtitleGenerate}
            onDelete={onSubtitleDelete}
            onSubtitleDataSave={onSubtitleDataSave}
            onSubtitleDataImport={onSubtitleDataImport}
          />
        );
      case 'first-frame':
        return (
          <FirstFrameSettingsPanel
            firstFrame={firstFrame}
            onImageChange={onFirstFrameImageChange}
            onFitChange={onFirstFrameFitChange}
          />
        );
      case 'export':
        return (
          <ExportSettingsPanel
            exportSettings={exportSettings}
            onExportSettingsChange={onExportSettingsChange}
            onExport={onExport}
            isExporting={isExporting}
            exportProgress={exportProgress}
            onCancelExport={onCancelExport}
            exportError={exportError}
            videoDurationSeconds={videoDurationSeconds}
            hasCamera={hasCameraData}
            hasWallpaper={hasWallpaper}
            uploadToCloud={uploadToCloud}
            onUploadToCloudChange={onUploadToCloudChange}
            cloudConfigured={cloudConfigured}
            cloudUploadState={cloudUploadState}
            uploadedUrl={uploadedUrl}
            onCopyUrl={onCopyUploadedUrl}
            onCancelUpload={onCancelCloudUpload}
          />
        );
    }
  };

  return (
    <div
      className="flex h-full shrink-0 border-l border-border bg-card"
      style={{ width }}
    >
      <div
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize sidebar"
        onMouseDown={onStartResize}
        className={`group flex w-1.5 shrink-0 cursor-ew-resize justify-center ${
          isResizing ? 'bg-primary/40' : 'hover:bg-primary/20'
        }`}
      >
        <div
          className={`my-auto h-8 w-0.5 rounded-full transition-colors ${
            isResizing
              ? 'bg-primary'
              : 'bg-muted-foreground/30 group-hover:bg-muted-foreground/50'
          }`}
        />
      </div>
      <div className="min-w-0 flex-1 overflow-y-auto">
        <Suspense
          fallback={
            <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
              Loading...
            </div>
          }
        >
          {renderContent()}
        </Suspense>
      </div>
    </div>
  );
}
