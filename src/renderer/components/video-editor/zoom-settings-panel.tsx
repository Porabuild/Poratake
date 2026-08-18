import { useCallback, useMemo } from 'react';
import { MousePointerClick } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import { Label } from '@/renderer/components/ui/label';
import { Slider } from '@/renderer/components/ui/slider';
import {
  Tabs,
  TabsList,
  TabsTrigger,
  TabsContent,
} from '@/renderer/components/ui/tabs';
import { EmptyState, ResetButton, SettingsPanelHeader } from './components';
import ManualZoomPreview from './manual-zoom-preview';
import {
  DEFAULT_ZOOM_LEVEL,
  MIN_ZOOM_LEVEL,
  MAX_ZOOM_LEVEL,
  ZOOM_LEVEL_STEP,
  type ZoomSegment,
  type ZoomSettings,
  type ZoomTargetMode,
  type ZoomFocusPoint,
} from '@/types/zoom';

interface ZoomSettingsPanelProps {
  selectedZoomId: string | null;
  zoomSegments: ZoomSegment[];
  zoomSettings: ZoomSettings;
  onUpdateZoomSegment: (id: string, updates: Partial<ZoomSegment>) => void;
  onUpdateZoomSettings: (settings: ZoomSettings) => void;
  videoSrc: string;
  timelinePosition: number;
  hasCursorData: boolean;
  onGenerateAutoZoom: () => void;
}

interface AutoZoomSectionProps {
  hasCursorData: boolean;
  onGenerateAutoZoom: () => void;
}

function AutoZoomSection({
  hasCursorData,
  onGenerateAutoZoom,
}: AutoZoomSectionProps) {
  return (
    <div className="space-y-3 border-b border-border p-4">
      <SettingsPanelHeader
        title="Auto Zoom"
        description="Highlight clicks, drags and scrolls with zoom"
      />
      <Button
        variant="tertiary"
        size="xs"
        onClick={onGenerateAutoZoom}
        disabled={!hasCursorData}
        className="w-full gap-2"
      >
        <MousePointerClick className="size-4" />
        Generate from Interactions
      </Button>
      {!hasCursorData && (
        <p className="text-xs text-muted-foreground">
          No cursor data recorded for this video
        </p>
      )}
    </div>
  );
}

function formatZoomLevel(value: number): string {
  return `${Math.round(value * 100)}%`;
}

function formatZoomSpeed(value: number): string {
  if (value <= 0.3) return 'Very Fast';
  if (value <= 0.6) return 'Fast';
  if (value <= 1.0) return 'Medium';
  if (value <= 1.5) return 'Slow';
  return 'Very Slow';
}

