import { Label } from '@/renderer/components/ui/label';
import { Switch } from '@/renderer/components/ui/switch';
import { WallpaperSliders } from '@/renderer/components/shared';
import {
  EmptyState,
  ResetButton,
  SettingsPanelHeader,
  TabSelector,
} from './components';
import { useStyleUpdater } from './hooks/use-style-updater';
import type {
  CameraStyle,
  CameraPosition,
  CameraOverlayShape,
  CameraOverlaySize,
} from '@/types/camera';
import { DEFAULT_CAMERA_STYLE } from '@/types/camera';
import { cn } from '@/renderer/lib/utils';

interface CameraSettingsPanelProps {
  cameraStyle: CameraStyle;
  onStyleChange: (style: CameraStyle) => void;
  hasCameraData: boolean;
}

const POSITION_GRID: CameraPosition[][] = [
  ['top-left', 'top-center', 'top-right'],
  ['middle-left', 'middle-center', 'middle-right'],
  ['bottom-left', 'bottom-center', 'bottom-right'],
];

const SIZE_OPTIONS: { value: CameraOverlaySize; label: string }[] = [
  { value: 'small', label: 'Small' },
  { value: 'medium', label: 'Medium' },
  { value: 'large', label: 'Large' },
];

const SHAPE_OPTIONS: { value: CameraOverlayShape; label: string }[] = [
  { value: 'rectangle', label: 'Rectangle' },
  { value: 'square', label: 'Square' },
  { value: 'vertical', label: 'Vertical' },
];

function PositionGrid({
  value,
  onChange,
}: {
  value: CameraPosition;
  onChange: (position: CameraPosition) => void;
}) {
  return (
    <div className="flex flex-col gap-1">
      {POSITION_GRID.map(row => (
        <div key={row.join('-')} className="flex gap-1">
          {row.map(position => (
            <button
              key={position}
              onClick={() => onChange(position)}
              className={cn(
                'h-6 flex-1 rounded-lg transition-colors',
                value === position
                  ? 'bg-primary'
                  : 'bg-default hover:bg-default-hover'
              )}
              title={position.replace('-', ' ')}
            />
          ))}
        </div>
      ))}
    </div>
  );
}

export default function CameraSettingsPanel({
  cameraStyle,
  onStyleChange,
  hasCameraData,
}: CameraSettingsPanelProps) {
  const updateStyle = useStyleUpdater(cameraStyle, onStyleChange);

  if (!hasCameraData) {
    return (
      <EmptyState
        message={
          <>
            No camera recording available for this video.
            <br />
            Enable camera during recording to use camera overlay.
          </>
        }
      />
    );
  }

  return (
    <div className="space-y-4 p-4">
      <SettingsPanelHeader
        title="Camera Overlay"
        description="Customize camera appearance in your video"
      />

      <div className="flex items-center justify-between">
        <Label className="text-sm">Show Camera</Label>
        <Switch
          size="sm"
          checked={cameraStyle.visible}
          onCheckedChange={checked => updateStyle({ visible: checked })}
        />
      </div>

      {cameraStyle.visible && (
        <>
          <div className="flex items-center justify-between">
            <Label className="text-sm">Mirror</Label>
            <Switch
              size="sm"
              checked={cameraStyle.mirrored}
              onCheckedChange={checked => updateStyle({ mirrored: checked })}
            />
          </div>

          <div className="space-y-2">
            <Label className="text-sm">Position</Label>
            <PositionGrid
              value={cameraStyle.position}
              onChange={position => updateStyle({ position })}
            />
          </div>

          <TabSelector
            label="Size"
            value={cameraStyle.size}
            options={SIZE_OPTIONS}
            onChange={value =>
              updateStyle({ size: value as CameraOverlaySize })
            }
          />

          <TabSelector
            label="Shape"
            value={cameraStyle.shape}
            options={SHAPE_OPTIONS}
            onChange={value =>
              updateStyle({ shape: value as CameraOverlayShape })
            }
          />

          <WallpaperSliders
            padding={{
              label: 'Edge Padding',
              value: cameraStyle.padding,
              min: 0,
              max: 10,
              onChange: value => updateStyle({ padding: value }),
            }}
            corners={{
              label: 'Radius',
              value: cameraStyle.borderRadius,
              min: 0,
              max: 100,
              onChange: value => updateStyle({ borderRadius: value }),
            }}
            shadow={{
              label: 'Shadow',
              value: cameraStyle.shadow,
              min: 0,
              max: 100,
              onChange: value => updateStyle({ shadow: value }),
            }}
          />

          <ResetButton onClick={() => onStyleChange(DEFAULT_CAMERA_STYLE)} />
        </>
      )}
    </div>
  );
}
