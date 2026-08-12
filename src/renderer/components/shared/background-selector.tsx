import { useState, useEffect, useCallback, useMemo } from 'react';
import { Plus, Pencil, Trash2, Monitor, Ban } from 'lucide-react';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/renderer/components/ui/tooltip';
import { cn } from '@/renderer/lib/utils';
import type { GradientOption } from '@/types/editor';
import type { CustomBackground } from '@/types/settings';
import { SVG_WALLPAPER_PRESETS } from '@/renderer/hooks/useWallpaperState';
import BackgroundEditor from '@/renderer/components/editor/wallpaper/background-editor';

interface BackgroundSelectorProps {
  selectedGradient: GradientOption | null;
  selectedBackgroundImage: string | null;
  onGradientChange: (gradient: GradientOption | null) => void;
  onBackgroundImageChange: (image: string | null) => void;
  showDesktopWallpaper?: boolean;
  showCustomBackgrounds?: boolean;
  /** Renders a leading "No wallpaper" tile when provided. */
  noWallpaper?: {
    selected: boolean;
    onSelect: () => void;
  };
}

export default function BackgroundSelector({
  selectedGradient,
  selectedBackgroundImage,
  onGradientChange,
  onBackgroundImageChange,
  showDesktopWallpaper = true,
  showCustomBackgrounds = true,
  noWallpaper,
}: BackgroundSelectorProps) {
  const [customBackgrounds, setCustomBackgrounds] = useState<
    CustomBackground[]
  >([]);
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
    if (selectedGradient) {
      return customBackgrounds.find(b => {
        if (b.type !== 'gradient') return false;
        return b.id === selectedGradient?.id;
      });
    }
    return customBackgrounds.find(b => {
      if (b.type !== 'image') return false;
      return selectedBackgroundImage === b.data.imageUrl;
    });
  }, [selectedGradient, selectedBackgroundImage, customBackgrounds]);

  useEffect(() => {
    const loadWallpaperSettings = async () => {
      try {
        const settings = await window.ipcRenderer.invoke(
          'wallpaper:getSettings'
        );
        setCustomBackgrounds(settings.customBackgrounds ?? []);
      } catch (error) {
        console.error('Failed to load wallpaper settings:', error);
      }
    };
    loadWallpaperSettings();
  }, []);

  useEffect(() => {
    if (!showDesktopWallpaper) return;

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
  }, [showDesktopWallpaper]);

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
        if (selectedGradient?.id === id) {
          onGradientChange(null);
        }
        if (
          backgroundToDelete?.type === 'image' &&
          selectedBackgroundImage === backgroundToDelete.data.imageUrl
        ) {
          onBackgroundImageChange(null);
        }
      } catch (error) {
        console.error('Failed to delete background:', error);
      }
    },
    [
      customBackgrounds,
      selectedGradient?.id,
      selectedBackgroundImage,
      onGradientChange,
      onBackgroundImageChange,
    ]
  );

  const handleEditBackground = useCallback((background: CustomBackground) => {
    setEditingBackground(background);
    setShowBackgroundEditor(true);
  }, []);

  const handleUseDesktopWallpaper = useCallback(async () => {
    if (desktopWallpaperPreview) {
      onGradientChange(null);
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
        onGradientChange(null);
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
  }, [onBackgroundImageChange, onGradientChange, desktopWallpaperPreview]);

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

  const handleClearBackground = useCallback(() => {
    onBackgroundImageChange(null);
    onGradientChange(null);
  }, [onBackgroundImageChange, onGradientChange]);

  // With a "No wallpaper" tile present, that tile owns the selection ring while
  // it is active so the underlying background choice stays visually unselected.
  const hasBackground = !noWallpaper?.selected;

  if (showBackgroundEditor) {
    return (
      <BackgroundEditor
        onSave={handleSaveBackground}
        onCancel={() => {
          setShowBackgroundEditor(false);
          setEditingBackground(null);
        }}
        initialBackground={editingBackground ?? undefined}
      />
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <span className="text-muted-foreground text-xs font-medium">
          Backgrounds
        </span>
        <div className="flex items-center gap-1">
          {showCustomBackgrounds && selectedCustomBackground && (
            <>
              <Tooltip>
                <TooltipTrigger asChild>
                  <button
                    onClick={() =>
                      handleEditBackground(selectedCustomBackground)
                    }
                    className="text-muted-foreground hover:text-foreground p-1"
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
                    className="text-muted-foreground hover:text-destructive p-1"
                  >
                    <Trash2 className="size-3" />
                  </button>
                </TooltipTrigger>
                <TooltipContent>Delete</TooltipContent>
              </Tooltip>
            </>
          )}
          {showCustomBackgrounds && (
            <Tooltip>
              <TooltipTrigger asChild>
                <button
                  onClick={() => setShowBackgroundEditor(true)}
                  className="text-muted-foreground hover:text-foreground p-1"
                >
                  <Plus className="size-3.5" />
                </button>
              </TooltipTrigger>
              <TooltipContent>Add Background</TooltipContent>
            </Tooltip>
          )}
        </div>
      </div>
      <div className="grid grid-cols-5 gap-2">
        {}
        {noWallpaper && (
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={noWallpaper.onSelect}
                className={cn(
                  'bg-muted flex aspect-square items-center justify-center rounded-lg transition-all',
                  noWallpaper.selected
                    ? 'ring-ring ring-2 ring-offset-2'
                    : 'hover:scale-105'
                )}
              >
                <Ban className="text-muted-foreground size-4" />
              </button>
            </TooltipTrigger>
            <TooltipContent>No Wallpaper</TooltipContent>
          </Tooltip>
        )}

        {}
        {showDesktopWallpaper && (
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={handleUseDesktopWallpaper}
                disabled={isLoadingDesktopWallpaper || desktopWallpaperError}
                className={cn(
                  'relative aspect-square overflow-hidden rounded-lg transition-all',
                  hasBackground &&
                    selectedBackgroundImage &&
                    selectedBackgroundImage === desktopWallpaperPreview
                    ? 'ring-ring ring-2 ring-offset-2'
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
                  <div className="bg-muted flex h-full w-full items-center justify-center">
                    <Monitor className="text-muted-foreground size-4" />
                  </div>
                )}
                {isLoadingDesktopWallpaper && (
                  <div className="bg-muted absolute inset-0 flex items-center justify-center">
                    <div className="border-primary size-3 animate-spin rounded-full border-2 border-t-transparent" />
                  </div>
                )}
              </button>
            </TooltipTrigger>
            <TooltipContent>
              {desktopWallpaperError
                ? 'Unable to access desktop wallpaper'
                : selectedBackgroundImage === desktopWallpaperPreview
                  ? 'Desktop Wallpaper (active)'
                  : 'Use Desktop Wallpaper'}
            </TooltipContent>
          </Tooltip>
        )}

        {}
        {SVG_WALLPAPER_PRESETS.map(preset => (
          <button
            key={preset.id}
            onClick={() => {
              onGradientChange(null);
              onBackgroundImageChange(preset.imageUrl);
            }}
            className={cn(
              'aspect-square rounded-lg transition-all',
              hasBackground && selectedBackgroundImage === preset.imageUrl
                ? 'ring-ring ring-2 ring-offset-2'
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

        {}
        {showCustomBackgrounds &&
          customBackgrounds.map(background => {
            if (background.type === 'gradient') {
              return (
                <button
                  key={background.id}
                  onClick={() => handleSelectCustomBackground(background)}
                  className={cn(
                    'aspect-square rounded-lg transition-all',
                    hasBackground && selectedGradient?.id === background.id
                      ? 'ring-ring ring-2 ring-offset-2'
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
                hasBackground &&
                selectedBackgroundImage === background.data.imageUrl;
              return (
                <button
                  key={background.id}
                  onClick={() => handleSelectCustomBackground(background)}
                  className={cn(
                    'aspect-square rounded-lg transition-all',
                    isSelected
                      ? 'ring-ring ring-2 ring-offset-2'
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

      {!noWallpaper && (selectedBackgroundImage || selectedGradient) && (
        <button
          onClick={handleClearBackground}
          className="text-muted-foreground hover:text-foreground text-xs"
        >
          Clear background
        </button>
      )}
    </div>
  );
}