export default function ZoomSettingsPanel({
  selectedZoomId,
  zoomSegments,
  zoomSettings,
  onUpdateZoomSegment,
  onUpdateZoomSettings,
  videoSrc,
  timelinePosition,
  hasCursorData,
  onGenerateAutoZoom,
}: ZoomSettingsPanelProps) {
  const selectedZoom = useMemo(
    () => zoomSegments.find(seg => seg.id === selectedZoomId),
    [zoomSegments, selectedZoomId]
  );

  const effectiveSpeed =
    selectedZoom?.transitionInDuration ??
    selectedZoom?.transitionOutDuration ??
    zoomSettings.transitionInDuration;

  const targetMode: ZoomTargetMode = selectedZoom?.targetMode ?? 'cursor';
  const focusPoint: ZoomFocusPoint = selectedZoom?.focusPoint ?? {
    x: 0.5,
    y: 0.5,
  };

  const handleZoomLevelChange = useCallback(
    (value: number) => {
      if (selectedZoomId) {
        onUpdateZoomSegment(selectedZoomId, { zoomLevel: value });
      }
    },
    [selectedZoomId, onUpdateZoomSegment]
  );

  const handleResetToDefault = useCallback(() => {
    if (selectedZoomId) {
      onUpdateZoomSegment(selectedZoomId, { zoomLevel: DEFAULT_ZOOM_LEVEL });
    }
  }, [selectedZoomId, onUpdateZoomSegment]);

  const handleZoomSpeedChange = useCallback(
    (value: number) => {
      if (selectedZoomId) {
        onUpdateZoomSegment(selectedZoomId, {
          transitionInDuration: value,
          transitionOutDuration: value,
        });
      }
    },
    [selectedZoomId, onUpdateZoomSegment]
  );

  const handleResetZoomSpeed = useCallback(() => {
    if (selectedZoomId) {
      onUpdateZoomSegment(selectedZoomId, {
        transitionInDuration: undefined,
        transitionOutDuration: undefined,
      });
    }
  }, [selectedZoomId, onUpdateZoomSegment]);

  const handleZoomSettingsChange = useCallback(
    (updates: Partial<ZoomSettings>) => {
      onUpdateZoomSettings({ ...zoomSettings, ...updates });
    },
    [onUpdateZoomSettings, zoomSettings]
  );

  const handleTargetModeChange = useCallback(
    (mode: string) => {
      if (selectedZoomId) {
        const newMode = mode as ZoomTargetMode;
        const updates: Partial<ZoomSegment> = { targetMode: newMode };

        if (newMode === 'manual' && !selectedZoom?.focusPoint) {
          updates.focusPoint = { x: 0.5, y: 0.5 };
        }

        onUpdateZoomSegment(selectedZoomId, updates);
      }
    },
    [selectedZoomId, selectedZoom?.focusPoint, onUpdateZoomSegment]
  );

  const handleFocusPointChange = useCallback(
    (point: ZoomFocusPoint) => {
      if (selectedZoomId) {
        onUpdateZoomSegment(selectedZoomId, { focusPoint: point });
      }
    },
    [selectedZoomId, onUpdateZoomSegment]
  );

  if (!selectedZoom) {
    return (
      <div className="flex h-full flex-col">
        <AutoZoomSection
          hasCursorData={hasCursorData}
          onGenerateAutoZoom={onGenerateAutoZoom}
        />
        <EmptyState
          className="h-auto flex-1"
          message="Select a zoom segment on the timeline to edit its settings"
        />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <AutoZoomSection
        hasCursorData={hasCursorData}
        onGenerateAutoZoom={onGenerateAutoZoom}
      />
      <div className="space-y-4 p-4">
        <SettingsPanelHeader
          title="Zoom Settings"
          description="Configure settings for the selected zoom segment"
        />

        <div className="space-y-2">
          <Label className="text-sm">Zoom Target</Label>
          <Tabs value={targetMode} onValueChange={handleTargetModeChange}>
            <TabsList className="w-full">
              <TabsTrigger value="cursor" className="flex-1">
                Cursor
              </TabsTrigger>
              <TabsTrigger value="manual" className="flex-1">
                Manual
              </TabsTrigger>
            </TabsList>
            <TabsContent value="cursor">
              <p className="text-xs text-muted-foreground">
                Zoom follows cursor position and movement
              </p>
            </TabsContent>
            <TabsContent value="manual">
              <div className="space-y-3">
                <ManualZoomPreview
                  videoSrc={videoSrc}
                  timelinePosition={timelinePosition}
                  focusPoint={focusPoint}
                  onFocusPointChange={handleFocusPointChange}
                />
              </div>
            </TabsContent>
          </Tabs>
        </div>

        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <Label className="text-sm">Zoom Level</Label>
            <span className="text-xs text-muted-foreground">
              {formatZoomLevel(selectedZoom.zoomLevel)}
            </span>
          </div>
          <Slider
            size="sm"
            value={[selectedZoom.zoomLevel]}
            onValueChange={([value]) => handleZoomLevelChange(value)}
            min={MIN_ZOOM_LEVEL}
            max={MAX_ZOOM_LEVEL}
            step={ZOOM_LEVEL_STEP}
          />
          <p className="text-xs text-muted-foreground">
            Magnification level (100% = no zoom, {MAX_ZOOM_LEVEL * 100}% ={' '}
            {MAX_ZOOM_LEVEL}x)
          </p>
        </div>

        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <Label className="text-sm">Zoom Speed</Label>
            <span className="text-xs text-muted-foreground">
              {formatZoomSpeed(effectiveSpeed)}
            </span>
          </div>
          <Slider
            size="sm"
            value={[effectiveSpeed]}
            onValueChange={([value]) => handleZoomSpeedChange(value)}
            min={0.2}
            max={2.0}
            step={0.1}
          />
          <p className="text-xs text-muted-foreground">
            Duration of zoom in/out transitions
          </p>
        </div>

        <div className="space-y-3">
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label className="text-sm">Smooth Follow</Label>
              <span className="text-xs text-muted-foreground">
                {formatZoomSpeed(zoomSettings.followSmoothness)}
              </span>
            </div>
            <Slider
              size="sm"
              value={[zoomSettings.followSmoothness]}
              onValueChange={([value]) =>
                handleZoomSettingsChange({ followSmoothness: value })
              }
              min={0.08}
              max={0.8}
              step={0.02}
            />
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label className="text-sm">Look Ahead</Label>
              <span className="text-xs text-muted-foreground">
                {Math.round(zoomSettings.lookAhead * 1000)}ms
              </span>
            </div>
            <Slider
              size="sm"
              value={[zoomSettings.lookAhead]}
              onValueChange={([value]) =>
                handleZoomSettingsChange({ lookAhead: value })
              }
              min={0}
              max={0.3}
              step={0.02}
            />
          </div>
        </div>

        <div className="space-y-1">
          <ResetButton
            label="Reset zoom level"
            onClick={handleResetToDefault}
          />
          <ResetButton
            label="Reset zoom speed"
            onClick={handleResetZoomSpeed}
          />
        </div>
      </div>
    </div>
  );
}
