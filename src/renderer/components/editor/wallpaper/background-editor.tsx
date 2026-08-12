import { useState, useCallback } from 'react';
import { Plus, Trash2, X, Upload, ImageIcon } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import { Slider } from '@/renderer/components/ui/slider';
import { cn } from '@/renderer/lib/utils';
import type { CustomBackground, BackgroundType } from '@/types/settings';

interface BackgroundEditorProps {
  onSave: (background: CustomBackground) => void;
  onCancel: () => void;
  initialBackground?: CustomBackground;
}

const DEFAULT_COLORS = ['#f97316', '#ec4899'];

const COLOR_PALETTE = [
  '#ef4444',
  '#f97316',
  '#f59e0b',
  '#84cc16',
  '#22c55e',
  '#14b8a6',
  '#06b6d4',
  '#0ea5e9',
  '#3b82f6',
  '#6366f1',
  '#8b5cf6',
  '#a855f7',
  '#d946ef',
  '#ec4899',
  '#f43f5e',
  '#1e293b',
];

export default function BackgroundEditor({
  onSave,
  onCancel,
  initialBackground,
}: BackgroundEditorProps) {
  const [backgroundType, setBackgroundType] = useState<BackgroundType>(
    initialBackground?.type ?? 'gradient'
  );

  const [colors, setColors] = useState<string[]>(
    initialBackground?.type === 'gradient'
      ? initialBackground.data.colors
      : DEFAULT_COLORS
  );
  const [angle, setAngle] = useState(
    initialBackground?.type === 'gradient' ? initialBackground.data.angle : 135
  );
  const [activeColorIndex, setActiveColorIndex] = useState<number | null>(null);

  const [imageUrl, setImageUrl] = useState<string | null>(
    initialBackground?.type === 'image' ? initialBackground.data.imageUrl : null
  );
  const [isLoadingImage, setIsLoadingImage] = useState(false);

  const handleAddColor = useCallback(() => {
    if (colors.length < 5) {
      const newColor =
        COLOR_PALETTE.find(c => !colors.includes(c)) ?? '#3b82f6';
      setColors(prev => [...prev, newColor]);
    }
  }, [colors]);

  const handleRemoveColor = useCallback(
    (index: number) => {
      if (colors.length > 2) {
        setColors(prev => prev.filter((_, i) => i !== index));
        setActiveColorIndex(null);
      }
    },
    [colors.length]
  );

  const handleColorChange = useCallback(
    (index: number, color: string) => {
      const newColors = [...colors];
      newColors[index] = color;
      setColors(newColors);
    },
    [colors]
  );

  const handleSelectImage = useCallback(async () => {
    setIsLoadingImage(true);
    try {
      const dataUrl = await window.ipcRenderer.invoke('wallpaper:selectImage');
      if (dataUrl) {
        setImageUrl(dataUrl);
      }
    } catch (error) {
      console.error('Failed to select image:', error);
    } finally {
      setIsLoadingImage(false);
    }
  }, []);

  const handleSave = useCallback(() => {
    if (backgroundType === 'gradient') {
      const background: CustomBackground = {
        id: initialBackground?.id ?? crypto.randomUUID(),
        type: 'gradient',
        data: {
          colors,
          angle,
        },
      };
      onSave(background);
    } else if (backgroundType === 'image' && imageUrl) {
      const background: CustomBackground = {
        id: initialBackground?.id ?? crypto.randomUUID(),
        type: 'image',
        data: {
          imageUrl,
        },
      };
      onSave(background);
    }
  }, [backgroundType, colors, angle, imageUrl, initialBackground?.id, onSave]);

  const gradientStyle = {
    background: `linear-gradient(${angle}deg, ${colors.join(', ')})`,
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <span className="text-sm font-medium">
          {initialBackground ? 'Edit Background' : 'New Background'}
        </span>
        <button
          onClick={onCancel}
          className="text-muted-foreground hover:text-foreground"
        >
          <X className="size-4" />
        </button>
      </div>

      {}
      {backgroundType === 'gradient' && (
        <div className="h-16 w-full rounded-lg border" style={gradientStyle} />
      )}
      {backgroundType === 'image' && imageUrl && (
        <div
          className="h-16 w-full rounded-lg border"
          style={{
            backgroundImage: `url(${imageUrl})`,
            backgroundSize: 'cover',
            backgroundPosition: 'center',
          }}
        />
      )}

      {}
      <div className="flex flex-col gap-1.5">
        <label className="text-xs font-medium">Type</label>
        <div className="flex gap-2">
          <button
            onClick={() => setBackgroundType('gradient')}
            className={cn(
              'flex-1 rounded-md border px-3 py-1.5 text-sm transition-colors',
              backgroundType === 'gradient'
                ? 'border-primary bg-primary/10 text-primary'
                : 'border-input hover:bg-muted'
            )}
          >
            Gradient
          </button>
          <button
            onClick={() => setBackgroundType('image')}
            className={cn(
              'flex-1 rounded-md border px-3 py-1.5 text-sm transition-colors',
              backgroundType === 'image'
                ? 'border-primary bg-primary/10 text-primary'
                : 'border-input hover:bg-muted'
            )}
          >
            Image
          </button>
        </div>
      </div>

      {}
      {backgroundType === 'gradient' && (
        <>
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <label className="text-xs font-medium">Colors</label>
              {colors.length < 5 && (
                <button
                  onClick={handleAddColor}
                  className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs"
                >
                  <Plus className="size-3" />
                  Add
                </button>
              )}
            </div>

            <div className="flex flex-col gap-2">
              {colors.map((color, index) => (
                <div key={index} className="flex items-center gap-2">
                  <button
                    onClick={() =>
                      setActiveColorIndex(
                        activeColorIndex === index ? null : index
                      )
                    }
                    className={cn(
                      'size-8 rounded-md border-2 transition-all',
                      activeColorIndex === index
                        ? 'ring-ring ring-2 ring-offset-1'
                        : ''
                    )}
                    style={{ backgroundColor: color }}
                  />
                  <input
                    type="text"
                    value={color}
                    onChange={e => handleColorChange(index, e.target.value)}
                    className="bg-field text-field-foreground rounded-field h-8 flex-1 border-0 px-2 font-mono text-xs outline-none"
                  />
                  {colors.length > 2 && (
                    <button
                      onClick={() => handleRemoveColor(index)}
                      className="text-muted-foreground hover:text-destructive"
                    >
                      <Trash2 className="size-4" />
                    </button>
                  )}
                </div>
              ))}
            </div>

            {activeColorIndex !== null && (
              <div className="bg-muted/50 mt-1 grid grid-cols-8 gap-1.5 rounded-md p-2">
                {COLOR_PALETTE.map(paletteColor => (
                  <button
                    key={paletteColor}
                    onClick={() => {
                      handleColorChange(activeColorIndex, paletteColor);
                      setActiveColorIndex(null);
                    }}
                    className={cn(
                      'size-6 rounded-md border transition-transform hover:scale-110',
                      colors[activeColorIndex] === paletteColor
                        ? 'ring-ring ring-2'
                        : ''
                    )}
                    style={{ backgroundColor: paletteColor }}
                  />
                ))}
              </div>
            )}
          </div>

          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <label className="text-xs font-medium">Angle</label>
              <span className="text-muted-foreground text-xs tabular-nums">
                {angle}°
              </span>
            </div>
            <Slider
              size="sm"
              value={[angle]}
              onValueChange={([value]) => setAngle(value)}
              min={0}
              max={360}
              step={1}
            />
          </div>
        </>
      )}

      {}
      {backgroundType === 'image' && (
        <div className="flex flex-col gap-3">
          <button
            onClick={handleSelectImage}
            disabled={isLoadingImage}
            className={cn(
              'flex h-24 flex-col items-center justify-center gap-2 rounded-lg border-2 border-dashed transition-colors',
              isLoadingImage
                ? 'cursor-wait'
                : 'hover:border-primary hover:bg-muted/50',
              imageUrl ? 'border-solid' : ''
            )}
            style={
              imageUrl
                ? {
                    backgroundImage: `url(${imageUrl})`,
                    backgroundSize: 'cover',
                    backgroundPosition: 'center',
                  }
                : undefined
            }
          >
            {!imageUrl && !isLoadingImage && (
              <>
                <Upload className="text-muted-foreground size-6" />
                <span className="text-muted-foreground text-xs">
                  Click to select image
                </span>
              </>
            )}
            {isLoadingImage && (
              <div className="border-primary size-5 animate-spin rounded-full border-2 border-t-transparent" />
            )}
          </button>
          {imageUrl && (
            <div className="flex items-center justify-between">
              <div className="text-muted-foreground flex items-center gap-1 text-xs">
                <ImageIcon className="size-3" />
                Image selected
              </div>
              <button
                onClick={handleSelectImage}
                className="text-primary text-xs hover:underline"
              >
                Change
              </button>
            </div>
          )}
          <p className="text-muted-foreground text-xs">
            Supports PNG, JPEG, SVG, and WebP
          </p>
        </div>
      )}

      <div className="flex gap-2">
        <Button variant="ghost" size="xs" onClick={onCancel} className="flex-1">
          Cancel
        </Button>
        <Button
          size="xs"
          onClick={handleSave}
          disabled={backgroundType === 'image' && !imageUrl}
          className="flex-1"
        >
          {initialBackground ? 'Update' : 'Save'}
        </Button>
      </div>
    </div>
  );
}
