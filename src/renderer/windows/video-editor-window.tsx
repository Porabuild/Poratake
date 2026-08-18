import {
  useEffect,
  useRef,
  useState,
  useCallback,
  useMemo,
  type SyntheticEvent,
} from 'react';
import { Camera, Film, PenLine, TriangleAlert, ZoomIn } from 'lucide-react';
import VideoTitleBar from '@/renderer/components/video-editor/video-title-bar';
import NativeVideoPlayer from '@/renderer/components/video-editor/native-video-player';
import EditorSidebar from '@/renderer/components/video-editor/editor-sidebar';
import EditorSidebarTabs from '@/renderer/components/video-editor/editor-sidebar-tabs';
import TimelineControls from '@/renderer/components/video-editor/timeline/timeline-controls';
import TimelineRuler from '@/renderer/components/video-editor/timeline/timeline-ruler';
import TimelineTrack from '@/renderer/components/video-editor/timeline/timeline-track';
import DrawingTrack from '@/renderer/components/video-editor/timeline/drawing-track';
import TimelineTracks from '@/renderer/components/video-editor/timeline/timeline-tracks';
import { TimelineProvider } from '@/renderer/components/video-editor/timeline/timeline-context';
import ZoomTrack from '@/renderer/components/video-editor/timeline/zoom-track';
import CameraTrack from '@/renderer/components/video-editor/timeline/camera-track';
import MusicTrack from '@/renderer/components/video-editor/timeline/music-track';
import TrackRow, {
  TRACK_HEIGHT,
} from '@/renderer/components/video-editor/timeline/track-row';
import { useTimelineZoom } from '@/renderer/components/video-editor/timeline/use-timeline-zoom';
import {
  DEFAULT_PIXELS_PER_SECOND,
  MIN_PIXELS_PER_SECOND,
  MAX_PIXELS_PER_SECOND,
} from '@/renderer/components/video-editor/timeline/timeline-constants';
import { useVideoHistory } from '@/renderer/components/video-editor/hooks/use-video-history';
import { useEditorHistory } from '@/renderer/components/video-editor/hooks/use-editor-history';
import { useEditorStatePersistence } from '@/renderer/components/video-editor/hooks/use-editor-state-persistence';
import { useVideoWallpaper } from '@/renderer/components/video-editor/hooks/use-video-wallpaper';
import { useEditorData } from '@/renderer/components/video-editor/hooks/use-editor-data';
import { useZoomSegments } from '@/renderer/components/video-editor/hooks/use-zoom-segments';
import { useDrawingSegments } from '@/renderer/components/video-editor/hooks/use-drawing-segments';
import { useSegmentOperations } from '@/renderer/components/video-editor/hooks/use-segment-operations';
import { useVideoExport } from '@/renderer/components/video-editor/hooks/use-video-export';
import { useEditorShortcuts } from '@/renderer/components/video-editor/hooks/use-editor-shortcuts';
import { usePlaybackControl } from '@/renderer/components/video-editor/hooks/use-playback-control';
import { useKeyboardSound } from '@/renderer/components/video-editor/hooks/use-keyboard-sound';
import { useSidebarShortcuts } from '@/renderer/components/video-editor/hooks/use-sidebar-shortcuts';
import { useFirstFrame } from '@/renderer/components/video-editor/hooks/use-first-frame';
import { useCameraSegments } from '@/renderer/components/video-editor/hooks/use-camera-segments';
import {
  useMusicTracks,
  buildBuiltInMusicTracks,
  mergeBuiltInMusicTracks,
  withDefaultGroupIds,
} from '@/renderer/components/video-editor/hooks/use-music-tracks';
import { useMusicPlayback } from '@/renderer/components/video-editor/hooks/use-music-playback';
import { useResizablePane } from '@/renderer/components/video-editor/hooks/use-resizable-pane';
import type { VideoEditorSidebarShortcuts } from '@/types/settings';
import { SOURCE_ICONS, groupMusicTracks } from '@/types/music';
import {
  splitVideoSegments,
  splitTrackSegments,
  splitDrawingSegments,
  splitMusicTracks,
} from '@/renderer/components/video-editor/timeline-split';
import {
  DEFAULT_DRAWING_TOOL_SETTINGS,
  MIN_DRAWING_SEGMENT_DURATION,
} from '@/types/drawing';
import type { DrawingToolSettings, VideoDrawingTool } from '@/types/drawing';
import type {
  NativeVideoPlayerHandle,
  Segment,
} from '@/renderer/components/video-editor/types';
import type { SidebarTab } from '@/renderer/components/video-editor/editor-sidebar-panel-loaders';
import type { VideoExportOptions, ProjectRenameResult } from '@/types/video';
import {
  generateAutoZoomSegments,
  mergeAutoZoomSegments,
} from '@/types/auto-zoom';
import {
  hasWallpaperEffect,
  DEFAULT_VIDEO_WALLPAPER,
  IOS_DEVICE_DEFAULT_WALLPAPER,
} from '@/types/video-wallpaper';
import { SVG_WALLPAPER_PRESETS } from '@/renderer/hooks/useWallpaperState';
import {
  adjustTimelineRangeSlices,
  getContentPlaybackState,
  getFileNameFromPath,
  getProjectPath,
  timelineToVideo,
  toFileUrl,
} from '@/renderer/components/video-editor/utils';

const DEFAULT_SIDEBAR_WIDTH = 288;
const MIN_SIDEBAR_WIDTH = 240;
const MAX_SIDEBAR_WIDTH = 560;

interface VideoEditorWindowProps {
  params: {
    filePath: string;
  };
}

