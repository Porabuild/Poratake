import { useEffect, useCallback, useRef } from 'react';
import { shouldIgnoreGlobalKeyboardShortcuts } from '@/renderer/utils/keyboard';
import type { SidebarTab } from '../editor-sidebar';

interface UseEditorShortcutsProps {
  selectedSegmentId: string | null;
  selectedZoomId: string | null;
  selectedDrawingId: string | null;
  segmentsLength: number;
  onDeleteSegment: () => void;
  onDeleteZoom: (id: string) => void;
  onDeleteDrawing: () => void;
  onDeleteVideo: () => void;
  onTogglePlayPause: () => void;
  onToggleCutTool: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onEscape: () => void;
  onReorderSegment: (segmentId: string, newIndex: number) => void;
  getSegmentIndex: (segmentId: string) => number;
  activateSidebarTab: (tab: SidebarTab) => void;
  onTimelineZoomIn?: () => void;
  onTimelineZoomOut?: () => void;
  onTimelineZoomReset?: () => void;
  onTimelineFitToView?: () => void;
  getTimelinePosition?: () => number;
  getTotalTimelineDuration?: () => number;
  onSeekTimeline?: (pos: number) => void;
}

const FRAME_STEP = 1 / 30;
const SHORT_STEP = 1;
const LONG_STEP = 5;

export function useEditorShortcuts({
  selectedSegmentId,
  selectedZoomId,
  selectedDrawingId,
  segmentsLength,
  onDeleteSegment,
  onDeleteZoom,
  onDeleteDrawing,
  onDeleteVideo,
  onTogglePlayPause,
  onToggleCutTool,
  onUndo,
  onRedo,
  onEscape,
  onReorderSegment,
  getSegmentIndex,
  activateSidebarTab,
  onTimelineZoomIn,
  onTimelineZoomOut,
  onTimelineZoomReset,
  onTimelineFitToView,
  getTimelinePosition,
  getTotalTimelineDuration,
  onSeekTimeline,
}: UseEditorShortcutsProps): void {
  const selectedZoomIdRef = useRef<string | null>(selectedZoomId);
  const selectedDrawingIdRef = useRef<string | null>(selectedDrawingId);

  useEffect(() => {
    selectedZoomIdRef.current = selectedZoomId;
  }, [selectedZoomId]);

  useEffect(() => {
    selectedDrawingIdRef.current = selectedDrawingId;
  }, [selectedDrawingId]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      const primaryModifier = e.metaKey || e.ctrlKey;

      if (e.key === 'Escape') {
        e.preventDefault();
        onEscape();
        return;
      }

      if (shouldIgnoreGlobalKeyboardShortcuts(e.target)) return;

      if (e.key === ' ' || e.code === 'Space') {
        e.preventDefault();
        onTogglePlayPause();
        return;
      }

      if (e.key === 'Backspace' && primaryModifier) {
        e.preventDefault();
        onDeleteVideo();
        return;
      }

      if (e.key === 'Backspace' || e.key === 'Delete') {
        if (selectedSegmentId && segmentsLength > 1) {
          e.preventDefault();
          onDeleteSegment();
          return;
        }
        const currentZoomId = selectedZoomIdRef.current;
        if (currentZoomId) {
          e.preventDefault();
          onDeleteZoom(currentZoomId);
          return;
        }
        const currentDrawingId = selectedDrawingIdRef.current;
        if (currentDrawingId) {
          e.preventDefault();
          onDeleteDrawing();
          return;
        }
      }

      if (e.key === 's' && primaryModifier) {
        e.preventDefault();
        activateSidebarTab('export');
        return;
      }

      if (e.key === 'c' && !e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        onToggleCutTool();
        return;
      }

      if (e.altKey && selectedSegmentId && segmentsLength > 1) {
        if (e.key === 'ArrowLeft') {
          const idx = getSegmentIndex(selectedSegmentId);
          if (idx > 0) {
            e.preventDefault();
            onReorderSegment(selectedSegmentId, idx - 1);
          }
          return;
        }
        if (e.key === 'ArrowRight') {
          const idx = getSegmentIndex(selectedSegmentId);
          if (idx >= 0 && idx < segmentsLength - 1) {
            e.preventDefault();
            onReorderSegment(selectedSegmentId, idx + 1);
          }
          return;
        }
      }

      if (e.key === 'z' && primaryModifier && !e.shiftKey) {
        e.preventDefault();
        onUndo();
        return;
      }

      if (e.key === 'z' && primaryModifier && e.shiftKey) {
        e.preventDefault();
        onRedo();
        return;
      }

      if (primaryModifier && (e.key === '=' || e.key === '+')) {
        e.preventDefault();
        onTimelineZoomIn?.();
        return;
      }

      if (primaryModifier && e.key === '-') {
        e.preventDefault();
        onTimelineZoomOut?.();
        return;
      }

      if (primaryModifier && e.key === '0') {
        e.preventDefault();
        onTimelineZoomReset?.();
        return;
      }

      if (
        e.key === 'f' &&
        !e.metaKey &&
        !e.ctrlKey &&
        !e.altKey &&
        !e.shiftKey
      ) {
        if (onTimelineFitToView) {
          e.preventDefault();
          onTimelineFitToView();
          return;
        }
      }

      const canSeek =
        getTimelinePosition && getTotalTimelineDuration && onSeekTimeline;

      if (canSeek && e.key === 'Home') {
        e.preventDefault();
        onSeekTimeline(0);
        return;
      }

      if (canSeek && e.key === 'End') {
        e.preventDefault();
        const duration = getTotalTimelineDuration();
        onSeekTimeline(Math.max(0, duration - 0.01));
        return;
      }

      if (
        canSeek &&
        (e.key === 'ArrowLeft' || e.key === 'ArrowRight') &&
        !e.metaKey &&
        !e.ctrlKey &&
        !e.altKey
      ) {
        e.preventDefault();
        const direction = e.key === 'ArrowLeft' ? -1 : 1;
        const step = e.shiftKey ? LONG_STEP : SHORT_STEP;
        const duration = getTotalTimelineDuration();
        const next = getTimelinePosition() + direction * step;
        onSeekTimeline(Math.max(0, Math.min(duration, next)));
        return;
      }

      if (canSeek && (e.key === ',' || e.key === '.')) {
        e.preventDefault();
        const direction = e.key === ',' ? -1 : 1;
        const duration = getTotalTimelineDuration();
        const next = getTimelinePosition() + direction * FRAME_STEP;
        onSeekTimeline(Math.max(0, Math.min(duration, next)));
      }
    },
    [
      selectedSegmentId,
      segmentsLength,
      onDeleteSegment,
      onDeleteZoom,
      onDeleteDrawing,
      onDeleteVideo,
      onEscape,
      onTogglePlayPause,
      onToggleCutTool,
      onReorderSegment,
      getSegmentIndex,
      onUndo,
      onRedo,
      activateSidebarTab,
      onTimelineZoomIn,
      onTimelineZoomOut,
      onTimelineZoomReset,
      onTimelineFitToView,
      getTimelinePosition,
      getTotalTimelineDuration,
      onSeekTimeline,
    ]
  );

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);
}
