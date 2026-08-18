import { XIcon, Plus, Pencil, Trash2, Monitor } from 'lucide-react';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/renderer/components/ui/tooltip';
import { Slider } from '@/renderer/components/ui/slider';
import { Switch } from '@/renderer/components/ui/switch';
import type {
  WallpaperSettings,
  GradientOption,
  WindowFrameStyle,
  AspectRatioOption,
} from '@/types/editor';
import type { CustomBackground, WallpaperPreset } from '@/types/settings';
import { SVG_WALLPAPER_PRESETS } from '@/renderer/hooks/useWallpaperState';
import { cn } from '@/renderer/lib/utils';
import { Separator } from '@/renderer/components/ui/separator';
import { useState, useEffect, useCallback, useMemo } from 'react';
import BackgroundEditor from './background-editor';
import PresetManager from './preset-manager';
import WindowFramePreview from './window-frame-preview';
import AspectRatioSelector from './aspect-ratio-selector';

const WINDOW_FRAME_STYLES: WindowFrameStyle[] = [
  'none',
  'macos-light',
  'macos-dark',
  'windows-light',
  'windows-dark',
];

interface WallpaperSheetProps {
  wallpaper: WallpaperSettings;
  hasMultipleLayers?: boolean;
  onGradientChange: (gradient: GradientOption | null) => void;
  onBackgroundImageChange: (image: string | null) => void;
  onBackgroundBlurChange: (blur: number) => void;
  onNoiseChange: (noise: number) => void;
  onPaddingChange: (padding: number) => void;
  onInsetChange: (inset: number) => void;
  onCornersChange: (corners: number) => void;
  onShadowChange: (shadow: number) => void;
  onSpacingChange: (spacing: number) => void;
  onWindowFrameChange: (style: WindowFrameStyle) => void;
  onBalanceChange: (balance: boolean) => void;
  onAspectRatioChange: (aspectRatio: AspectRatioOption) => void;
  onApplyPreset?: (preset: WallpaperPreset) => void;
}

