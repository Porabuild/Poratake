import { useState, useCallback } from 'react';
import { Save, Trash2, X } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/renderer/components/ui/select';
import type { WallpaperPreset } from '@/types/settings';
import type { WallpaperSettings } from '@/types/editor';

interface PresetManagerProps {
  presets: WallpaperPreset[];
  currentSettings: WallpaperSettings;
  onLoadPreset: (preset: WallpaperPreset) => void;
  onSavePreset: (preset: WallpaperPreset) => void;
  onDeletePreset: (id: string) => void;
}

export default function PresetManager({
  presets,
  currentSettings,
  onLoadPreset,
  onSavePreset,
  onDeletePreset,
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

        <div className="bg-muted/50 rounded-md p-2 text-xs">
          <div className="text-muted-foreground mb-1">Preview:</div>
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
          className="border-input bg-background focus:ring-ring h-8 rounded-md border px-2 text-sm focus:ring-2 focus:outline-none"
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
            size="sm"
            onClick={() => setShowSaveDialog(false)}
            className="flex-1"
          >
            Cancel
          </Button>
          <Button
            size="sm"
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
        <span className="text-muted-foreground text-xs font-medium">
          Presets
        </span>
        <button
          onClick={() => setShowSaveDialog(true)}
          className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs"
          title="Save current settings as preset"
        >
          <Save className="size-3" />
          Save
        </button>
      </div>

      {presets.length > 0 ? (
        <div className="flex items-center gap-2">
          <Select value={selectedPresetId} onValueChange={handlePresetSelect}>
            <SelectTrigger size="sm" className="h-8 flex-1">
              <SelectValue placeholder="Select a preset" />
            </SelectTrigger>
            <SelectContent>
              {presets.map(preset => (
                <SelectItem key={preset.id} value={preset.id}>
                  <div className="flex items-center gap-2">
                    {preset.gradient && (
                      <div
                        className="size-4 rounded"
                        style={{
                          background: `linear-gradient(${preset.gradient.angle}deg, ${preset.gradient.colors.join(', ')})`,
                        }}
                      />
                    )}
                    <span>{preset.name}</span>
                  </div>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          {selectedPresetId && (
            <button
              onClick={() => handleDeletePreset(selectedPresetId)}
              className="text-muted-foreground hover:text-destructive"
              title="Delete preset"
            >
              <Trash2 className="size-4" />
            </button>
          )}
        </div>
      ) : (
        <p className="text-muted-foreground text-xs">
          No presets saved yet. Use the Save button to create one.
        </p>
      )}
    </div>
  );
}
