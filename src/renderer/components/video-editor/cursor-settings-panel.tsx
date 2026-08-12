import { useCallback, useState } from 'react';
import { Edit3, FileUp, Upload, X } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import { Label } from '@/renderer/components/ui/label';
import { Select } from '@/renderer/components/ui/select';
import { Slider } from '@/renderer/components/ui/slider';
import { Switch } from '@/renderer/components/ui/switch';
import CursorDataEditorDialog from './cursor-data-editor-dialog';
import {
  DataEditorSection,
  ResetButton,
  SettingsPanelHeader,
} from './components';
import { useStyleUpdater } from './hooks/use-style-updater';
import type { CursorData, CursorStyle } from '@/types/cursor';
import { DEFAULT_CURSOR_STYLE } from '@/types/cursor';

interface CursorSettingsPanelProps {
  cursorStyle: CursorStyle;
  onStyleChange: (style: CursorStyle) => void;
  hasCursorData: boolean;
  cursorData: CursorData | null;
  videoDuration: number;
  videoWidth: number;
  videoHeight: number;
  onCursorDataSave: (
    data: CursorData
  ) => Promise<{ success: boolean; error?: string }>;
  onCursorDataImport: () => Promise<{ success: boolean; error?: string }>;
}

interface ColorOption {
  name: string;
  value: string;
}

const CURSOR_COLORS: ColorOption[] = [
  { name: 'White', value: '#ffffff' },
  { name: 'Black', value: '#000000' },
  { name: 'Yellow', value: '#facc15' },
  { name: 'Red', value: '#ef4444' },
  { name: 'Blue', value: '#3b82f6' },
  { name: 'Green', value: '#22c55e' },
  { name: 'Orange', value: '#f97316' },
  { name: 'Purple', value: '#a855f7' },
  { name: 'Pink', value: '#ec4899' },
];

const BORDER_COLORS: ColorOption[] = [
  { name: 'Black', value: '#000000' },
  { name: 'White', value: '#ffffff' },
  { name: 'Gray', value: '#6b7280' },
  { name: 'None', value: 'transparent' },
];

function getSmoothingLabel(value: number): string {
  if (value === 0) return 'Off';
  if (value <= 0.3) return 'Low';
  if (value <= 0.6) return 'Medium';
  return 'High';
}

function ColorSwatch({ color }: { color: string }) {
  return (
    <span
      className="size-3 shrink-0 rounded-full border border-gray-300"
      style={{ backgroundColor: color }}
    />
  );
}

function ColorSelect({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: ColorOption[];
  onChange: (value: string) => void;
}) {
  const selectOptions = options.map(option => ({
    value: option.value,
    label: option.name,
    content: (
      <span className="flex flex-1 items-center gap-2 text-xs">
        <ColorSwatch color={option.value} />
        {option.name}
      </span>
    ),
  }));

  return (
    <div className="space-y-2">
      <Label className="text-sm">{label}</Label>
      <Select
        label={label}
        size="sm"
        className="w-full"
        placeholder="Custom"
        value={value}
        options={selectOptions}
        onChange={onChange}
      />
    </div>
  );
}

