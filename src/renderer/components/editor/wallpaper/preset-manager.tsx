import { useState, useCallback } from 'react';
import { Save, Star, Trash2, X } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import SettingsSelect from '@/renderer/components/settings/settings-select';
import type { WallpaperPreset } from '@/types/settings';
import type { WallpaperSettings } from '@/types/editor';

interface PresetManagerProps {
  presets: WallpaperPreset[];
  currentSettings: WallpaperSettings;
  defaultPresetId: string | null;
  onLoadPreset: (preset: WallpaperPreset) => void;
  onSavePreset: (preset: WallpaperPreset) => void;
  onDeletePreset: (id: string) => void;
  onSetDefaultPreset: (id: string | null) => void;
}

export default function PresetManager({
  presets,
  currentSettings,
  defaultPresetId,
  onLoadPreset,
  onSavePreset,
  onDeletePreset,
  onSetDefaultPreset,
}: PresetManagerProps) {
  const [selectedPresetId, setSelectedPresetId] = useState<string>('');
  const [showSaveDialog, setShowSaveDialog] = useState(false);
  const [presetName, setPresetName] = useState('');

  const handlePresetSelect = useCallback(
    (presetId: string) => {
      setSelectedPresetId(presetId);
      const preset = presets.find(p => p.id === presetId);
      if (preset) {
        onLoadPreset(preset);
      }
    },
    [presets, onLoadPreset]
  );

  const handleSavePreset = useCallback(() => {
    if (!presetName.trim()) return;

    const preset: WallpaperPreset = {
      id: crypto.randomUUID(),
      name: presetName.trim(),
      ...currentSettings,
    };

    onSavePreset(preset);
    setPresetName('');
    setShowSaveDialog(false);
    setSelectedPresetId(preset.id);
  }, [presetName, currentSettings, onSavePreset]);

  const handleDeletePreset = useCallback(
    (id: string) => {
      onDeletePreset(id);
      if (selectedPresetId === id) {
        setSelectedPresetId('');
      }
    },
    [onDeletePreset, selectedPresetId]
  );

  const handleToggleDefault = useCallback(
    (id: string) => {
      onSetDefaultPreset(defaultPresetId === id ? null : id);
    },
    [defaultPresetId, onSetDefaultPreset]
  );

  const defaultPreset = presets.find(p => p.id === defaultPresetId) ?? null;
  const isSelectedDefault =
    Boolean(selectedPresetId) && selectedPresetId === defaultPresetId;

  if (showSaveDialog) {
    return (
      <div className="flex flex-col gap-3">
        <div className="flex items-center justify-between">
          <span className="text-xs font-medium">Save Preset</span>
          <button
            onClick={() => setShowSaveDialog(false)}
            className="text-muted-foreground hover:text-foreground"
          >
            <X className="size-4" />
          </button>
        </div>

        <div className="rounded-md bg-muted/50 p-2 text-xs">
          <div className="mb-1 text-muted-foreground">Preview:</div>
          <div className="flex items-center gap-2">
            {currentSettings.gradient && (
              <div
                className="size-6 rounded"
                style={{
                  background: `linear-gradient(${currentSettings.gradient.angle}deg, ${currentSettings.gradient.colors.join(', ')})`,
                }}
              />
            )}
            <span>
              Padding: {currentSettings.padding}, Corners:{' '}
              {currentSettings.corners}, Shadow: {currentSettings.shadow}
            </span>
          </div>
        </div>

        <input
          type="text"
          value={presetName}
          onChange={e => setPresetName(e.target.value)}
          placeholder="Preset name"
          className="h-7 rounded-field border-0 bg-field px-2 text-xs text-field-foreground placeholder:text-field-placeholder focus:ring-2 focus:ring-ring focus:outline-none"
          autoFocus
          onKeyDown={e => {
            if (e.key === 'Enter' && presetName.trim()) {
              handleSavePreset();
            }
          }}
        />

        <div className="flex gap-2">
          <Button
            variant="ghost"
            size="xs"
            onClick={() => setShowSaveDialog(false)}
            className="flex-1"
          >
            Cancel
          </Button>
          <Button
            size="xs"
            onClick={handleSavePreset}
            disabled={!presetName.trim()}
            className="flex-1"
          >
            Save
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <span className="text-xs font-medium text-muted-foreground">
          Presets
        </span>
        <button
          onClick={() => setShowSaveDialog(true)}
          className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
          title="Save current settings as preset"
        >
          <Save className="size-3" />
          Save
        </button>
      </div>

      {presets.length > 0 ? (
        <>
          <div className="flex items-center gap-2">
            <SettingsSelect
              label="Preset"
              options={presets.map(preset => ({
                value: preset.id,
                label:
                  preset.id === defaultPresetId
                    ? `${preset.name} (default)`
                    : preset.name,
              }))}
              value={selectedPresetId}
              onChange={handlePresetSelect}
              size="sm"
              className="w-auto flex-1"
            />

            {selectedPresetId && (
              <>
                <button
                  onClick={() => handleToggleDefault(selectedPresetId)}
                  className={
                    isSelectedDefault
                      ? 'text-primary'
                      : 'text-muted-foreground hover:text-foreground'
                  }
                  title={
                    isSelectedDefault
                      ? 'Stop using this preset for Polish'
                      : 'Use this preset for Polish'
                  }
                >
                  <Star
                    className="size-4"
                    fill={isSelectedDefault ? 'currentColor' : 'none'}
                  />
                </button>
                <button
                  onClick={() => handleDeletePreset(selectedPresetId)}
                  className="text-muted-foreground hover:text-destructive"
                  title="Delete preset"
                >
                  <Trash2 className="size-4" />
                </button>
              </>
            )}
          </div>

          <p className="text-xs text-muted-foreground">
            {defaultPreset
              ? `Polish on the capture preview copies with "${defaultPreset.name}".`
              : 'Star a preset to enable Polish on the capture preview.'}
          </p>
        </>
      ) : (
        <p className="text-xs text-muted-foreground">
          No presets saved yet. Use the Save button to create one.
        </p>
      )}
    </div>
  );
}