export function WallpaperSheetContent({
  wallpaper,
  hasMultipleLayers = false,
  onGradientChange,
  onBackgroundImageChange,
  onBackgroundBlurChange,
  onNoiseChange,
  onPaddingChange,
  onInsetChange,
  onCornersChange,
  onShadowChange,
  onSpacingChange,
  onWindowFrameChange,
  onBalanceChange,
  onAspectRatioChange,
  onApplyPreset,
  onClose,
  isOpen,
}: WallpaperSheetProps & { onClose: () => void; isOpen: boolean }) {
  const [shouldRender, setShouldRender] = useState(false);
  const [isAnimating, setIsAnimating] = useState(false);

  const [customBackgrounds, setCustomBackgrounds] = useState<
    CustomBackground[]
  >([]);
  const [presets, setPresets] = useState<WallpaperPreset[]>([]);
  const [defaultPresetId, setDefaultPresetId] = useState<string | null>(null);

  const [showBackgroundEditor, setShowBackgroundEditor] = useState(false);
  const [editingBackground, setEditingBackground] =
    useState<CustomBackground | null>(null);
  const [isLoadingDesktopWallpaper, setIsLoadingDesktopWallpaper] =
    useState(false);
  const [desktopWallpaperPreview, setDesktopWallpaperPreview] = useState<
    string | null
  >(null);
  const [desktopWallpaperError, setDesktopWallpaperError] = useState(false);

  const selectedCustomBackground = useMemo(() => {
    if (wallpaper.gradient) {
      return customBackgrounds.find(b => {
        if (b.type !== 'gradient') return false;
        return b.id === wallpaper.gradient?.id;
      });
    }
    return customBackgrounds.find(b => {
      if (b.type !== 'image') return false;
      return wallpaper.backgroundImage === b.data.imageUrl;
    });
  }, [wallpaper.gradient, wallpaper.backgroundImage, customBackgrounds]);

  useEffect(() => {
    const loadWallpaperSettings = async () => {
      try {
        const settings = await window.ipcRenderer.invoke(
          'wallpaper:getSettings'
        );
        setCustomBackgrounds(settings.customBackgrounds ?? []);
        setPresets(settings.presets ?? []);
        setDefaultPresetId(settings.defaultPresetId ?? null);
      } catch (error) {
        console.error('Failed to load wallpaper settings:', error);
      }
    };
    loadWallpaperSettings();
  }, []);

  useEffect(() => {
    const fetchDesktopWallpaperPreview = async () => {
      try {
        setIsLoadingDesktopWallpaper(true);
        const wallpaperDataUrl = await window.ipcRenderer.invoke(
          'wallpaper:getDesktopWallpaper'
        );
        if (wallpaperDataUrl) {
          setDesktopWallpaperPreview(wallpaperDataUrl);
          setDesktopWallpaperError(false);
        } else {
          setDesktopWallpaperError(true);
        }
      } catch (error) {
        console.error('Failed to fetch desktop wallpaper preview:', error);
        setDesktopWallpaperError(true);
      } finally {
        setIsLoadingDesktopWallpaper(false);
      }
    };
    fetchDesktopWallpaperPreview();
  }, []);

  useEffect(() => {
    if (isOpen) {
      setShouldRender(true);
      const timeout = setTimeout(() => {
        setIsAnimating(true);
      }, 10);
      return () => clearTimeout(timeout);
    } else {
      setIsAnimating(false);
      const timeout = setTimeout(() => {
        setShouldRender(false);
      }, 300);
      return () => clearTimeout(timeout);
    }
  }, [isOpen]);

  const handleSaveBackground = useCallback(
    async (background: CustomBackground) => {
      try {
        let updatedBackgrounds: CustomBackground[];
        if (editingBackground) {
          updatedBackgrounds = await window.ipcRenderer.invoke(
            'wallpaper:updateBackground',
            background
          );
        } else {
          updatedBackgrounds = await window.ipcRenderer.invoke(
            'wallpaper:addBackground',
            background
          );
        }
        setCustomBackgrounds(updatedBackgrounds);
        setShowBackgroundEditor(false);
        setEditingBackground(null);

        if (background.type === 'gradient') {
          onBackgroundImageChange(null);
          onGradientChange({
            id: background.id,
            colors: background.data.colors,
            angle: background.data.angle,
          });
        } else if (background.type === 'image') {
          onGradientChange(null);
          onBackgroundImageChange(background.data.imageUrl);
        }
      } catch (error) {
        console.error('Failed to save background:', error);
      }
    },
    [editingBackground, onGradientChange, onBackgroundImageChange]
  );

  const handleDeleteBackground = useCallback(
    async (id: string) => {
      try {
        const backgroundToDelete = customBackgrounds.find(b => b.id === id);
        const updatedBackgrounds = await window.ipcRenderer.invoke(
          'wallpaper:deleteBackground',
          id
        );
        setCustomBackgrounds(updatedBackgrounds);
        if (wallpaper.gradient?.id === id) {
          onGradientChange(null);
        }
        if (
          backgroundToDelete?.type === 'image' &&
          wallpaper.backgroundImage === backgroundToDelete.data.imageUrl
        ) {
          onBackgroundImageChange(null);
        }
      } catch (error) {
        console.error('Failed to delete background:', error);
      }
    },
    [
      customBackgrounds,
      wallpaper.gradient?.id,
      wallpaper.backgroundImage,
      onGradientChange,
      onBackgroundImageChange,
    ]
  );

  const handleEditBackground = useCallback((background: CustomBackground) => {
    setEditingBackground(background);
    setShowBackgroundEditor(true);
  }, []);

  const handleSavePreset = useCallback(async (preset: WallpaperPreset) => {
    try {
      const updatedPresets = await window.ipcRenderer.invoke(
        'wallpaper:addPreset',
        preset
      );
      setPresets(updatedPresets);
    } catch (error) {
      console.error('Failed to save preset:', error);
    }
  }, []);

  const handleDeletePreset = useCallback(async (id: string) => {
    try {
      const updatedPresets = await window.ipcRenderer.invoke(
        'wallpaper:deletePreset',
        id
      );
      setPresets(updatedPresets);
      setDefaultPresetId(current => (current === id ? null : current));
    } catch (error) {
      console.error('Failed to delete preset:', error);
    }
  }, []);

  const handleSetDefaultPreset = useCallback(async (id: string | null) => {
    try {
      const nextId = await window.ipcRenderer.invoke(
        'wallpaper:setDefaultPreset',
        id
      );
      setDefaultPresetId(nextId ?? null);
    } catch (error) {
      console.error('Failed to set default preset:', error);
    }
  }, []);

  const handleLoadPreset = useCallback(
    (preset: WallpaperPreset) => {
      if (onApplyPreset) {
        onApplyPreset(preset);
        return;
      }

      onGradientChange(preset.gradient);
      onBackgroundImageChange(preset.backgroundImage ?? null);
      onBackgroundBlurChange(preset.backgroundBlur ?? 0);
      onNoiseChange(preset.noise ?? 0);
      onPaddingChange(preset.padding);
      onCornersChange(preset.corners);
      onShadowChange(preset.shadow);
      onWindowFrameChange(preset.windowFrame?.style ?? 'none');
      if (typeof preset.spacing === 'number') {
        onSpacingChange(preset.spacing);
      }
    },
    [
      onApplyPreset,
      onGradientChange,
      onBackgroundImageChange,
      onBackgroundBlurChange,
      onNoiseChange,
      onPaddingChange,
      onCornersChange,
      onShadowChange,
      onWindowFrameChange,
      onSpacingChange,
    ]
  );

  const handleUseDesktopWallpaper = useCallback(async () => {
    if (desktopWallpaperPreview) {
      onBackgroundImageChange(desktopWallpaperPreview);
      return;
    }

    setIsLoadingDesktopWallpaper(true);
    try {
      const wallpaperDataUrl = await window.ipcRenderer.invoke(
        'wallpaper:getDesktopWallpaper'
      );
      if (wallpaperDataUrl) {
        setDesktopWallpaperPreview(wallpaperDataUrl);
        setDesktopWallpaperError(false);
        onBackgroundImageChange(wallpaperDataUrl);
      } else {
        setDesktopWallpaperError(true);
      }
    } catch (error) {
      console.error('Failed to get desktop wallpaper:', error);
      setDesktopWallpaperError(true);
    } finally {
      setIsLoadingDesktopWallpaper(false);
    }
  }, [onBackgroundImageChange, desktopWallpaperPreview]);

  const handleSelectCustomBackground = useCallback(
    (background: CustomBackground) => {
      if (background.type === 'gradient') {
        onBackgroundImageChange(null);
        onGradientChange({
          id: background.id,
          colors: background.data.colors,
          angle: background.data.angle,
        });
      } else if (background.type === 'image') {
        onGradientChange(null);
        onBackgroundImageChange(background.data.imageUrl);
      }
    },
    [onGradientChange, onBackgroundImageChange]
  );

  if (!shouldRender) {
    return null;
  }

  if (showBackgroundEditor) {
    return (
      <div
        className={cn(
          'flex h-full w-80 flex-col gap-4 overflow-y-auto border-r bg-popover p-5 shadow-lg transition-transform duration-300 ease-in-out',
          isAnimating ? 'translate-x-0' : '-translate-x-full'
        )}
      >
        <BackgroundEditor
          onSave={handleSaveBackground}
          onCancel={() => {
            setShowBackgroundEditor(false);
            setEditingBackground(null);
          }}
          initialBackground={editingBackground ?? undefined}
        />
      </div>
    );
  }

  return (
    <div
      className={cn(
        'flex h-full w-80 flex-col gap-4 overflow-y-auto border-r bg-popover p-5 shadow-lg transition-transform duration-300 ease-in-out',
        isAnimating ? 'translate-x-0' : '-translate-x-full'
      )}
    >
      <div className="flex items-center justify-between">
        <span className="text-sm font-medium">Wallpaper</span>
        <button
          onClick={onClose}
          className="rounded-xs bg-transparent opacity-70 ring-offset-background transition-opacity hover:opacity-100 focus:ring-2 focus:ring-ring focus:ring-offset-2 focus:outline-hidden"
        >
          <XIcon className="size-4" />
          <span className="sr-only">Close</span>
        </button>
      </div>

      <div className="flex flex-col gap-6">
        <PresetManager
          presets={presets}
          currentSettings={wallpaper}
          defaultPresetId={defaultPresetId}
          onLoadPreset={handleLoadPreset}
          onSavePreset={handleSavePreset}
          onDeletePreset={handleDeletePreset}
          onSetDefaultPreset={handleSetDefaultPreset}
        />

        <Separator />

        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-muted-foreground">
              Backgrounds
            </span>
            <div className="flex items-center gap-1">
              {selectedCustomBackground && (
                <>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <button
                        onClick={() =>
                          handleEditBackground(selectedCustomBackground)
                        }
                        className="p-1 text-muted-foreground hover:text-foreground"
                      >
                        <Pencil className="size-3" />
                      </button>
                    </TooltipTrigger>
                    <TooltipContent>Edit</TooltipContent>
                  </Tooltip>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <button
                        onClick={() =>
                          handleDeleteBackground(selectedCustomBackground.id)
                        }
                        className="p-1 text-muted-foreground hover:text-destructive"
                      >
                        <Trash2 className="size-3" />
                      </button>
                    </TooltipTrigger>
                    <TooltipContent>Delete</TooltipContent>
                  </Tooltip>
                </>
              )}
              <Tooltip>
                <TooltipTrigger asChild>
                  <button
                    onClick={() => setShowBackgroundEditor(true)}
                    className="p-1 text-muted-foreground hover:text-foreground"
                  >
                    <Plus className="size-3.5" />
                  </button>
                </TooltipTrigger>
                <TooltipContent>Add Background</TooltipContent>
              </Tooltip>
            </div>
          </div>
          <div className="grid grid-cols-5 gap-2">
            <Tooltip>
              <TooltipTrigger asChild>
                <button
                  onClick={handleUseDesktopWallpaper}
                  disabled={isLoadingDesktopWallpaper || desktopWallpaperError}
                  className={cn(
                    'relative aspect-square overflow-hidden rounded-lg transition-all',
                    wallpaper.backgroundImage &&
                      wallpaper.backgroundImage === desktopWallpaperPreview
                      ? 'ring-2 ring-ring ring-offset-2'
                      : 'hover:scale-105',
                    isLoadingDesktopWallpaper && 'cursor-wait',
                    desktopWallpaperError && 'cursor-not-allowed opacity-50'
                  )}
                  style={{
                    backgroundImage: desktopWallpaperPreview
                      ? `url(${desktopWallpaperPreview})`
                      : undefined,
                    backgroundSize: 'cover',
                    backgroundPosition: 'center',
                  }}
                >
                  {!desktopWallpaperPreview && !isLoadingDesktopWallpaper && (
                    <div className="flex h-full w-full items-center justify-center bg-muted">
                      <Monitor className="size-4 text-muted-foreground" />
                    </div>
                  )}
                  {isLoadingDesktopWallpaper && (
                    <div className="absolute inset-0 flex items-center justify-center bg-muted">
                      <div className="size-3 animate-spin rounded-full border-2 border-primary border-t-transparent" />
                    </div>
                  )}
                </button>
              </TooltipTrigger>
              <TooltipContent>
                {desktopWallpaperError
                  ? 'Unable to access desktop wallpaper'
                  : wallpaper.backgroundImage
                    ? 'Desktop Wallpaper (active)'
                    : 'Use Desktop Wallpaper'}
              </TooltipContent>
            </Tooltip>

            {SVG_WALLPAPER_PRESETS.map(preset => (
              <button
                key={preset.id}
                onClick={() => onBackgroundImageChange(preset.imageUrl)}
                className={cn(
                  'aspect-square rounded-lg transition-all',
                  wallpaper.backgroundImage === preset.imageUrl
                    ? 'ring-2 ring-ring ring-offset-2'
                    : 'hover:scale-105'
                )}
                style={{
                  backgroundImage: `url(${preset.imageUrl})`,
                  backgroundSize: 'cover',
                  backgroundPosition: 'center',
                }}
                title={preset.name}
              />
            ))}

            {customBackgrounds.map(background => {
              if (background.type === 'gradient') {
                return (
                  <button
                    key={background.id}
                    onClick={() => handleSelectCustomBackground(background)}
                    className={cn(
                      'aspect-square rounded-lg transition-all',
                      wallpaper.gradient?.id === background.id
                        ? 'ring-2 ring-ring ring-offset-2'
                        : 'hover:scale-105'
                    )}
                    style={{
                      background: `linear-gradient(${background.data.angle}deg, ${background.data.colors.join(', ')})`,
                    }}
                  />
                );
              }
              if (background.type === 'image') {
                const isSelected =
                  wallpaper.backgroundImage === background.data.imageUrl;
                return (
                  <button
                    key={background.id}
                    onClick={() => handleSelectCustomBackground(background)}
                    className={cn(
                      'aspect-square rounded-lg transition-all',
                      isSelected
                        ? 'ring-2 ring-ring ring-offset-2'
                        : 'hover:scale-105'
                    )}
                    style={{
                      backgroundImage: `url(${background.data.imageUrl})`,
                      backgroundSize: 'cover',
                      backgroundPosition: 'center',
                    }}
                  />
                );
              }
              return null;
            })}
          </div>

          {(wallpaper.backgroundImage || wallpaper.gradient) && (
            <div className="flex flex-col gap-3">
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium">Blur</span>
                <span className="text-xs tabular-nums">
                  {wallpaper.backgroundBlur ?? 0}
                </span>
              </div>
              <Slider
                size="sm"
                value={[wallpaper.backgroundBlur ?? 0]}
                onValueChange={([value]) => onBackgroundBlurChange(value)}
                min={0}
                max={100}
                step={1}
              />
            </div>
          )}

          {(wallpaper.backgroundImage || wallpaper.gradient) && (
            <div className="flex flex-col gap-3">
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium">Noise</span>
                <span className="text-xs tabular-nums">
                  {wallpaper.noise ?? 0}
                </span>
              </div>
              <Slider
                size="sm"
                value={[wallpaper.noise ?? 0]}
                onValueChange={([value]) => onNoiseChange(value)}
                min={0}
                max={100}
                step={1}
              />
            </div>
          )}

          {(wallpaper.backgroundImage || wallpaper.gradient) && (
            <button
              onClick={() => {
                onBackgroundImageChange(null);
                onGradientChange(null);
              }}
              className="text-xs text-muted-foreground hover:text-foreground"
            >
              Clear background
            </button>
          )}
        </div>

        <Separator />

        <AspectRatioSelector
          value={wallpaper.aspectRatio}
          onChange={onAspectRatioChange}
        />

        <Separator />

        <div className="flex items-center justify-between">
          <span className="text-xs font-medium">Balance</span>
          <Switch
            size="sm"
            checked={wallpaper.balance}
            onCheckedChange={onBalanceChange}
          />
        </div>

        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium">Padding</span>
            <span className="text-xs tabular-nums">{wallpaper.padding}</span>
          </div>
          <Slider
            size="sm"
            value={[wallpaper.padding]}
            onValueChange={([value]) => onPaddingChange(value)}
            min={0}
            max={300}
            step={1}
          />
        </div>

        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium">Inset</span>
            <span className="text-xs tabular-nums">{wallpaper.inset}</span>
          </div>
          <Slider
            size="sm"
            value={[wallpaper.inset]}
            onValueChange={([value]) => onInsetChange(value)}
            min={0}
            max={200}
            step={1}
          />
        </div>

        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium">Corners</span>
            <span className="text-xs tabular-nums">{wallpaper.corners}</span>
          </div>
          <Slider
            size="sm"
            value={[wallpaper.corners]}
            onValueChange={([value]) => onCornersChange(value)}
            min={0}
            max={200}
            step={1}
          />
        </div>

        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium">Shadow</span>
            <span className="text-xs tabular-nums">{wallpaper.shadow}</span>
          </div>
          <Slider
            size="sm"
            value={[wallpaper.shadow]}
            onValueChange={([value]) => onShadowChange(value)}
            min={0}
            max={300}
            step={1}
          />
        </div>

        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <span
              className={cn(
                'text-xs font-medium',
                !hasMultipleLayers && 'text-muted-foreground'
              )}
            >
              Spacing
            </span>
            <span className="text-xs tabular-nums">{wallpaper.spacing}</span>
          </div>
          <Slider
            size="sm"
            value={[wallpaper.spacing]}
            onValueChange={([value]) => onSpacingChange(value)}
            min={0}
            max={200}
            step={1}
            disabled={!hasMultipleLayers}
          />
          {!hasMultipleLayers && (
            <span className="text-xs text-muted-foreground">
              Drop another image to enable spacing
            </span>
          )}
        </div>

        <Separator />

        <div className="flex flex-col gap-3">
          <span className="text-xs font-medium text-muted-foreground">
            Window Frame
          </span>
          <div className="grid grid-cols-3 gap-2">
            {WINDOW_FRAME_STYLES.map(style => (
              <WindowFramePreview
                key={style}
                style={style}
                isSelected={wallpaper.windowFrame?.style === style}
                onClick={() => onWindowFrameChange(style)}
              />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

export default WallpaperSheetContent;
