import { Keyboard, Play, Plus, Square, Trash2 } from 'lucide-react';
import { Label } from '@/renderer/components/ui/label';
import { Slider } from '@/renderer/components/ui/slider';
import { Switch } from '@/renderer/components/ui/switch';
import { Button } from '@/renderer/components/ui/button';
import { Select } from '@/renderer/components/ui/select';
import { SettingsPanelHeader } from './components';
import { useStyleUpdater } from './hooks/use-style-updater';
import type { AudioStyle, KeyboardSoundType } from '@/types/audio';
import { KEYBOARD_SOUND_OPTIONS } from '@/types/audio';
import type { MusicTrack } from '@/types/music';
import { SOURCE_ICONS } from '@/types/music';
import {
  PLAYBACK_SPEED_PRESETS,
  formatPlaybackSpeed,
} from '@/types/playback-speed';

const PLAYBACK_SPEED_OPTIONS = PLAYBACK_SPEED_PRESETS.map(speed => ({
  value: speed.toString(),
  label: formatPlaybackSpeed(speed),
}));

interface AudioSettingsPanelProps {
  audioStyle: AudioStyle;
  onStyleChange: (style: AudioStyle) => void;
  hasKeyboardData: boolean;
  onPlayDemo: () => void;
  onStopDemo: () => void;
  isDemoPlaying: boolean;
  musicTrackGroups: MusicTrack[][];
  onAddMusicTrack: () => void;
  onRemoveMusicTrackGroup: (groupId: string) => void;
  onUpdateMusicTrackGroup: (
    groupId: string,
    updates: Partial<MusicTrack>
  ) => void;
}

export default function AudioSettingsPanel({
  audioStyle,
  onStyleChange,
  hasKeyboardData,
  onPlayDemo,
  onStopDemo,
  isDemoPlaying,
  musicTrackGroups,
  onAddMusicTrack,
  onRemoveMusicTrackGroup,
  onUpdateMusicTrackGroup,
}: AudioSettingsPanelProps) {
  const updateStyle = useStyleUpdater(audioStyle, onStyleChange);

  return (
    <div className="space-y-4 p-4">
      <SettingsPanelHeader
        title="Audio Tracks"
        description="Manage audio tracks in your project"
        action={
          <Button variant="ghost" size="icon-xs" onClick={onAddMusicTrack}>
            <Plus className="size-4" />
          </Button>
        }
      />

      {musicTrackGroups.length === 0 && (
        <p className="text-xs text-muted-foreground">No audio tracks.</p>
      )}

      {musicTrackGroups.map(group => {
        const track = group[0];
        const Icon = SOURCE_ICONS[track.source];
        const isRemovable = track.source === 'music';

        return (
          <div key={track.groupId} className="space-y-2 rounded-md border p-3">
            <div className="flex items-center justify-between gap-2">
              <div className="flex min-w-0 items-center gap-2">
                <Icon className="size-4 shrink-0 text-muted-foreground" />
                <span className="truncate text-sm font-medium">
                  {track.name}
                </span>
              </div>
              <div className="flex items-center gap-1">
                <Switch
                  size="sm"
                  checked={track.enabled}
                  onCheckedChange={checked =>
                    onUpdateMusicTrackGroup(track.groupId, {
                      enabled: checked,
                    })
                  }
                />
                {isRemovable && (
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    onClick={() => onRemoveMusicTrackGroup(track.groupId)}
                  >
                    <Trash2 className="size-3.5" />
                  </Button>
                )}
              </div>
            </div>

            {track.enabled && (
              <>
                <div className="flex items-center gap-3">
                  <Label className="w-12 shrink-0 text-xs text-muted-foreground">
                    Volume
                  </Label>
                  <Slider
                    size="sm"
                    value={[track.volume * 100]}
                    onValueChange={([value]) =>
                      onUpdateMusicTrackGroup(track.groupId, {
                        volume: value / 100,
                      })
                    }
                    min={0}
                    max={100}
                    step={1}
                    className="flex-1"
                  />
                  <span className="w-8 text-right text-xs text-muted-foreground">
                    {Math.round(track.volume * 100)}%
                  </span>
                </div>

                <div className="flex items-center gap-3">
                  <Label className="w-12 shrink-0 text-xs text-muted-foreground">
                    Speed
                  </Label>
                  <Select
                    label="Speed"
                    size="sm"
                    className="flex-1"
                    value={track.speed.toString()}
                    options={PLAYBACK_SPEED_OPTIONS}
                    onChange={value =>
                      onUpdateMusicTrackGroup(track.groupId, {
                        speed: parseFloat(value),
                      })
                    }
                  />
                </div>
              </>
            )}
          </div>
        );
      })}

      {hasKeyboardData && (
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Keyboard className="size-4 text-muted-foreground" />
              <Label className="text-sm">Keyboard Sound</Label>
            </div>
            <Switch
              size="sm"
              checked={audioStyle.keyboardSoundEnabled}
              onCheckedChange={checked =>
                updateStyle({ keyboardSoundEnabled: checked })
              }
            />
          </div>
          {audioStyle.keyboardSoundEnabled && (
            <div className="space-y-3">
              <div className="flex items-center gap-2">
                <Select
                  label="Keyboard sound"
                  size="sm"
                  className="flex-1"
                  value={audioStyle.keyboardSoundType}
                  options={KEYBOARD_SOUND_OPTIONS}
                  onChange={value =>
                    updateStyle({
                      keyboardSoundType: value as KeyboardSoundType,
                    })
                  }
                />
                <Button
                  variant="tertiary"
                  size="icon-xs"
                  onClick={isDemoPlaying ? onStopDemo : onPlayDemo}
                >
                  {isDemoPlaying ? (
                    <Square className="size-3.5" />
                  ) : (
                    <Play className="size-3.5" />
                  )}
                </Button>
              </div>
              <div className="flex items-center gap-3">
                <Slider
                  size="sm"
                  value={[audioStyle.keyboardSoundVolume * 100]}
                  onValueChange={([value]) =>
                    updateStyle({ keyboardSoundVolume: value / 100 })
                  }
                  min={0}
                  max={100}
                  step={1}
                  className="flex-1"
                />
                <span className="w-8 text-right text-xs text-muted-foreground">
                  {Math.round(audioStyle.keyboardSoundVolume * 100)}%
                </span>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