export default function VideoEditorWindow({ params }: VideoEditorWindowProps) {
  const [filePath, setFilePath] = useState(params.filePath);
  const nativePlayerRef = useRef<NativeVideoPlayerHandle>(null);
  const timelineRef = useRef<HTMLDivElement>(null);
  const isConfirming = useRef(false);

  const [originalDuration, setOriginalDuration] = useState(0);
  const [videoSrc, setVideoSrc] = useState(() =>
    /^(https?|blob|data|file):/.test(params.filePath)
      ? params.filePath
      : toFileUrl(params.filePath)
  );
  const [videoDimensions, setVideoDimensions] = useState<{
    width: number;
    height: number;
  } | null>(null);
  const [videoLoadFailed, setVideoLoadFailed] = useState(false);
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [sidebarTab, setSidebarTab] = useState<SidebarTab>('cursor');
  const [isScrubAudioEnabled, setIsScrubAudioEnabled] = useState(false);
  const [initialTimelineZoom, setInitialTimelineZoom] = useState(
    DEFAULT_PIXELS_PER_SECOND
  );
  const [sidebarShortcuts, setSidebarShortcuts] =
    useState<VideoEditorSidebarShortcuts | null>(null);
  const [drawingToolSettings, setDrawingToolSettings] =
    useState<DrawingToolSettings>(DEFAULT_DRAWING_TOOL_SETTINGS);
  const [textFocusNonce, setTextFocusNonce] = useState(0);

  const timelineZoomState = useTimelineZoom({
    initialPixelsPerSecond: initialTimelineZoom,
  });

  const activateSidebarTab = useCallback((tab: SidebarTab) => {
    setSidebarTab(tab);
    setIsSidebarOpen(true);
  }, []);

  const history = useEditorHistory();
  const { undo, redo, canUndo, canRedo } = history;

  const editorData = useEditorData({
    cursorStyleSlice: history.cursorStyle,
    cameraStyleSlice: history.cameraStyle,
    keyboardStyleSlice: history.keyboardStyle,
    subtitleStyleSlice: history.subtitleStyle,
    audioStyleSlice: history.audioStyle,
  });

  const {
    segments,
    setSegments,
    setSegmentsWithoutHistory,
    commitSegmentsToHistory,
  } = useVideoHistory(history.segments);

  const firstFrameControl = useFirstFrame(history.firstFrame);

  const {
    wallpaper,
    setEnabled: setWallpaperEnabled,
    setGradient: setWallpaperGradient,
    setBackgroundImage: setWallpaperBackgroundImage,
    setPadding: setWallpaperPadding,
    setCorners: setWallpaperCorners,
    setShadow: setWallpaperShadow,
    setAspectRatio: setWallpaperAspectRatio,
    setDeviceFrame: setWallpaperDeviceFrame,
  } = useVideoWallpaper(history.wallpaper);

  const videoExport = useVideoExport();
  const [uploadToCloud, setUploadToCloud] = useState(false);
  const [cloudConfigured, setCloudConfigured] = useState(false);

  useEffect(() => {
    if (!isSidebarOpen || sidebarTab !== 'export') {
      return;
    }

    const refreshCloudConfigured = () => {
      window.ipcRenderer
        .invoke('cloud:isConfigured')
        .then((configured: boolean) => setCloudConfigured(configured))
        .catch(() => setCloudConfigured(false));
    };

    refreshCloudConfigured();
    window.addEventListener('focus', refreshCloudConfigured);
    return () => window.removeEventListener('focus', refreshCloudConfigured);
  }, [isSidebarOpen, sidebarTab]);

  const previewFrameRate = useMemo(() => {
    const parsed = parseInt(videoExport.exportSettings.frameRate, 10);
    return Number.isFinite(parsed) && parsed > 0 ? parsed : 30;
  }, [videoExport.exportSettings.frameRate]);

  const activeFirstFrameDuration =
    firstFrameControl.firstFrame.enabled &&
    firstFrameControl.firstFrame.imageData
      ? 1 / previewFrameRate
      : 0;

  const playback = usePlaybackControl({
    nativePlayerRef,
    segments,
    firstFrameDuration: activeFirstFrameDuration,
  });
  const contentPlayback = getContentPlaybackState(
    playback.effectiveTimelinePosition,
    activeFirstFrameDuration,
    playback.isPlaying
  );

  const keyboardSound = useKeyboardSound({
    keyboardData: editorData.keyboardData,
    segments,
    enabled: editorData.audioStyle.keyboardSoundEnabled,
    volume: editorData.audioStyle.keyboardSoundVolume,
    soundType: editorData.audioStyle.keyboardSoundType,
    isPlaying: playback.isPlaying,
    timelinePosition: contentPlayback.timelinePosition,
  });

  const zoomControl = useZoomSegments({
    totalTimelineDuration: playback.totalTimelineDuration,
    activateSidebarTab,
    segmentsSlice: history.zoomSegments,
    settingsSlice: history.zoomSettings,
  });

  const { setZoomSegments } = zoomControl;

  const handleGenerateAutoZoom = useCallback(() => {
    const cursorData = editorData.cursorData;
    if (!cursorData) return;

    const generated = generateAutoZoomSegments(cursorData);
    if (generated.length === 0) return;

    setZoomSegments(prev => mergeAutoZoomSegments(prev, generated));
  }, [editorData.cursorData, setZoomSegments]);

  const drawingControl = useDrawingSegments({
    totalTimelineDuration: playback.totalTimelineDuration,
    slice: history.drawingSegments,
  });

  const musicControl = useMusicTracks({
    totalTimelineDuration: playback.totalTimelineDuration,
    slice: history.musicTracks,
  });
  const setMusicTracksWithoutHistory = history.musicTracks.setWithoutHistory;

  useMusicPlayback({
    musicTracks: musicControl.musicTracks,
    timelinePosition: contentPlayback.timelinePosition,
    isPlaying: contentPlayback.isPlaying,
    systemAudioPath: editorData.systemAudioPath,
    micAudioPath: editorData.micAudioPath,
    embeddedAudioPath: editorData.hasEmbeddedAudio ? filePath : null,
  });

  const { loadedState, isStateLoaded, recordingType, resetState } =
    useEditorStatePersistence({
      isReady: originalDuration > 0,
      values: {
        segments,
        cursorStyle: editorData.cursorStyle,
        cameraStyle: editorData.cameraStyle,
        keyboardStyle: editorData.keyboardStyle,
        subtitleStyle: editorData.subtitleStyle,
        audioStyle: editorData.audioStyle,
        zoomSegments: zoomControl.zoomSegments,
        zoomSettings: zoomControl.zoomSettings,
        cameraSegments: history.cameraSegments.value,
        drawingSegments: drawingControl.drawingSegments,
        wallpaper,
        firstFrame: firstFrameControl.firstFrame,
        musicTracks: musicControl.musicTracks,
        exportSettings: videoExport.exportSettings,
        timelineZoom: timelineZoomState.pixelsPerSecond,
        sidebarOpen: isSidebarOpen,
        sidebarTab,
        scrubAudioEnabled: isScrubAudioEnabled,
      },
    });

  const cameraControl = useCameraSegments({
    ready: segments.length > 0,
    hasCameraData: editorData.cameraData !== null,
    hasSavedSegments: loadedState?.cameraSegments !== undefined,
    initialVisibleRanges: editorData.cameraData?.meta?.visibleRanges ?? null,
    segments,
    totalTimelineDuration: playback.totalTimelineDuration,
    slice: history.cameraSegments,
  });

  const cameraVisibleRanges = editorData.cameraData
    ? cameraControl.cameraSegments
    : null;

  const handleTimelineRangesAdjust = useCallback(
    (
      _segmentIndex: number,
      oldSegmentDuration: number,
      newSegmentDuration: number,
      segmentStartOnTimeline: number,
      segmentEndOnTimeline: number,
      newTotalDuration: number,
      nextSegments: Segment[]
    ) => {
      const adjustment = {
        oldSegmentDuration,
        newSegmentDuration,
        segmentStartOnTimeline,
        segmentEndOnTimeline,
        newTotalDuration,
      };

      history.replaceDocument(
        adjustTimelineRangeSlices({
          nextSegments,
          zoomSegments: zoomControl.zoomSegments,
          cameraSegments: cameraControl.cameraSegments,
          drawingSegments: drawingControl.drawingSegments,
          adjustment,
          drawingMinDuration: MIN_DRAWING_SEGMENT_DURATION,
        })
      );
    },
    [
      cameraControl.cameraSegments,
      drawingControl.drawingSegments,
      history,
      zoomControl.zoomSegments,
    ]
  );

  const segmentOps = useSegmentOperations({
    segments,
    setSegments,
    setSegmentsWithoutHistory,
    commitSegmentsToHistory,
    totalTimelineDuration: playback.totalTimelineDuration,
    originalDuration,
    pixelsPerSecond: timelineZoomState.pixelsPerSecond,
    nativePlayerRef,
    timelineRef,
    setTimelinePosition: playback.setTimelinePosition,
    onTimelineRangesAdjust: handleTimelineRangesAdjust,
  });

  const [displayTimelineDuration, setDisplayTimelineDuration] = useState(0);
  const musicTimelineDuration = useMemo(
    () =>
      musicControl.musicTracks.reduce(
        (duration, track) =>
          track.source === 'music' && track.enabled
            ? Math.max(duration, track.endTime)
            : duration,
        0
      ),
    [musicControl.musicTracks]
  );
  const editingTimelineDuration = Math.max(
    displayTimelineDuration,
    musicTimelineDuration
  );

  useEffect(() => {
    setDisplayTimelineDuration(0);
  }, [filePath]);

  useEffect(() => {
    setDisplayTimelineDuration(prev =>
      Math.max(prev, playback.totalTimelineDuration)
    );
  }, [playback.totalTimelineDuration]);

  const fileName = useMemo(() => getFileNameFromPath(filePath), [filePath]);
  const projectPath = useMemo(() => getProjectPath(filePath), [filePath]);

  const exportVideoMetadata = useMemo(() => {
    if (editorData.videoMetadata) return editorData.videoMetadata;
    if (!videoDimensions || originalDuration <= 0) return null;
    return {
      fileSize: 0,
      bitrate: 0,
      width: videoDimensions.width,
      height: videoDimensions.height,
      duration: originalDuration,
    };
  }, [editorData.videoMetadata, videoDimensions, originalDuration]);

  const handleExport = useCallback(
    (options: VideoExportOptions) => {
      return videoExport.handleExport(options, {
        filePath,
        fileName,
        videoMetadata: exportVideoMetadata,
        segments,
        wallpaper,
        zoomSegments: zoomControl.zoomSegments,
        zoomSettings: zoomControl.zoomSettings,
        drawingSegments: drawingControl.drawingSegments,
        cursorData: editorData.cursorData,
        cursorStyle: editorData.cursorStyle,
        cameraStyle: editorData.cameraStyle,
        cameraVisibleRanges,
        cameraVideoPath: editorData.cameraVideoPath,
        systemAudioPath: editorData.systemAudioPath,
        micAudioPath: editorData.micAudioPath,
        audioStyle: editorData.audioStyle,
        hasEmbeddedAudio: editorData.hasEmbeddedAudio,
        keyboardData: editorData.keyboardData,
        keyboardStyle: editorData.keyboardStyle,
        subtitleData: editorData.subtitleData,
        subtitleStyle: editorData.subtitleStyle,
        firstFrame: firstFrameControl.firstFrame,
        musicTracks: musicControl.musicTracks,
        uploadToCloud,
      });
    },
    [
      videoExport,
      filePath,
      fileName,
      editorData,
      exportVideoMetadata,
      segments,
      wallpaper,
      zoomControl.zoomSegments,
      zoomControl.zoomSettings,
      cameraVisibleRanges,
      drawingControl.drawingSegments,
      firstFrameControl.firstFrame,
      musicControl.musicTracks,
      uploadToCloud,
    ]
  );

  const handleDeleteVideo = useCallback(async () => {
    if (isConfirming.current) return;
    isConfirming.current = true;

    try {
      const confirmed = await window.ipcRenderer.invoke(
        'video-editor:confirmDelete'
      );

      if (confirmed) {
        window.ipcRenderer.send('video-editor:delete');
      }
    } finally {
      isConfirming.current = false;
    }
  }, []);

  const handleRename = useCallback(
    async (newName: string): Promise<string | null> => {
      const result = (await window.ipcRenderer.invoke(
        'project:rename',
        newName
      )) as ProjectRenameResult;

      if (!result.success) {
        return result.error ?? 'Failed to rename project';
      }

      setFilePath(result.newVideoPath);
      setVideoSrc(toFileUrl(result.newVideoPath));
      return null;
    },
    []
  );

  const handleReset = useCallback(async () => {
    if (isConfirming.current) return;
    isConfirming.current = true;

    try {
      const confirmed = await window.ipcRenderer.invoke(
        'video-editor:confirmReset'
      );

      if (confirmed) {
        const success = await resetState();
        if (success) {
          window.location.reload();
        }
      }
    } finally {
      isConfirming.current = false;
    }
  }, [resetState]);

  const handleEscape = useCallback(() => {
    segmentOps.clearSegmentSelection();
    zoomControl.clearZoomSelection();
    cameraControl.clearCameraSelection();
    drawingControl.clearDrawingSelection();
    musicControl.clearMusicSelection();
  }, [segmentOps, zoomControl, cameraControl, drawingControl, musicControl]);

  const handleSegmentSelect = useCallback(
    (segmentId: string | null) => {
      segmentOps.handleSegmentSelect(segmentId);
      if (segmentId !== null) {
        zoomControl.clearZoomSelection();
        cameraControl.clearCameraSelection();
        drawingControl.clearDrawingSelection();
        musicControl.clearMusicSelection();
      }
    },
    [segmentOps, zoomControl, cameraControl, drawingControl, musicControl]
  );

  const handleZoomSelect = useCallback(
    (id: string | null) => {
      if (segmentOps.isCutToolActive) return;
      zoomControl.handleZoomSelect(id);
      if (id !== null) {
        segmentOps.setSelectedSegmentId(null);
        cameraControl.clearCameraSelection();
        drawingControl.clearDrawingSelection();
        musicControl.clearMusicSelection();
      }
    },
    [zoomControl, segmentOps, cameraControl, drawingControl, musicControl]
  );

  const handleCameraSelect = useCallback(
    (id: string | null) => {
      if (segmentOps.isCutToolActive) return;
      cameraControl.handleCameraSelect(id);
      if (id !== null) {
        segmentOps.setSelectedSegmentId(null);
        zoomControl.clearZoomSelection();
        drawingControl.clearDrawingSelection();
        musicControl.clearMusicSelection();
      }
    },
    [cameraControl, segmentOps, zoomControl, drawingControl, musicControl]
  );

  const handleDrawingSelect = useCallback(
    (id: string | null, addToSelection = false) => {
      if (segmentOps.isCutToolActive) return;
      drawingControl.handleSelectDrawingSegment(id, addToSelection);
      if (id !== null) {
        segmentOps.setSelectedSegmentId(null);
        zoomControl.clearZoomSelection();
        cameraControl.clearCameraSelection();
        musicControl.clearMusicSelection();
        activateSidebarTab('drawing');
      }
    },
    [
      drawingControl,
      segmentOps,
      zoomControl,
      cameraControl,
      musicControl,
      activateSidebarTab,
    ]
  );

  const handleAnnotationAdded = useCallback((tool: VideoDrawingTool) => {
    setDrawingToolSettings(prev => ({ ...prev, activeTool: 'select' }));
    if (tool === 'text') {
      setTextFocusNonce(nonce => nonce + 1);
    }
  }, []);

  const handleMusicSelect = useCallback(
    (id: string | null) => {
      if (segmentOps.isCutToolActive) return;
      musicControl.handleSelectMusicTrack(id);
      if (id !== null) {
        segmentOps.setSelectedSegmentId(null);
        zoomControl.clearZoomSelection();
        cameraControl.clearCameraSelection();
        drawingControl.clearDrawingSelection();
      }
    },
    [musicControl, segmentOps, zoomControl, cameraControl, drawingControl]
  );

  const handleZoomCut = useCallback(
    (cutTime: number) => {
      zoomControl.handleSplitZoom(cutTime);
      playback.seekToTimelinePosition(cutTime);
    },
    [zoomControl, playback]
  );

  const handleDrawingCut = useCallback(
    (id: string, cutTime: number) => {
      drawingControl.handleSplitDrawingSegment(id, cutTime);
      playback.seekToTimelinePosition(cutTime);
    },
    [drawingControl, playback]
  );

  const handleMusicCut = useCallback(
    (id: string, cutTime: number) => {
      musicControl.handleSplitMusicTrack(id, cutTime);
      playback.seekToTimelinePosition(cutTime);
    },
    [musicControl, playback]
  );

  const handleCameraCut = useCallback(
    (cutTime: number) => {
      cameraControl.handleSplitCamera(cutTime);
      playback.seekToTimelinePosition(cutTime);
    },
    [cameraControl, playback]
  );

  const handleCutAll = useCallback(
    (cutTime: number) => {
      const { videoTime } = timelineToVideo(segments, cutTime);
      const nextSegments = splitVideoSegments(segments, videoTime) ?? segments;
      const nextZoomSegments = splitTrackSegments(
        zoomControl.zoomSegments,
        cutTime
      );
      const nextCameraSegments = splitTrackSegments(
        cameraControl.cameraSegments,
        cutTime
      );
      const nextDrawingSegments = splitDrawingSegments(
        drawingControl.drawingSegments,
        cutTime
      );
      const nextMusicTracks = splitMusicTracks(
        musicControl.musicTracks,
        cutTime
      );

      if (
        nextSegments === segments &&
        nextZoomSegments === zoomControl.zoomSegments &&
        nextCameraSegments === cameraControl.cameraSegments &&
        nextDrawingSegments === drawingControl.drawingSegments &&
        nextMusicTracks === musicControl.musicTracks
      ) {
        return;
      }

      history.replaceDocument({
        segments: nextSegments,
        zoomSegments: nextZoomSegments,
        cameraSegments: nextCameraSegments,
        drawingSegments: nextDrawingSegments,
        musicTracks: nextMusicTracks,
      });
      playback.seekToTimelinePosition(cutTime);
    },
    [
      segments,
      zoomControl.zoomSegments,
      cameraControl.cameraSegments,
      drawingControl.drawingSegments,
      musicControl.musicTracks,
      history,
      playback,
    ]
  );

  const handleToggleCutTool = useCallback(() => {
    segmentOps.toggleCutTool();
    zoomControl.clearZoomSelection();
    cameraControl.clearCameraSelection();
    drawingControl.clearDrawingSelection();
    musicControl.clearMusicSelection();
  }, [segmentOps, zoomControl, cameraControl, drawingControl, musicControl]);

  const musicTrackGroups = useMemo(
    () => groupMusicTracks(musicControl.musicTracks),
    [musicControl.musicTracks]
  );

  const enabledMusicTrackGroups = useMemo(
    () =>
      groupMusicTracks(musicControl.musicTracks.filter(track => track.enabled)),
    [musicControl.musicTracks]
  );

  const getSegmentIndex = useCallback(
    (id: string) => segments.findIndex(s => s.id === id),
    [segments]
  );

  const handleFitToView = useCallback(() => {
    const container = timelineRef.current;
    const duration = playback.totalTimelineDuration;
    if (!container || duration <= 0) return;

    const target = container.clientWidth / duration;
    const clamped = Math.max(
      MIN_PIXELS_PER_SECOND,
      Math.min(MAX_PIXELS_PER_SECOND, target)
    );
    timelineZoomState.setZoomLevel(clamped);
    container.scrollLeft = 0;
  }, [playback.totalTimelineDuration, timelineZoomState]);

  const getTimelinePosition = useCallback(
    () => playback.timelinePosition,
    [playback.timelinePosition]
  );

  const getTotalTimelineDuration = useCallback(
    () => playback.totalTimelineDuration,
    [playback.totalTimelineDuration]
  );

  useEditorShortcuts({
    selectedSegmentId: segmentOps.selectedSegmentId,
    selectedZoomId: zoomControl.selectedZoomId,
    selectedCameraId: cameraControl.selectedCameraId,
    selectedDrawingId: drawingControl.selectedDrawingId,
    segmentsLength: segments.length,
    onDeleteSegment: segmentOps.handleDeleteSegment,
    onDeleteZoom: zoomControl.handleDeleteZoom,
    onDeleteCamera: cameraControl.handleDeleteCamera,
    onDeleteDrawing: drawingControl.handleDeleteSelectedDrawings,
    onDeleteVideo: handleDeleteVideo,
    onTogglePlayPause: playback.togglePlayPause,
    onToggleCutTool: handleToggleCutTool,
    onUndo: undo,
    onRedo: redo,
    onEscape: handleEscape,
    onReorderSegment: segmentOps.handleReorderSegment,
    getSegmentIndex,
    activateSidebarTab,
    onTimelineZoomIn: timelineZoomState.zoomIn,
    onTimelineZoomOut: timelineZoomState.zoomOut,
    onTimelineZoomReset: timelineZoomState.resetZoom,
    onTimelineFitToView: handleFitToView,
    getTimelinePosition,
    getTotalTimelineDuration,
    onSeekTimeline: playback.seekToTimelinePosition,
  });

  useSidebarShortcuts({
    shortcuts: sidebarShortcuts ?? undefined,
    onTabChange: activateSidebarTab,
  });

  useEffect(() => {
    const loadSettings = async () => {
      try {
        const settings = await window.ipcRenderer.invoke('settings:get-ui');
        if (settings?.shortcuts?.videoEditorSidebar) {
          setSidebarShortcuts(settings.shortcuts.videoEditorSidebar);
        }
      } catch {
        // Ignore settings load errors, use defaults
      }
    };
    loadSettings();
  }, []);

  useEffect(() => {
    if (fileName) {
      document.title = `${fileName} - Poratake`;
    }
    return () => {
      document.title = 'Poratake';
    };
  }, [fileName]);

  useEffect(() => {
    if (
      editorData.videoMetadata?.duration &&
      editorData.videoMetadata.duration > 0
    ) {
      setOriginalDuration(current =>
        current > 0 ? current : editorData.videoMetadata!.duration
      );
    }
  }, [editorData.videoMetadata]);

  useEffect(() => {
    if (originalDuration > 0) return;
    const savedDuration = loadedState?.sourceDuration;
    if (savedDuration !== undefined && savedDuration > 0) {
      setOriginalDuration(savedDuration);
    }
  }, [loadedState, originalDuration]);

  useEffect(() => {
    if (originalDuration <= 0 || !isStateLoaded || segments.length > 0) {
      return;
    }

    const defaultSegments = [
      {
        id: crypto.randomUUID(),
        originalStart: 0,
        originalEnd: originalDuration,
        trimMinStart: 0,
        trimMaxEnd: originalDuration,
      },
    ];

    const iosWallpaper = () => {
      const randomPreset =
        SVG_WALLPAPER_PRESETS[
          Math.floor(Math.random() * SVG_WALLPAPER_PRESETS.length)
        ];
      return {
        ...DEFAULT_VIDEO_WALLPAPER,
        ...IOS_DEVICE_DEFAULT_WALLPAPER,
        backgroundImage: randomPreset.imageUrl,
      };
    };

    if (!loadedState) {
      history.initializeDocument({
        segments: defaultSegments,
        wallpaper:
          recordingType === 'ios-device'
            ? iosWallpaper()
            : DEFAULT_VIDEO_WALLPAPER,
      });
      return;
    }

    const validSegments = loadedState.segments.filter(
      seg =>
        seg.originalStart >= 0 &&
        seg.originalEnd <= originalDuration &&
        seg.originalStart < seg.originalEnd
    );

    const savedMusicTracks = withDefaultGroupIds(loadedState.musicTracks ?? []);

    history.initializeDocument({
      segments: validSegments.length > 0 ? validSegments : defaultSegments,
      zoomSegments: loadedState.zoomSegments,
      zoomSettings: loadedState.zoomSettings,
      cameraSegments: loadedState.cameraSegments,
      drawingSegments: loadedState.drawingSegments ?? [],
      musicTracks: savedMusicTracks.length > 0 ? savedMusicTracks : undefined,
      wallpaper: loadedState.wallpaper
        ? loadedState.wallpaper
        : recordingType === 'ios-device'
          ? iosWallpaper()
          : DEFAULT_VIDEO_WALLPAPER,
      firstFrame: loadedState.firstFrame ?? undefined,
      cursorStyle: loadedState.cursorStyle,
      cameraStyle: loadedState.cameraStyle,
      keyboardStyle: loadedState.keyboardStyle,
      subtitleStyle: loadedState.subtitleStyle,
      audioStyle: loadedState.audioStyle,
    });

    if (loadedState.exportSettings) {
      videoExport.restoreExportSettings(loadedState.exportSettings);
    }
    if (loadedState.timelineZoom) {
      setInitialTimelineZoom(loadedState.timelineZoom);
    }
    setIsSidebarOpen(loadedState.ui.sidebarOpen);
    setSidebarTab(loadedState.ui.sidebarTab);
    setIsScrubAudioEnabled(loadedState.ui.scrubAudioEnabled ?? false);
  }, [
    originalDuration,
    segments.length,
    isStateLoaded,
    loadedState,
    recordingType,
    history,
    videoExport,
  ]);

  useEffect(() => {
    if (!editorData.audioPathsLoaded || segments.length === 0) return;

    const builtInTracks = buildBuiltInMusicTracks({
      systemAudioPath: editorData.systemAudioPath,
      micAudioPath: editorData.micAudioPath,
      hasEmbeddedAudio: editorData.hasEmbeddedAudio,
      originalDuration,
    });
    setMusicTracksWithoutHistory(existing =>
      mergeBuiltInMusicTracks(existing, builtInTracks)
    );
  }, [
    editorData.audioPathsLoaded,
    editorData.systemAudioPath,
    editorData.micAudioPath,
    editorData.hasEmbeddedAudio,
    originalDuration,
    segments.length,
    setMusicTracksWithoutHistory,
  ]);

  const handleBootstrapMetadata = useCallback(
    (event: SyntheticEvent<HTMLVideoElement>) => {
      const video = event.currentTarget;
      if (Number.isFinite(video.duration) && video.duration > 0) {
        setOriginalDuration(Math.round(video.duration * 100) / 100);
      }
      if (video.videoWidth > 0 && video.videoHeight > 0) {
        setVideoDimensions({
          width: video.videoWidth,
          height: video.videoHeight,
        });
      }
      setVideoLoadFailed(false);
    },
    []
  );

  const handleLoadedMetadata = useCallback(() => {}, []);

  const toggleSidebar = useCallback(() => {
    setIsSidebarOpen(prev => !prev);
  }, []);

  const handlePreviewSeek = useCallback(
    (pos: number | null) => {
      if (pos === null) {
        playback.setPreviewTimelinePosition(prev => {
          if (prev === null) return null;

          playback.setTimelinePosition(prev);
          nativePlayerRef.current?.seekTo(prev);

          return null;
        });
        nativePlayerRef.current?.setPreviewTime(null);
        return;
      }

      playback.setPreviewTimelinePosition(pos);
      nativePlayerRef.current?.setPreviewTime(pos);
    },
    [playback, nativePlayerRef]
  );

  const TIMELINE_SCROLLBAR_HEIGHT = 12;
  const MIN_TIMELINE_TRACKS = 3;
  const MAX_TIMELINE_TRACKS = 12;
  const DEFAULT_TIMELINE_TRACKS = 5;
  const minTimelineHeight =
    MIN_TIMELINE_TRACKS * TRACK_HEIGHT + TIMELINE_SCROLLBAR_HEIGHT;
  const maxTimelineHeight =
    MAX_TIMELINE_TRACKS * TRACK_HEIGHT + TIMELINE_SCROLLBAR_HEIGHT;
  const defaultTimelineHeight =
    DEFAULT_TIMELINE_TRACKS * TRACK_HEIGHT + TIMELINE_SCROLLBAR_HEIGHT;

  const {
    size: timelineHeight,
    isResizing: isResizingTimeline,
    startResize: startTimelineResize,
  } = useResizablePane({
    storageKey: 'video-editor:timeline-height',
    axis: 'vertical',
    defaultSize: defaultTimelineHeight,
    minSize: minTimelineHeight,
    maxSize: maxTimelineHeight,
  });

  const {
    size: sidebarWidth,
    isResizing: isResizingSidebar,
    startResize: startSidebarResize,
  } = useResizablePane({
    storageKey: 'video-editor:sidebar-width',
    axis: 'horizontal',
    defaultSize: DEFAULT_SIDEBAR_WIDTH,
    minSize: MIN_SIDEBAR_WIDTH,
    maxSize: MAX_SIDEBAR_WIDTH,
  });

  const hasScrubAudioSource =
    editorData.hasEmbeddedAudio ||
    !!editorData.systemAudioPath ||
    !!editorData.micAudioPath;

  useEffect(() => {
    if (hasScrubAudioSource) return;
    setIsScrubAudioEnabled(false);
  }, [hasScrubAudioSource]);

  const isEditorReady = isStateLoaded && segments.length > 0;

  if (!isEditorReady) {
    if (editorData.videoMetadataStatus === 'unavailable' && videoLoadFailed) {
      return (
        <div className="flex h-screen w-full flex-col items-center justify-center gap-3 bg-background px-10 text-center select-none">
          <TriangleAlert className="size-8 text-muted-foreground" />
          <p className="text-sm font-medium">Could not read this recording</p>
          <p className="max-w-md text-xs text-muted-foreground">
            Poratake could not determine the video duration, so the editor
            cannot open. The recording may be corrupted, or the bundled FFmpeg
            binary may be missing from this build.
          </p>
          <p className="max-w-md font-mono text-xs break-all text-muted-foreground">
            {filePath}
          </p>
        </div>
      );
    }

    return (
      <div className="flex h-screen w-full items-center justify-center bg-background select-none">
        <span className="text-sm text-muted-foreground">
          Loading recording...
        </span>
        <video
          src={videoSrc}
          preload="metadata"
          onLoadedMetadata={handleBootstrapMetadata}
          onError={() => setVideoLoadFailed(true)}
          className="hidden"
        />
      </div>
    );
  }

  return (
    <div className="flex h-screen w-full flex-col bg-background pt-10 select-none">
      <VideoTitleBar
        fileName={fileName}
        projectPath={projectPath}
        onDelete={handleDeleteVideo}
        onUndo={undo}
        onRedo={redo}
        onReset={handleReset}
        canUndo={canUndo}
        canRedo={canRedo}
        isSidebarOpen={isSidebarOpen}
        onToggleSidebar={toggleSidebar}
        isExporting={videoExport.isExporting}
        exportProgress={videoExport.exportProgress}
        onCancelExport={videoExport.handleCancelExport}
        onRename={handleRename}
      />

      <div className="flex min-h-0 flex-1">
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          <div className="flex min-h-0 flex-1 flex-col">
            <NativeVideoPlayer
              ref={nativePlayerRef}
              videoSrc={videoSrc}
              systemAudioEnabled={false}
              micAudioEnabled={false}
              hasEmbeddedAudio={false}
              segments={segments}
              width={
                editorData.videoMetadata?.width ??
                videoDimensions?.width ??
                1920
              }
              height={
                editorData.videoMetadata?.height ??
                videoDimensions?.height ??
                1080
              }
              fps={previewFrameRate}
              durationInSeconds={originalDuration}
              cursorData={editorData.cursorData}
              cursorStyle={editorData.cursorStyle}
              zoomSegments={zoomControl.zoomSegments}
              zoomSettings={zoomControl.zoomSettings}
              drawingSegments={drawingControl.drawingSegments}
              drawingToolSettings={
                sidebarTab === 'drawing' ? drawingToolSettings : null
              }
              drawingTimelinePosition={playback.effectiveTimelinePosition}
              selectedDrawingIds={drawingControl.selectedDrawingIds}
              onAddDrawingSegment={drawingControl.handleAddDrawingSegment}
              onSelectDrawing={handleDrawingSelect}
              onSelectMultipleDrawings={
                drawingControl.handleSelectMultipleDrawings
              }
              onSelectAllDrawings={drawingControl.handleSelectMultipleDrawings}
              onUpdateDrawingAnnotation={
                drawingControl.handleUpdateDrawingAnnotationLive
              }
              onUpdateDrawingAnnotationsMultiple={
                drawingControl.handleUpdateDrawingAnnotationsMultiple
              }
              onCommitDrawingGesture={drawingControl.handleCommitDrawingGesture}
              onAnnotationAdded={handleAnnotationAdded}
              cameraSrc={editorData.cameraSrc}
              cameraStyle={editorData.cameraStyle}
              cameraVisibleRanges={cameraVisibleRanges}
              cameraDurationInFrames={
                editorData.cameraData
                  ? Math.ceil(editorData.cameraData.meta.duration * 30)
                  : 0
              }
              keyboardData={editorData.keyboardData}
              keyboardStyle={editorData.keyboardStyle}
              subtitleData={editorData.subtitleData}
              subtitleStyle={editorData.subtitleStyle}
              wallpaper={wallpaper}
              firstFrame={firstFrameControl.firstFrame}
              scrubAudioEnabled={isScrubAudioEnabled}
              onLoadedMetadata={handleLoadedMetadata}
              onTimeUpdate={playback.handleTimeUpdate}
              onPlayingChange={playback.handlePlayingChange}
            />
          </div>

          <div className="flex shrink-0 flex-col border-t border-border bg-card">
            <div
              role="separator"
              aria-orientation="horizontal"
              onMouseDown={startTimelineResize}
              className={`group flex h-1.5 shrink-0 cursor-ns-resize items-center justify-center ${
                isResizingTimeline ? 'bg-primary/40' : 'hover:bg-primary/20'
              }`}
            >
              <div
                className={`h-0.5 w-8 rounded-full transition-colors ${
                  isResizingTimeline
                    ? 'bg-primary'
                    : 'bg-muted-foreground/30 group-hover:bg-muted-foreground/50'
                }`}
              />
            </div>
            <TimelineControls
              isPlaying={playback.isPlaying}
              isCutToolActive={segmentOps.isCutToolActive}
              hasSelectedSegment={segmentOps.selectedSegmentId !== null}
              canDeleteSegment={segments.length > 1}
              timelinePosition={playback.timelinePosition}
              totalTimelineDuration={playback.totalTimelineDuration}
              segmentCount={segments.length}
              selectedSegmentSpeed={segmentOps.selectedSegmentSpeed}
              onTogglePlayPause={playback.togglePlayPause}
              onToggleCutTool={handleToggleCutTool}
              onDeleteSegment={segmentOps.handleDeleteSegment}
              onSpeedChange={segmentOps.handleSpeedChange}
              pixelsPerSecond={timelineZoomState.pixelsPerSecond}
              onZoomIn={timelineZoomState.zoomIn}
              onZoomOut={timelineZoomState.zoomOut}
              onZoomChange={timelineZoomState.setZoomLevel}
              onFitToView={handleFitToView}
              canZoomIn={timelineZoomState.canZoomIn}
              canZoomOut={timelineZoomState.canZoomOut}
              scrubAudioEnabled={isScrubAudioEnabled}
              onScrubAudioChange={setIsScrubAudioEnabled}
              isScrubAudioAvailable={hasScrubAudioSource}
            />

            <TimelineProvider
              initialPixelsPerSecond={timelineZoomState.pixelsPerSecond}
              onZoomChange={timelineZoomState.setZoomLevel}
            >
              <TimelineRuler
                totalDuration={playback.totalTimelineDuration}
                minDisplayDuration={editingTimelineDuration}
              />

              <div
                id="timeline-container"
                className="scrollbar-overlay-vertical flex items-start overflow-y-auto"
                style={{ height: timelineHeight }}
              >
                <div className="flex w-10 shrink-0 flex-col border-r border-border">
                  <TrackRow className="flex items-center justify-center">
                    <Film className="size-4 text-muted-foreground" />
                  </TrackRow>
                  <TrackRow className="flex items-center justify-center">
                    <ZoomIn className="size-4 text-muted-foreground" />
                  </TrackRow>
                  {editorData.cameraData && (
                    <TrackRow className="flex items-center justify-center">
                      <Camera className="size-4 text-muted-foreground" />
                    </TrackRow>
                  )}
                  {drawingControl.drawingSegments.length === 0 ? (
                    <TrackRow className="flex items-center justify-center">
                      <PenLine className="size-4 text-muted-foreground" />
                    </TrackRow>
                  ) : (
                    drawingControl.drawingSegments.map(drawing => (
                      <TrackRow
                        key={drawing.id}
                        className="flex items-center justify-center"
                      >
                        <PenLine className="size-4 text-muted-foreground" />
                      </TrackRow>
                    ))
                  )}
                  {enabledMusicTrackGroups.map(group => {
                    const Icon = SOURCE_ICONS[group[0].source];
                    return (
                      <TrackRow
                        key={group[0].groupId}
                        className="flex items-center justify-center"
                      >
                        <Icon className="size-4 text-muted-foreground" />
                      </TrackRow>
                    );
                  })}
                </div>
                <TimelineTracks
                  ref={timelineRef}
                  totalDuration={playback.totalTimelineDuration}
                  minDisplayDuration={editingTimelineDuration}
                  playheadPosition={playback.playheadPosition}
                  isPlaying={playback.isPlaying}
                  isTrimming={segmentOps.trimState !== null}
                  onPreviewSeek={handlePreviewSeek}
                >
                  <TimelineTrack
                    segments={segments}
                    selectedSegmentId={segmentOps.selectedSegmentId}
                    isCutToolActive={segmentOps.isCutToolActive}
                    trimState={segmentOps.trimState}
                    onSegmentSelect={handleSegmentSelect}
                    onTrimStart={segmentOps.handleTrimStart}
                    onCut={segmentOps.handleCut}
                    onCutAll={handleCutAll}
                    onReorder={segmentOps.handleReorderSegment}
                    onSeek={playback.seekToTimelinePosition}
                  />

                  <ZoomTrack
                    segments={zoomControl.zoomSegments}
                    totalDuration={playback.totalTimelineDuration}
                    selectedId={zoomControl.selectedZoomId}
                    isCutToolActive={segmentOps.isCutToolActive}
                    onSelect={handleZoomSelect}
                    onResize={zoomControl.handleUpdateZoom}
                    onMove={zoomControl.handleUpdateZoom}
                    onGestureEnd={zoomControl.handleCommitZoomGesture}
                    onAdd={zoomControl.handleAddZoom}
                    onCut={handleZoomCut}
                    onCutAll={handleCutAll}
                    onUpdateZoomLevel={zoomControl.handleUpdateZoomLevel}
                    onDelete={zoomControl.handleDeleteZoom}
                    onApplyToAll={zoomControl.handleApplyZoomToAll}
                    onDeleteOthers={zoomControl.handleDeleteOtherZooms}
                  />

                  {editorData.cameraData && (
                    <CameraTrack
                      segments={cameraControl.cameraSegments}
                      totalDuration={playback.totalTimelineDuration}
                      selectedId={cameraControl.selectedCameraId}
                      isCutToolActive={segmentOps.isCutToolActive}
                      onSelect={handleCameraSelect}
                      onResize={cameraControl.handleUpdateCamera}
                      onMove={cameraControl.handleUpdateCamera}
                      onGestureEnd={cameraControl.handleCommitCameraGesture}
                      onAdd={cameraControl.handleAddCamera}
                      onCut={handleCameraCut}
                      onCutAll={handleCutAll}
                      onDelete={cameraControl.handleDeleteCamera}
                    />
                  )}

                  {drawingControl.drawingSegments.length === 0 ? (
                    <TrackRow />
                  ) : (
                    drawingControl.drawingSegments.map(drawing => (
                      <DrawingTrack
                        key={drawing.id}
                        segment={drawing}
                        totalDuration={playback.totalTimelineDuration}
                        selectedId={
                          drawingControl.selectedDrawingIds.includes(drawing.id)
                            ? drawing.id
                            : null
                        }
                        isCutToolActive={segmentOps.isCutToolActive}
                        onSelect={handleDrawingSelect}
                        onResize={drawingControl.handleResizeDrawingSegment}
                        onMove={drawingControl.handleMoveDrawingSegment}
                        onGestureEnd={drawingControl.handleCommitDrawingGesture}
                        onCut={handleDrawingCut}
                        onCutAll={handleCutAll}
                        onDelete={drawingControl.handleDeleteDrawingSegment}
                      />
                    ))
                  )}

                  {enabledMusicTrackGroups.map(group => (
                    <MusicTrack
                      key={group[0].groupId}
                      tracks={group}
                      totalDuration={editingTimelineDuration}
                      selectedId={musicControl.selectedMusicTrackId}
                      isCutToolActive={segmentOps.isCutToolActive}
                      onSelect={handleMusicSelect}
                      onResize={musicControl.handleResizeMusicTrack}
                      onMove={musicControl.handleMoveMusicTrack}
                      onGestureEnd={musicControl.handleCommitMusicGesture}
                      onSpeedChange={(groupId, speed) =>
                        musicControl.handleUpdateMusicTrackGroup(groupId, {
                          speed,
                        })
                      }
                      onCut={handleMusicCut}
                      onCutAll={handleCutAll}
                      onDelete={musicControl.handleRemoveMusicTrackGroup}
                    />
                  ))}
                </TimelineTracks>
              </div>
            </TimelineProvider>
          </div>
        </div>

        <EditorSidebar
          isOpen={isSidebarOpen}
          width={sidebarWidth}
          isResizing={isResizingSidebar}
          onStartResize={startSidebarResize}
          activeTab={sidebarTab}
          cursorStyle={editorData.cursorStyle}
          onCursorStyleChange={editorData.setCursorStyle}
          hasCursorData={editorData.cursorData !== null}
          cursorData={editorData.cursorData}
          videoDuration={playback.totalTimelineDuration}
          videoWidth={editorData.videoMetadata?.width ?? 1920}
          videoHeight={editorData.videoMetadata?.height ?? 1080}
          onCursorDataSave={editorData.handleCursorDataSave}
          onCursorDataImport={editorData.handleCursorDataImport}
          selectedZoomId={zoomControl.selectedZoomId}
          zoomSegments={zoomControl.zoomSegments}
          zoomSettings={zoomControl.zoomSettings}
          onUpdateZoomSegment={zoomControl.handleUpdateZoomSegment}
          onUpdateZoomSettings={zoomControl.setZoomSettings}
          onGenerateAutoZoom={handleGenerateAutoZoom}
          drawingSegments={drawingControl.drawingSegments}
          selectedDrawingId={drawingControl.selectedDrawingId}
          drawingToolSettings={drawingToolSettings}
          textFocusNonce={textFocusNonce}
          onDrawingToolSettingsChange={setDrawingToolSettings}
          onUpdateDrawingAnnotation={
            drawingControl.handleUpdateDrawingAnnotation
          }
          onDeleteDrawingSegment={drawingControl.handleDeleteDrawingSegment}
          videoSrc={videoSrc}
          timelinePosition={playback.effectiveTimelinePosition}
          cameraStyle={editorData.cameraStyle}
          onCameraStyleChange={editorData.setCameraStyle}
          hasCameraData={editorData.cameraData !== null}
          audioStyle={editorData.audioStyle}
          onAudioStyleChange={editorData.setAudioStyle}
          hasMicAudio={!!editorData.micAudioPath}
          hasKeyboardData={editorData.keyboardData !== null}
          musicTrackGroups={musicTrackGroups}
          onAddMusicTrack={musicControl.handleAddMusicTrack}
          onRemoveMusicTrackGroup={musicControl.handleRemoveMusicTrackGroup}
          onUpdateMusicTrackGroup={musicControl.handleUpdateMusicTrackGroup}
          onPlayDemo={keyboardSound.playDemo}
          onStopDemo={keyboardSound.stopDemo}
          isDemoPlaying={keyboardSound.isDemoPlaying}
          keyboardStyle={editorData.keyboardStyle}
          onKeyboardStyleChange={editorData.setKeyboardStyle}
          subtitleStyle={editorData.subtitleStyle}
          onSubtitleStyleChange={editorData.setSubtitleStyle}
          subtitleData={editorData.subtitleData}
          onSubtitleGenerate={editorData.handleSubtitleGenerate}
          onSubtitleDelete={editorData.handleSubtitleDelete}
          onSubtitleDataSave={editorData.handleSubtitleDataSave}
          onSubtitleDataImport={editorData.handleSubtitleDataImport}
          wallpaper={wallpaper}
          onWallpaperEnabledChange={setWallpaperEnabled}
          onWallpaperGradientChange={setWallpaperGradient}
          onWallpaperBackgroundImageChange={setWallpaperBackgroundImage}
          onWallpaperPaddingChange={setWallpaperPadding}
          onWallpaperCornersChange={setWallpaperCorners}
          onWallpaperShadowChange={setWallpaperShadow}
          onWallpaperAspectRatioChange={setWallpaperAspectRatio}
          onWallpaperDeviceFrameChange={setWallpaperDeviceFrame}
          recordingType={recordingType}
          firstFrame={firstFrameControl.firstFrame}
          onFirstFrameImageChange={firstFrameControl.setImageData}
          onFirstFrameFitChange={firstFrameControl.setFit}
          exportSettings={videoExport.exportSettings}
          onExportSettingsChange={videoExport.setExportSettings}
          onExport={handleExport}
          isExporting={videoExport.isExporting}
          exportProgress={videoExport.exportProgress}
          onCancelExport={videoExport.handleCancelExport}
          exportError={videoExport.exportError}
          videoDurationSeconds={playback.totalTimelineDuration}
          hasWallpaper={hasWallpaperEffect(wallpaper)}
          uploadToCloud={uploadToCloud}
          onUploadToCloudChange={setUploadToCloud}
          cloudConfigured={cloudConfigured}
          cloudUploadState={videoExport.cloudUploadState}
          uploadedUrl={videoExport.uploadedUrl}
          onCopyUploadedUrl={videoExport.copyUploadedUrl}
          onCancelCloudUpload={videoExport.cancelCloudUpload}
        />

        <EditorSidebarTabs
          activeTab={isSidebarOpen ? sidebarTab : null}
          onTabChange={activateSidebarTab}
          shortcuts={sidebarShortcuts ?? undefined}
        />
      </div>
    </div>
  );
}