export default function CursorSettingsPanel({
  cursorStyle,
  onStyleChange,
  hasCursorData,
  cursorData,
  videoDuration,
  videoWidth,
  videoHeight,
  onCursorDataSave,
  onCursorDataImport,
}: CursorSettingsPanelProps) {
  const [isLoadingImage, setIsLoadingImage] = useState(false);
  const [isEditorOpen, setIsEditorOpen] = useState(false);
  const [isImporting, setIsImporting] = useState(false);

  const updateStyle = useStyleUpdater(cursorStyle, onStyleChange);

  const handleSelectCustomCursor = useCallback(async () => {
    setIsLoadingImage(true);
    try {
      const dataUrl = await window.ipcRenderer.invoke('cursor:selectImage');
      if (dataUrl) {
        onStyleChange({ ...cursorStyle, customCursorImage: dataUrl });
      }
    } catch (error) {
      console.error('Failed to select cursor image:', error);
    } finally {
      setIsLoadingImage(false);
    }
  }, [cursorStyle, onStyleChange]);

  const handleRemoveCustomCursor = useCallback(() => {
    onStyleChange({ ...cursorStyle, customCursorImage: undefined });
  }, [cursorStyle, onStyleChange]);

  const handleImport = useCallback(async () => {
    setIsImporting(true);
    try {
      await onCursorDataImport();
    } finally {
      setIsImporting(false);
    }
  }, [onCursorDataImport]);

  if (!hasCursorData) {
    return (
      <div className="space-y-4 p-4">
        <SettingsPanelHeader
          title="Cursor Data"
          description="No cursor data available for this video"
        />

        <div className="space-y-2">
          <p className="text-muted-foreground text-sm">
            You can add cursor data manually by importing a JSON file or
            creating it in the editor.
          </p>

          <div className="flex flex-col gap-2">
            <Button
              variant="tertiary"
              size="xs"
              onClick={handleImport}
              disabled={isImporting}
              className="w-full gap-2"
            >
              <FileUp className="size-3.5" />
              {isImporting ? 'Importing...' : 'Import from File'}
            </Button>

            <Button
              variant="tertiary"
              size="xs"
              onClick={() => setIsEditorOpen(true)}
              className="w-full gap-2"
            >
              <Edit3 className="size-3.5" />
              Create Manually
            </Button>
          </div>
        </div>

        <div className="border-border bg-muted-background space-y-2 rounded-md border p-3">
          <p className="text-sm font-medium">Cursor Data Format</p>
          <p className="text-muted-foreground text-xs">
            Cursor data is a JSON file containing mouse movement events with
            normalized coordinates (0-1). Each event has a timestamp, x/y
            position, and event type (move, down, up, scroll).
          </p>
        </div>

        <CursorDataEditorDialog
          open={isEditorOpen}
          onOpenChange={setIsEditorOpen}
          initialData={null}
          videoDuration={videoDuration}
          videoWidth={videoWidth}
          videoHeight={videoHeight}
          onSave={onCursorDataSave}
        />
      </div>
    );
  }

  const hasCustomCursor = !!cursorStyle.customCursorImage;

  return (
    <div className="space-y-4 p-4">
      <SettingsPanelHeader
        title="Cursor Overlay"
        description="Show cursor in your video"
        enabled={cursorStyle.enabled}
        onEnabledChange={enabled => updateStyle({ enabled })}
      />

      {!cursorStyle.enabled ? (
        <p className="text-muted-foreground text-sm">
          Cursor overlay is disabled. Enable it to show cursor in your video.
        </p>
      ) : (
        <>
          <div className="space-y-2">
            <Label className="text-sm">Custom Cursor</Label>
            {hasCustomCursor ? (
              <div className="flex items-center gap-2">
                <div className="border-border flex size-10 items-center justify-center overflow-hidden rounded border bg-neutral-100 dark:bg-neutral-800">
                  <img
                    src={cursorStyle.customCursorImage}
                    alt="Custom cursor"
                    className="max-h-full max-w-full object-contain"
                  />
                </div>
                <Button
                  variant="tertiary"
                  size="xs"
                  onClick={handleRemoveCustomCursor}
                  className="gap-1"
                >
                  <X className="size-3" />
                  Remove
                </Button>
              </div>
            ) : (
              <Button
                variant="tertiary"
                size="xs"
                onClick={handleSelectCustomCursor}
                disabled={isLoadingImage}
                className="w-full gap-2"
              >
                <Upload className="size-3.5" />
                {isLoadingImage ? 'Loading...' : 'Upload Custom Cursor'}
              </Button>
            )}
            <p className="text-muted-foreground text-xs">
              {hasCustomCursor
                ? 'Using custom cursor image'
                : 'Upload a PNG, SVG, or other image to use as cursor'}
            </p>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label className="text-sm">Size</Label>
              <span className="text-muted-foreground text-xs">
                {cursorStyle.size}%
              </span>
            </div>
            <Slider
              size="sm"
              value={[cursorStyle.size]}
              onValueChange={([value]) => updateStyle({ size: value })}
              min={50}
              max={250}
              step={5}
            />
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label className="text-sm">Smoothing</Label>
              <span className="text-muted-foreground text-xs">
                {getSmoothingLabel(cursorStyle.smoothing)}
              </span>
            </div>
            <Slider
              size="sm"
              value={[cursorStyle.smoothing]}
              onValueChange={([value]) => updateStyle({ smoothing: value })}
              min={0}
              max={1}
              step={0.1}
            />
            <p className="text-muted-foreground text-xs">
              Reduces cursor shake for smoother movement
            </p>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <div>
                <Label className="text-sm">Motion Blur</Label>
                <p className="text-muted-foreground text-xs">
                  Blur the cursor along its movement
                </p>
              </div>
              <Switch
                size="sm"
                checked={cursorStyle.motionBlur}
                onCheckedChange={checked =>
                  updateStyle({ motionBlur: checked })
                }
              />
            </div>
            {cursorStyle.motionBlur && (
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <Label className="text-sm">Blur Strength</Label>
                  <span className="text-muted-foreground text-xs">
                    {Math.round(cursorStyle.motionBlurStrength * 100)}%
                  </span>
                </div>
                <Slider
                  size="sm"
                  value={[cursorStyle.motionBlurStrength]}
                  onValueChange={([value]) =>
                    updateStyle({ motionBlurStrength: value })
                  }
                  min={0}
                  max={1}
                  step={0.05}
                />
              </div>
            )}
          </div>

          {!hasCustomCursor && (
            <div className="grid grid-cols-2 gap-3">
              <ColorSelect
                label="Color"
                value={cursorStyle.color}
                options={CURSOR_COLORS}
                onChange={value => updateStyle({ color: value })}
              />
              <ColorSelect
                label="Border"
                value={cursorStyle.borderColor}
                options={BORDER_COLORS}
                onChange={value => updateStyle({ borderColor: value })}
              />
            </div>
          )}

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <div>
                <Label className="text-sm">Hide When Idle</Label>
                <p className="text-muted-foreground text-xs">
                  Fade out cursor when not moving
                </p>
              </div>
              <Switch
                size="sm"
                checked={cursorStyle.hideOnIdle}
                onCheckedChange={checked =>
                  updateStyle({ hideOnIdle: checked })
                }
              />
            </div>
            {cursorStyle.hideOnIdle && (
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <Label className="text-sm">Timeout</Label>
                  <span className="text-muted-foreground text-xs">
                    {cursorStyle.hideOnIdleTimeout}s
                  </span>
                </div>
                <Slider
                  size="sm"
                  value={[cursorStyle.hideOnIdleTimeout]}
                  onValueChange={([value]) =>
                    updateStyle({ hideOnIdleTimeout: value })
                  }
                  min={0.5}
                  max={5}
                  step={0.5}
                />
              </div>
            )}
          </div>

          <DataEditorSection
            label="Cursor Data"
            onEdit={() => setIsEditorOpen(true)}
            onImport={handleImport}
            isImporting={isImporting}
          >
            {cursorData
              ? `${cursorData.events.length} events, ${cursorData.meta.duration.toFixed(1)}s duration`
              : 'Edit or replace cursor movement data'}
          </DataEditorSection>

          <ResetButton onClick={() => onStyleChange(DEFAULT_CURSOR_STYLE)} />

          <CursorDataEditorDialog
            open={isEditorOpen}
            onOpenChange={setIsEditorOpen}
            initialData={cursorData}
            videoDuration={videoDuration}
            videoWidth={videoWidth}
            videoHeight={videoHeight}
            onSave={onCursorDataSave}
          />
        </>
      )}
    </div>
  );
}
