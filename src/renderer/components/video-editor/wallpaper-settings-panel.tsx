import { Label } from '@/renderer/components/ui/label';
import { Separator } from '@/renderer/components/ui/separator';
import { Switch } from '@/renderer/components/ui/switch';
import {
  AspectRatioSelector,
  BackgroundSelector,
  WallpaperSliders,
} from '@/renderer/components/shared';
import { SettingsPanelHeader } from './components';
import type { AspectRatio } from '@/types/aspect-ratio';
import type { VideoWallpaperSettings } from '@/types/video-wallpaper';
import type { GradientOption } from '@/types/editor';
import type { RecordingType } from '@/types/video';

interface WallpaperSettingsPanelProps {
  wallpaper: VideoWallpaperSettings;
  onEnabledChange: (enabled: boolean) => void;
  onGradientChange: (gradient: GradientOption | null) => void;
  onBackgroundImageChange: (image: string | null) => void;
  onPaddingChange: (padding: number) => void;
  onCornersChange: (corners: number) => void;
  onShadowChange: (shadow: number) => void;
  onAspectRatioChange: (aspectRatio: AspectRatio | null) => void;
  onDeviceFrameChange: (deviceFrame: boolean) => void;
  recordingType?: RecordingType;
}

export default function WallpaperSettingsPanel({
  wallpaper,
  onEnabledChange,
  onGradientChange,
  onBackgroundImageChange,
  onPaddingChange,
  onCornersChange,
  onShadowChange,
  onAspectRatioChange,
  onDeviceFrameChange,
  recordingType,
}: WallpaperSettingsPanelProps) {
  const isIOSDevice = recordingType === 'ios-device';
  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 space-y-4 overflow-y-auto p-4">
        <SettingsPanelHeader
          title="Wallpaper"
          description="Add background and styling to your video"
        />

        <BackgroundSelector
          selectedGradient={wallpaper.gradient}
          selectedBackgroundImage={wallpaper.backgroundImage}
          onGradientChange={onGradientChange}
          onBackgroundImageChange={onBackgroundImageChange}
          showDesktopWallpaper={true}
          showCustomBackgrounds={true}
          noWallpaper={{
            selected: !wallpaper.enabled,
            onSelect: () => onEnabledChange(false),
          }}
        />

        {isIOSDevice && wallpaper.enabled && (
          <>
            <div className="flex items-center justify-between">
              <Label className="text-sm">Device Frame</Label>
              <Switch
                size="sm"
                checked={wallpaper.deviceFrame}
                onCheckedChange={onDeviceFrameChange}
              />
            </div>

            <Separator />
          </>
        )}

        {wallpaper.enabled && (
          <>
            <Separator />

            <AspectRatioSelector
              value={wallpaper.aspectRatio}
              onChange={onAspectRatioChange}
            />

            <Separator />

            <WallpaperSliders
              padding={{
                label: 'Padding',
                value: wallpaper.padding,
                min: 0,
                max: 300,
                onChange: onPaddingChange,
              }}
              corners={
                wallpaper.deviceFrame
                  ? undefined
                  : {
                      label: 'Corners',
                      value: wallpaper.corners,
                      min: 0,
                      max: 100,
                      onChange: onCornersChange,
                    }
              }
              shadow={{
                label: 'Shadow',
                value: wallpaper.shadow,
                min: 0,
                max: 300,
                onChange: onShadowChange,
              }}
            />
          </>
        )}
      </div>
    </div>
  );
}
