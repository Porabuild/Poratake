import {
  lazy,
  Suspense,
  useEffect,
  useCallback,
  useRef,
  useState,
  useMemo,
} from 'react';
import TitleBar from '@/renderer/components/title-bar';
import EditorCanvas, {
  type EditorCanvasHandle,
} from '@/renderer/components/editor/editor-canvas';
import ZoomControl, {
  MAX_FIT_ZOOM,
  MAX_ZOOM,
  MIN_ZOOM,
  ZOOM_STEP,
} from '@/renderer/components/editor/zoom';
import DropZoneOverlay from '@/renderer/components/editor/drop-zone-overlay';
import CaptureEdgeOverlay from '@/renderer/components/editor/capture-edge-overlay';
import { copyImageToClipboard } from '@/renderer/utils/clipboard';
import { printImage } from '@/renderer/utils/print';
import { loadImageFromBase64 } from '@/renderer/utils/image-compositing';
import { useEditorState } from '@/renderer/hooks/useEditorState';
import { useHistory } from '@/renderer/hooks/useHistory';
import { useCanvasExport } from '@/renderer/hooks/useCanvasExport';
import { useWallpaperState } from '@/renderer/hooks/useWallpaperState';
import { useEditorToolShortcuts } from '@/renderer/hooks/useEditorToolShortcuts';
import { useAcceleratorShortcut } from '@/renderer/hooks/use-accelerator-shortcut';
import { useAnnotationClipboard } from '@/renderer/hooks/useAnnotationClipboard';
import { useContentDimensions } from '@/renderer/hooks/useContentDimensions';
import { useImageDrop, type DropEdge } from '@/renderer/hooks/useImageDrop';
import { usePanOnDrag } from '@/renderer/hooks/use-pan-on-drag';
import {
  renumberAnnotations,
  getNextNumberValue,
} from '@/renderer/components/editor/number/number-utils';
import { getFontFamilyCSS } from '@/renderer/components/editor/text/text-utils';
import type {
  Annotation,
  ArrowStyle,
  HighlightColor,
  HighlightOpacity,
  ImageLayer,
  NumberSize,
  RedactIntensity,
  RedactStyle,
  ShapeFillMode,
  TextFontFamily,
  TextFontSize,
} from '@/types/editor';
import type {
  EditorActionShortcuts,
  EditorPreferences,
  EditorShortcuts,
  ScreenshotFormat,
} from '@/types/settings';
import { DEFAULT_UPLOAD_TO_CLOUD_SHORTCUT } from '@/types/settings';
import type { CloudUploadState } from '@/types/cloud';
import type { EditorState } from '@/types/history';
import { shouldIgnoreGlobalKeyboardShortcuts } from '@/renderer/utils/keyboard';
import { isMacPlatform } from '@/renderer/utils/platform';
import {
  loadImageSource,
  loadScreenshotImage,
} from '@/renderer/utils/screenshot-image';

const loadWallpaperSheetContent = () =>
  import('@/renderer/components/editor/wallpaper');
const WallpaperSheetContent = lazy(loadWallpaperSheetContent);

interface ScreenshotWindowProps {
  params: {
    filePath: string;
    imageUrl?: string;
    width?: number;
    height?: number;
    editorState?: EditorState;
    historyId?: string;
  };
  initialPreferences: EditorPreferences;
  screenshotSettings: {
    closeOnCopy: boolean;
    closeOnSave: boolean;
    format: ScreenshotFormat;
  };
  editorShortcuts?: EditorShortcuts;
  editorActionShortcuts?: EditorActionShortcuts;
}

export default function ScreenshotWindow({
  params,
  initialPreferences,
  screenshotSettings: initialScreenshotSettings,
  editorShortcuts,
  editorActionShortcuts,
}: ScreenshotWindowProps) {
  const {
    filePath,
    imageUrl,
    width: initialWidth,
    height: initialHeight,
    editorState: initialEditorState,
  } = params;
  const canvasRef = useRef<EditorCanvasHandle>(null);
  const dropTargetRef = useRef<HTMLDivElement>(null);
  const pan = usePanOnDrag(dropTargetRef);
  const [imageElement, setImageElement] = useState<HTMLImageElement | null>(
    null
  );
  const [width, setWidth] = useState(initialWidth);
  const [height, setHeight] = useState(initialHeight);
  const [extraLayers, setExtraLayers] = useState<ImageLayer[]>(
    initialEditorState?.layers ?? []
  );
  const [extraLayerImages, setExtraLayerImages] = useState<
    Record<string, HTMLImageElement>
  >({});
  const [zoom, setZoom] = useState(1);
  const [isCopied, setIsCopied] = useState(false);
  const [cloudUploadState, setCloudUploadState] =
    useState<CloudUploadState>('idle');
  const [selectedAnnotationIds, setSelectedAnnotationIds] = useState<string[]>(
    []
  );
  const [screenshotSettings, setScreenshotSettings] = useState(
    initialScreenshotSettings
  );
  const [isCaptureMode, setIsCaptureMode] = useState(false);
  const [isMetaHeld, setIsMetaHeld] = useState(false);
  const [hasOpenedWallpaperSheet, setHasOpenedWallpaperSheet] = useState(false);

  const updateImageSource = useCallback(async (source: string) => {
    try {
      setImageElement(await loadImageSource(source));
    } catch (error) {
      console.error('Failed to load screenshot:', error);
    }
  }, []);

  useEffect(() => {
    if (isCopied) {
      const timer = setTimeout(() => {
        setIsCopied(false);
      }, 2000);
      return () => clearTimeout(timer);
    }
  }, [isCopied]);

  useEffect(() => {
    if (cloudUploadState === 'success') {
      const timer = setTimeout(() => {
        setCloudUploadState('idle');
      }, 2000);
      return () => clearTimeout(timer);
    }
  }, [cloudUploadState]);

  useEffect(() => {
    const handleSettingsUpdate = (
      _event: unknown,
      settings: typeof screenshotSettings
    ) => {
      setScreenshotSettings(settings);
    };

    window.ipcRenderer.on('screenshot-settings:updated', handleSettingsUpdate);

    return () => {
      window.ipcRenderer.off(
        'screenshot-settings:updated',
        handleSettingsUpdate
      );
    };
  }, []);

  const lastCropStateRef = useRef<{
    imageSource: string;
    width: number | undefined;
    height: number | undefined;
    annotations: Annotation[];
  } | null>(null);

  useEffect(() => {
    let cancelled = false;

    void loadScreenshotImage(imageUrl, () =>
      window.ipcRenderer.invoke('screenshot:read-file')
    )
      .then(loadedImage => {
        if (!cancelled) {
          setImageElement(loadedImage);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          console.error('Failed to load screenshot:', error);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [filePath, imageUrl]);

  useEffect(() => {
    let cancelled = false;

    const layerIds = new Set(extraLayers.map(l => l.id));
    const missing = extraLayers.filter(l => !extraLayerImages[l.id]);

    if (missing.length === 0) {
      const hasStale = Object.keys(extraLayerImages).some(
        id => !layerIds.has(id)
      );
      if (hasStale) {
        setExtraLayerImages(prev => {
          const next: Record<string, HTMLImageElement> = {};
          for (const id of Object.keys(prev)) {
            if (layerIds.has(id)) next[id] = prev[id];
          }
          return next;
        });
      }
      return;
    }

    Promise.all(
      missing.map(async layer => {
        const img = await loadImageFromBase64(layer.base64);
        return [layer.id, img] as const;
      })
    )
      .then(loaded => {
        if (cancelled) return;
        setExtraLayerImages(prev => {
          const next: Record<string, HTMLImageElement> = {};
          for (const id of Object.keys(prev)) {
            if (layerIds.has(id)) next[id] = prev[id];
          }
          for (const [id, img] of loaded) {
            next[id] = img;
          }
          return next;
        });
      })
      .catch(err => {
        if (!cancelled) {
          console.error('Failed to load extra layer image:', err);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [extraLayers, extraLayerImages]);

  const {
    activeTool,
    selectedColor,
    strokeWidth,
    arrowStyle,
    highlightColor,
    highlightOpacity,
    numberStyle,
    numberSize,
    numberStartValue,
    textBackground,
    textFontSize,
    textFontFamily,
    redactStyle,
    redactIntensity,
    shapeFillMode,
    setActiveTool,
    setSelectedColor,
    setStrokeWidth,
    setArrowStyle,
    setHighlightColor,
    setHighlightOpacity,
    setNumberStyle,
    setNumberSize,
    setNumberStartValue,
    setTextBackground,
    setTextFontSize,
    setTextFontFamily,
    setRedactStyle,
    setRedactIntensity,
    setShapeFillMode,
  } = useEditorState({ initialPreferences });
  const {
    state: annotations,
    set: setAnnotations,
    setWithoutHistory: setAnnotationsWithoutHistory,
    commitToHistory,
    undo,
    redo,
    canUndo,
    canRedo,
  } = useHistory<Annotation[]>(initialEditorState?.annotations || []);
  const {
    wallpaper,
    setGradient,
    setBackgroundImage,
    setBackgroundBlur,
    setNoise,
    setPadding,
    setInset,
    setCorners,
    setShadow,
    setSpacing,
    setWindowFrame,
    setBalance,
    setAspectRatio,
    applyPreset,
  } = useWallpaperState(initialEditorState?.wallpaper);

  const {
    contentWidth: croppedWidth,
    contentHeight: croppedHeight,
    nativeBalanceCrop,
    canvasWidth: totalCanvasWidth,
    canvasHeight: totalCanvasHeight,
  } = useContentDimensions({
    image: imageElement,
    imageWidth: width || 0,
    imageHeight: height || 0,
    wallpaper,
    extraLayers,
  });

  const { copyAnnotations, pasteAnnotations, hasClipboard } =
    useAnnotationClipboard();

  const { exportToImage } = useCanvasExport({
    padding: wallpaper.padding,
    inset: wallpaper.inset,
    image: imageElement,
    imageWidth: width || 0,
    imageHeight: height || 0,
    cornerRadius: wallpaper.corners,
    shadow: wallpaper.shadow,
    spacing: wallpaper.spacing,
    gradient: wallpaper.gradient,
    backgroundImage: wallpaper.backgroundImage,
    backgroundBlur: wallpaper.backgroundBlur,
    noise: wallpaper.noise,
    getSvgForExport: (scale: number) =>
      canvasRef.current?.getSvgForExport(scale) ?? '',
    annotations,
    windowFrame: wallpaper.windowFrame?.style,
    balance: wallpaper.balance,
    extraLayers,
    extraLayerImages,
  });

  const handleSelectionChange = useCallback((ids: string[]) => {
    setSelectedAnnotationIds(ids);
  }, []);

  const updateSelectedAnnotations = useCallback(
    (updates: Record<string, unknown>) => {
      if (selectedAnnotationIds.length === 0) return;

      const updatedAnnotations = annotations.map(ann => {
        if (!selectedAnnotationIds.includes(ann.id)) return ann;

        const applicableUpdates: Record<string, unknown> = {};

        if ('stroke' in updates && 'stroke' in ann) {
          applicableUpdates.stroke = updates.stroke;
        }
        if ('fill' in updates && 'fill' in ann) {
          applicableUpdates.fill = updates.fill;
        }

        if ('strokeWidth' in updates && 'strokeWidth' in ann) {
          applicableUpdates.strokeWidth = updates.strokeWidth;
        }

        if ('arrowStyle' in updates && ann.type === 'arrow') {
          applicableUpdates.arrowStyle = updates.arrowStyle;
        }

        if ('size' in updates && ann.type === 'number') {
          applicableUpdates.size = updates.size;
        }

        if (ann.type === 'text') {
          if ('fontSize' in updates) {
            applicableUpdates.fontSize = updates.fontSize;
          }
          if ('fontFamily' in updates) {
            applicableUpdates.fontFamily = updates.fontFamily;
          }
          if ('backgroundColor' in updates) {
            applicableUpdates.backgroundColor = updates.backgroundColor;
          }
          if ('backgroundPadding' in updates) {
            applicableUpdates.backgroundPadding = updates.backgroundPadding;
          }
          if ('backgroundRadius' in updates) {
            applicableUpdates.backgroundRadius = updates.backgroundRadius;
          }
        }

        if (ann.type === 'redact') {
          if ('style' in updates) {
            applicableUpdates.style = updates.style;
          }
          if ('intensity' in updates) {
            applicableUpdates.intensity = updates.intensity;
          }
        }

        if (Object.keys(applicableUpdates).length === 0) return ann;
        return { ...ann, ...applicableUpdates } as Annotation;
      });

      setAnnotations(updatedAnnotations);
    },
    [selectedAnnotationIds, annotations, setAnnotations]
  );

  const handleColorChange = useCallback(
    (color: string) => {
      setSelectedColor(color);
      updateSelectedAnnotations({ stroke: color, fill: color });
    },
    [setSelectedColor, updateSelectedAnnotations]
  );

  const handleStrokeWidthChange = useCallback(
    (width: number) => {
      setStrokeWidth(width);
      updateSelectedAnnotations({ strokeWidth: width });
    },
    [setStrokeWidth, updateSelectedAnnotations]
  );

  const handleArrowStyleChange = useCallback(
    (style: ArrowStyle) => {
      setArrowStyle(style);
      updateSelectedAnnotations({ arrowStyle: style });
    },
    [setArrowStyle, updateSelectedAnnotations]
  );

  const handleNumberSizeChange = useCallback(
    (size: NumberSize) => {
      setNumberSize(size);
      updateSelectedAnnotations({ size } as Partial<Annotation>);
    },
    [setNumberSize, updateSelectedAnnotations]
  );

  const handleTextFontSizeChange = useCallback(
    (size: TextFontSize) => {
      setTextFontSize(size);
      updateSelectedAnnotations({ fontSize: size } as Partial<Annotation>);
    },
    [setTextFontSize, updateSelectedAnnotations]
  );

  const handleTextFontFamilyChange = useCallback(
    (family: TextFontFamily) => {
      setTextFontFamily(family);
      updateSelectedAnnotations({
        fontFamily: getFontFamilyCSS(family),
      } as Partial<Annotation>);
    },
    [setTextFontFamily, updateSelectedAnnotations]
  );

  const handleTextBackgroundChange = useCallback(
    (enabled: boolean) => {
      setTextBackground(enabled);
      if (enabled) {
        updateSelectedAnnotations({
          backgroundColor: 'rgba(0, 0, 0, 0.75)',
          backgroundPadding: { x: 8, y: 4 },
          backgroundRadius: 4,
        } as Partial<Annotation>);
      } else {
        updateSelectedAnnotations({
          backgroundColor: undefined,
          backgroundPadding: undefined,
          backgroundRadius: undefined,
        } as Partial<Annotation>);
      }
    },
    [setTextBackground, updateSelectedAnnotations]
  );

  const handleRedactStyleChange = useCallback(
    (style: RedactStyle) => {
      setRedactStyle(style);
      updateSelectedAnnotations({ style } as Partial<Annotation>);
    },
    [setRedactStyle, updateSelectedAnnotations]
  );

  const handleRedactIntensityChange = useCallback(
    (intensity: RedactIntensity) => {
      setRedactIntensity(intensity);
      updateSelectedAnnotations({ intensity } as Partial<Annotation>);
    },
    [setRedactIntensity, updateSelectedAnnotations]
  );

  const handleShapeFillModeChange = useCallback(
    (mode: ShapeFillMode) => {
      setShapeFillMode(mode);
      const selectedShapes = annotations.filter(
        ann =>
          selectedAnnotationIds.includes(ann.id) &&
          (ann.type === 'rectangle' || ann.type === 'circle')
      );
      if (selectedShapes.length > 0) {
        const updatedAnnotations = annotations.map(ann => {
          if (
            !selectedAnnotationIds.includes(ann.id) ||
            (ann.type !== 'rectangle' && ann.type !== 'circle')
          ) {
            return ann;
          }
          return {
            ...ann,
            fill: mode === 'filled' ? ann.stroke : undefined,
          } as Annotation;
        });
        setAnnotations(updatedAnnotations);
      }
    },
    [setShapeFillMode, annotations, selectedAnnotationIds, setAnnotations]
  );

  const handleHighlightColorChange = useCallback(
    (color: HighlightColor) => {
      setHighlightColor(color);
      updateSelectedAnnotations({ fill: color } as Partial<Annotation>);
    },
    [setHighlightColor, updateSelectedAnnotations]
  );

  const handleHighlightOpacityChange = useCallback(
    (opacity: HighlightOpacity) => {
      setHighlightOpacity(opacity);
      updateSelectedAnnotations({ opacity } as Partial<Annotation>);
    },
    [setHighlightOpacity, updateSelectedAnnotations]
  );

  const handleZoomIn = useCallback(() => {
    setZoom(prev => Math.min(prev + ZOOM_STEP, MAX_ZOOM));
  }, []);

  const handleZoomOut = useCallback(() => {
    setZoom(prev => Math.max(prev - ZOOM_STEP, MIN_ZOOM));
  }, []);

  const handleZoomReset = useCallback(() => {
    setZoom(1);
  }, []);

  const handleWheelZoom = useCallback((e: WheelEvent) => {
    if (!e.metaKey && !e.ctrlKey) return;

    e.preventDefault();
    const zoomDelta = e.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP;
    setZoom(prev => Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, prev + zoomDelta)));
  }, []);

  const calculateOptimalZoom = useCallback(
    (isSheetOpen: boolean) => {
      if (!width || !height) return 1;

      const viewportPadding = 80;
      const windowHeight = window.innerHeight - 40;

      const availableWidth = isSheetOpen
        ? window.innerWidth - 320 - viewportPadding
        : window.innerWidth - viewportPadding;
      const availableHeight = windowHeight - viewportPadding;

      const zoomX = availableWidth / totalCanvasWidth;
      const zoomY = availableHeight / totalCanvasHeight;
      const calculatedZoom = Math.round(Math.min(zoomX, zoomY) * 100) / 100;

      return Math.min(MAX_FIT_ZOOM, Math.max(MIN_ZOOM, calculatedZoom));
    },
    [width, height, totalCanvasWidth, totalCanvasHeight]
  );

  const isWallpaperSheetOpen = activeTool === 'wallpaper';

  const preloadWallpaperSheet = useCallback(() => {
    void loadWallpaperSheetContent();
  }, []);

  const handleToolChange = useCallback(
    (tool: typeof activeTool) => {
      const wasWallpaperOpen = activeTool === 'wallpaper';
      const willWallpaperOpen = tool === 'wallpaper';

      if (willWallpaperOpen) {
        setHasOpenedWallpaperSheet(true);
        preloadWallpaperSheet();
      }

      if (wasWallpaperOpen && willWallpaperOpen) {
        setActiveTool('select');
        setZoom(calculateOptimalZoom(false));
      } else {
        setActiveTool(tool);
        if (wasWallpaperOpen !== willWallpaperOpen) {
          setZoom(calculateOptimalZoom(willWallpaperOpen));
        }
      }
    },
    [activeTool, setActiveTool, calculateOptimalZoom, preloadWallpaperSheet]
  );

  useEditorToolShortcuts({
    shortcuts: editorShortcuts,
    onToolChange: handleToolChange,
  });

  useEffect(() => {
    setZoom(calculateOptimalZoom(isWallpaperSheetOpen));
  }, [
    wallpaper.padding,
    wallpaper.inset,
    wallpaper.balance,
    wallpaper.aspectRatio,
    croppedWidth,
    croppedHeight,
    isWallpaperSheetOpen,
    calculateOptimalZoom,
  ]);

  useEffect(() => {
    const fitToWindow = () =>
      setZoom(calculateOptimalZoom(isWallpaperSheetOpen));

    window.addEventListener('resize', fitToWindow);
    return () => window.removeEventListener('resize', fitToWindow);
  }, [calculateOptimalZoom, isWallpaperSheetOpen]);

  const nextNumberValue = useMemo(
    () => getNextNumberValue(annotations, numberStartValue),
    [annotations, numberStartValue]
  );

  const handleUndo = useCallback(() => {
    if (lastCropStateRef.current) {
      const restored = lastCropStateRef.current;
      void updateImageSource(restored.imageSource);
      setWidth(restored.width);
      setHeight(restored.height);
      setAnnotations(restored.annotations);
      lastCropStateRef.current = null;
      return;
    }

    undo();
  }, [undo, setAnnotations, updateImageSource]);

  const handleRedo = useCallback(() => {
    redo();
  }, [redo]);

  const prevAnnotationsRef = useRef<Annotation[]>(annotations);
  useEffect(() => {
    const hasNumberAnnotations = annotations.some(a => a.type === 'number');
    if (hasNumberAnnotations && prevAnnotationsRef.current !== annotations) {
      const numberAnnotations = annotations.filter(a => a.type === 'number');
      const needsRenumber = numberAnnotations.some((ann, index) => {
        if (ann.type !== 'number') return false;
        const expectedValue = numberStartValue + index;
        return ann.value !== expectedValue;
      });

      if (needsRenumber) {
        const renumbered = renumberAnnotations(
          annotations,
          numberStyle,
          numberStartValue
        );
        setAnnotationsWithoutHistory(renumbered);
      }
    }
    prevAnnotationsRef.current = annotations;
  }, [
    annotations,
    numberStyle,
    numberStartValue,
    setAnnotationsWithoutHistory,
  ]);

  useEffect(() => {
    window.ipcRenderer.send('screenshot:sync-state', {
      editorState: { annotations, wallpaper, layers: extraLayers },
    });
  }, [annotations, wallpaper, extraLayers]);

  const lastSavedStateRef = useRef<string | null>(null);
  const pendingStateRef = useRef<{
    annotations: Annotation[];
    wallpaper: typeof wallpaper;
    layers: ImageLayer[];
  } | null>(null);
  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const saveStateNow = useCallback(() => {
    if (pendingStateRef.current) {
      window.ipcRenderer.send(
        'history:save-editor-state',
        pendingStateRef.current
      );
      lastSavedStateRef.current = JSON.stringify(pendingStateRef.current);
      pendingStateRef.current = null;
    }
    if (saveTimeoutRef.current) {
      clearTimeout(saveTimeoutRef.current);
      saveTimeoutRef.current = null;
    }
  }, []);

  useEffect(() => {
    const currentState = JSON.stringify({
      annotations,
      wallpaper,
      layers: extraLayers,
    });

    if (currentState === lastSavedStateRef.current) return;

    if (lastSavedStateRef.current === null) {
      lastSavedStateRef.current = currentState;
      return;
    }

    pendingStateRef.current = {
      annotations,
      wallpaper,
      layers: extraLayers,
    };

    if (saveTimeoutRef.current) {
      clearTimeout(saveTimeoutRef.current);
    }

    saveTimeoutRef.current = setTimeout(() => {
      saveStateNow();
    }, 500);

    return () => {
      if (saveTimeoutRef.current) {
        clearTimeout(saveTimeoutRef.current);
      }
    };
  }, [annotations, wallpaper, extraLayers, saveStateNow]);

  useEffect(() => {
    const handleBeforeUnload = () => {
      saveStateNow();
    };

    window.addEventListener('beforeunload', handleBeforeUnload);
    return () => window.removeEventListener('beforeunload', handleBeforeUnload);
  }, [saveStateNow]);

  const handleAnnotationAdd = useCallback(
    (annotation: Annotation) => {
      setAnnotations([...annotations, annotation]);
      lastCropStateRef.current = null;
    },
    [annotations, setAnnotations]
  );

  const handleAnnotationUpdate = useCallback(
    (id: string, updates: Partial<Annotation>) => {
      const updatedAnnotations = annotations.map(ann =>
        ann.id === id ? ({ ...ann, ...updates } as Annotation) : ann
      );
      setAnnotationsWithoutHistory(updatedAnnotations);
    },
    [annotations, setAnnotationsWithoutHistory]
  );

  const handleAnnotationsUpdateMultiple = useCallback(
    (updates: Array<{ id: string; updates: Partial<Annotation> }>) => {
      const updatesMap = new Map(updates.map(u => [u.id, u.updates]));
      const updatedAnnotations = annotations.map(ann => {
        const annUpdates = updatesMap.get(ann.id);
        return annUpdates ? ({ ...ann, ...annUpdates } as Annotation) : ann;
      });
      setAnnotationsWithoutHistory(updatedAnnotations);
    },
    [annotations, setAnnotationsWithoutHistory]
  );

  const handleAnnotationDragEnd = useCallback(() => {
    commitToHistory();
  }, [commitToHistory]);

  const handleAnnotationDelete = useCallback(
    (id: string) => {
      const filteredAnnotations = annotations.filter(ann => ann.id !== id);
      const updatedAnnotations = renumberAnnotations(
        filteredAnnotations,
        numberStyle,
        numberStartValue
      );
      setAnnotations(updatedAnnotations);
      lastCropStateRef.current = null;
    },
    [annotations, setAnnotations, numberStyle, numberStartValue]
  );

  const handleAnnotationsDeleteMultiple = useCallback(
    (ids: string[]) => {
      const idsSet = new Set(ids);
      const filteredAnnotations = annotations.filter(
        ann => !idsSet.has(ann.id)
      );
      const updatedAnnotations = renumberAnnotations(
        filteredAnnotations,
        numberStyle,
        numberStartValue
      );
      setAnnotations(updatedAnnotations);
      lastCropStateRef.current = null;
    },
    [annotations, setAnnotations, numberStyle, numberStartValue]
  );

  const handleCrop = useCallback(
    (
      cropData: { x: number; y: number; width: number; height: number },
      adjustedAnnotations: Annotation[]
    ) => {
      if (!imageElement) return;

      lastCropStateRef.current = {
        imageSource: imageElement.src,
        width,
        height,
        annotations,
      };

      const canvas = document.createElement('canvas');
      const ctx = canvas.getContext('2d');
      if (!ctx) return;

      const scaleX =
        imageElement.naturalWidth / (width || imageElement.naturalWidth);
      const scaleY =
        imageElement.naturalHeight / (height || imageElement.naturalHeight);

      const actualCropX = cropData.x * scaleX + nativeBalanceCrop.left;
      const actualCropY = cropData.y * scaleY + nativeBalanceCrop.top;
      const actualCropWidth = cropData.width * scaleX;
      const actualCropHeight = cropData.height * scaleY;

      canvas.width = actualCropWidth;
      canvas.height = actualCropHeight;

      ctx.drawImage(
        imageElement,
        actualCropX,
        actualCropY,
        actualCropWidth,
        actualCropHeight,
        0,
        0,
        actualCropWidth,
        actualCropHeight
      );

      void updateImageSource(canvas.toDataURL('image/png'));
      setWidth(cropData.width);
      setHeight(cropData.height);
      setAnnotations(adjustedAnnotations);
      setActiveTool('select');
    },
    [
      imageElement,
      width,
      height,
      annotations,
      nativeBalanceCrop,
      setAnnotations,
      setActiveTool,
      updateImageSource,
    ]
  );

  const handleImageDrop = useCallback(
    async (droppedBase64: string, edge: DropEdge) => {
      if (!edge || !imageElement) return;

      try {
        const droppedImg = await loadImageFromBase64(droppedBase64);
        setExtraLayers(prev => [
          ...prev,
          {
            id: `layer-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
            base64: droppedBase64,
            naturalWidth: droppedImg.naturalWidth,
            naturalHeight: droppedImg.naturalHeight,
            edge,
          },
        ]);
      } catch (error) {
        console.error('Failed to add dropped image layer:', error);
      }
    },
    [imageElement]
  );

  const { isDragging, dropEdge } = useImageDrop({
    onImageDrop: handleImageDrop,
    dropTargetRef,
  });

  const copyScreenshot = useCallback(async () => {
    try {
      const editedImage = await exportToImage('png');
      await copyImageToClipboard(editedImage);

      setIsCopied(true);

      if (screenshotSettings.closeOnCopy) {
        window.ipcRenderer.send('screenshot:close-confirmed');
      }

      return true;
    } catch (error) {
      console.error('Failed to copy screenshot:', error);
      return false;
    }
  }, [exportToImage, screenshotSettings]);

  const handleCopyAnnotations = useCallback(() => {
    if (selectedAnnotationIds.length > 0) {
      const selected = annotations.filter(a =>
        selectedAnnotationIds.includes(a.id)
      );
      copyAnnotations(selected);
      return true;
    }
    return false;
  }, [selectedAnnotationIds, annotations, copyAnnotations]);

  const handlePasteAnnotations = useCallback(() => {
    if (hasClipboard) {
      const pasted = pasteAnnotations();
      setAnnotations([...annotations, ...pasted]);
      canvasRef.current?.selectAnnotations(pasted.map(a => a.id));
    }
  }, [hasClipboard, pasteAnnotations, annotations, setAnnotations]);

  const handleCutAnnotations = useCallback(() => {
    if (selectedAnnotationIds.length > 0) {
      const selected = annotations.filter(a =>
        selectedAnnotationIds.includes(a.id)
      );
      copyAnnotations(selected);
      const remaining = annotations.filter(
        a => !selectedAnnotationIds.includes(a.id)
      );
      setAnnotations(remaining);
      setSelectedAnnotationIds([]);
      return true;
    }
    return false;
  }, [selectedAnnotationIds, annotations, copyAnnotations, setAnnotations]);

  const saveScreenshot = useCallback(async () => {
    try {
      const format = screenshotSettings.format;
      const editedImage = await exportToImage(format);
      window.ipcRenderer.send('screenshot:save-edited', editedImage, format);
    } catch (error) {
      console.error('Failed to save screenshot:', error);
    }
  }, [exportToImage, screenshotSettings.format]);

  const isDeleting = useRef(false);
  const deleteScreenshot = useCallback(async () => {
    if (isDeleting.current) return;
    isDeleting.current = true;

    try {
      const confirmed = await window.ipcRenderer.invoke(
        'screenshot:confirmDelete'
      );
      if (confirmed) {
        window.ipcRenderer.send('screenshot:delete');
      }
    } finally {
      isDeleting.current = false;
    }
  }, []);

  const pinScreenshot = useCallback(async () => {
    try {
      const editedImage = await exportToImage('png');

      const editorState = {
        annotations,
        wallpaper,
      };

      window.ipcRenderer.send('screenshot:pin', {
        imageBase64: editedImage,
        editorState,
        filePath,
        originalWidth: width,
        originalHeight: height,
      });
    } catch (error) {
      console.error('Failed to pin screenshot:', error);
    }
  }, [exportToImage, annotations, wallpaper, filePath, width, height]);

  const uploadToCloud = useCallback(async () => {
    if (cloudUploadState === 'uploading') return;

    try {
      const isConfigured =
        await window.ipcRenderer.invoke('cloud:isConfigured');

      if (!isConfigured) {
        window.ipcRenderer.send('open-settings', 'cloud');
        return;
      }

      setCloudUploadState('uploading');

      const editedImage = await exportToImage(screenshotSettings.format);

      const result = await window.ipcRenderer.invoke(
        'cloud:upload',
        editedImage
      );

      if (result.success && result.url) {
        setCloudUploadState('success');
      } else {
        setCloudUploadState('idle');
      }
    } catch (error) {
      console.error('Failed to upload to cloud:', error);
      setCloudUploadState('idle');
    }
  }, [exportToImage, screenshotSettings.format, cloudUploadState]);

  const uploadToCloudShortcut =
    editorActionShortcuts?.uploadToCloud ?? DEFAULT_UPLOAD_TO_CLOUD_SHORTCUT;

  useAcceleratorShortcut({
    accelerator: uploadToCloudShortcut,
    onTrigger: uploadToCloud,
  });

  const printScreenshot = useCallback(async () => {
    try {
      const editedImage = await exportToImage('png');
      await printImage(editedImage);
    } catch (error) {
      console.error('Failed to print screenshot:', error);
    }
  }, [exportToImage]);

  const copyScreenshotRef = useRef(copyScreenshot);
  const saveScreenshotRef = useRef(saveScreenshot);
  const deleteScreenshotRef = useRef(deleteScreenshot);
  const handleUndoRef = useRef(handleUndo);
  const handleRedoRef = useRef(handleRedo);
  const handleZoomInRef = useRef(handleZoomIn);
  const handleZoomOutRef = useRef(handleZoomOut);
  const handleZoomResetRef = useRef(handleZoomReset);
  const screenshotSettingsRef = useRef(screenshotSettings);
  const handleCopyAnnotationsRef = useRef(handleCopyAnnotations);
  const handlePasteAnnotationsRef = useRef(handlePasteAnnotations);
  const handleCutAnnotationsRef = useRef(handleCutAnnotations);
  const printScreenshotRef = useRef(printScreenshot);

  useEffect(() => {
    copyScreenshotRef.current = copyScreenshot;
    saveScreenshotRef.current = saveScreenshot;
    deleteScreenshotRef.current = deleteScreenshot;
    handleUndoRef.current = handleUndo;
    handleRedoRef.current = handleRedo;
    handleZoomInRef.current = handleZoomIn;
    handleZoomOutRef.current = handleZoomOut;
    handleZoomResetRef.current = handleZoomReset;
    screenshotSettingsRef.current = screenshotSettings;
    handleCopyAnnotationsRef.current = handleCopyAnnotations;
    handlePasteAnnotationsRef.current = handlePasteAnnotations;
    handleCutAnnotationsRef.current = handleCutAnnotations;
    printScreenshotRef.current = printScreenshot;
  });

  useEffect(() => {
    const handleScreenshotSaved = () => {
      if (screenshotSettingsRef.current.closeOnSave) {
        window.ipcRenderer.send('screenshot:close-confirmed');
      }
    };

    const handleCopyFromMenu = async () => {
      await copyScreenshotRef.current();
    };

    const handleSaveAndClose = async () => {
      await saveScreenshotRef.current();
    };

    window.ipcRenderer.on('screenshot:saved', handleScreenshotSaved);
    window.ipcRenderer.on('screenshot:copy', handleCopyFromMenu);
    window.ipcRenderer.on('screenshot:save-and-close', handleSaveAndClose);

    const handleKeyDown = async (e: KeyboardEvent) => {
      if (shouldIgnoreGlobalKeyboardShortcuts(e.target)) {
        return;
      }

      if (e.code === 'Backspace' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        await deleteScreenshotRef.current();
        return;
      }

      if (e.code === 'KeyP' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        await printScreenshotRef.current();
        return;
      }

      if (e.code === 'KeyS' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        await saveScreenshotRef.current();
      } else if (e.code === 'KeyC' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        const copiedAnnotations = handleCopyAnnotationsRef.current();
        if (!copiedAnnotations) {
          await copyScreenshotRef.current();
        }
      } else if (e.code === 'KeyV' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handlePasteAnnotationsRef.current();
      } else if (e.code === 'KeyX' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handleCutAnnotationsRef.current();
      } else if (e.code === 'KeyZ' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        if (e.shiftKey) {
          handleRedoRef.current();
        } else {
          handleUndoRef.current();
        }
      } else if (e.code === 'Equal' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handleZoomInRef.current();
      } else if (e.code === 'Minus' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handleZoomOutRef.current();
      } else if (e.code === 'Digit0' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handleZoomResetRef.current();
      }
    };

    window.addEventListener('keydown', handleKeyDown);

    return () => {
      window.ipcRenderer.off('screenshot:saved', handleScreenshotSaved);
      window.ipcRenderer.off('screenshot:copy', handleCopyFromMenu);
      window.ipcRenderer.off('screenshot:save-and-close', handleSaveAndClose);
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, []);

  useEffect(() => {
    window.addEventListener('wheel', handleWheelZoom, { passive: false });
    return () => window.removeEventListener('wheel', handleWheelZoom);
  }, [handleWheelZoom]);

  useEffect(() => {
    const primaryModifier = isMacPlatform() ? 'Meta' : 'Control';
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === primaryModifier) setIsMetaHeld(true);
    };
    const handleKeyUp = (e: KeyboardEvent) => {
      if (e.key === primaryModifier) setIsMetaHeld(false);
    };
    const handleBlur = () => setIsMetaHeld(false);

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    window.addEventListener('blur', handleBlur);

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
      window.removeEventListener('blur', handleBlur);
    };
  }, []);

  const handleCaptureAndAttach = useCallback(
    async (
      edge: NonNullable<import('@/renderer/hooks/useImageDrop').DropEdge>
    ) => {
      if (!imageElement) return;

      setIsCaptureMode(false);
      setIsMetaHeld(false);

      try {
        const capturedBase64 = (await window.ipcRenderer.invoke(
          'screenshot:capture-for-editor'
        )) as string | null;

        if (!capturedBase64 || !imageElement) return;

        const capturedImg = await loadImageFromBase64(capturedBase64);
        setExtraLayers(prev => [
          ...prev,
          {
            id: `layer-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
            base64: capturedBase64,
            naturalWidth: capturedImg.naturalWidth,
            naturalHeight: capturedImg.naturalHeight,
            edge,
          },
        ]);
      } catch (error) {
        console.error('Failed to capture and attach:', error);
      }
    },
    [imageElement]
  );

  const handleCaptureToggle = useCallback(() => {
    setIsCaptureMode(prev => !prev);
  }, []);

  const showCaptureOverlay = isCaptureMode || isMetaHeld;

  return (
    <div className="bg-background relative flex h-screen w-full flex-col p-0 pt-10">
      <TitleBar
        activeTool={activeTool}
        onToolChange={handleToolChange}
        color={selectedColor}
        onColorChange={handleColorChange}
        strokeWidth={strokeWidth}
        onStrokeWidthChange={handleStrokeWidthChange}
        arrowStyle={arrowStyle}
        onArrowStyleChange={handleArrowStyleChange}
        highlightColor={highlightColor}
        onHighlightColorChange={handleHighlightColorChange}
        highlightOpacity={highlightOpacity}
        onHighlightOpacityChange={handleHighlightOpacityChange}
        numberStyle={numberStyle}
        onNumberStyleChange={setNumberStyle}
        numberSize={numberSize}
        onNumberSizeChange={handleNumberSizeChange}
        numberStartValue={numberStartValue}
        onNumberStartValueChange={setNumberStartValue}
        textBackground={textBackground}
        onTextBackgroundChange={handleTextBackgroundChange}
        textFontSize={textFontSize}
        onTextFontSizeChange={handleTextFontSizeChange}
        textFontFamily={textFontFamily}
        onTextFontFamilyChange={handleTextFontFamilyChange}
        redactStyle={redactStyle}
        onRedactStyleChange={handleRedactStyleChange}
        redactIntensity={redactIntensity}
        onRedactIntensityChange={handleRedactIntensityChange}
        shapeFillMode={shapeFillMode}
        onShapeFillModeChange={handleShapeFillModeChange}
        onUndo={handleUndo}
        onRedo={handleRedo}
        canUndo={canUndo}
        canRedo={canRedo}
        onCopy={copyScreenshot}
        onSave={saveScreenshot}
        onCloudUpload={uploadToCloud}
        onPin={pinScreenshot}
        isCopied={isCopied}
        cloudUploadState={cloudUploadState}
        cloudUploadShortcut={uploadToCloudShortcut}
        editorShortcuts={editorShortcuts}
        isCaptureMode={isCaptureMode}
        onCaptureClick={handleCaptureToggle}
        onWallpaperIntent={preloadWallpaperSheet}
      />
      <div className="relative flex h-full w-full">
        <div className="absolute top-0 left-0 z-10 h-full">
          {hasOpenedWallpaperSheet && (
            <Suspense
              fallback={<div className="bg-popover h-full w-80 border-r" />}
            >
              <WallpaperSheetContent
                wallpaper={wallpaper}
                hasMultipleLayers={extraLayers.length > 0}
                onGradientChange={setGradient}
                onBackgroundImageChange={setBackgroundImage}
                onBackgroundBlurChange={setBackgroundBlur}
                onNoiseChange={setNoise}
                onPaddingChange={setPadding}
                onInsetChange={setInset}
                onCornersChange={setCorners}
                onShadowChange={setShadow}
                onSpacingChange={setSpacing}
                onWindowFrameChange={setWindowFrame}
                onBalanceChange={setBalance}
                onAspectRatioChange={setAspectRatio}
                onApplyPreset={applyPreset}
                onClose={() => setActiveTool('select')}
                isOpen={isWallpaperSheetOpen}
              />
            </Suspense>
          )}
        </div>
        <div
          ref={dropTargetRef}
          className="bg-background relative h-full overflow-auto transition-all duration-300"
          onMouseDownCapture={pan.onMouseDownCapture}
          style={{
            marginLeft: isWallpaperSheetOpen ? '320px' : '0',
            width: isWallpaperSheetOpen ? 'calc(100% - 320px)' : '100%',
            cursor: pan.isPanning ? 'grabbing' : undefined,
          }}
        >
          <div
            style={{
              minWidth: '100%',
              minHeight: '100%',
              width: width && height ? `${totalCanvasWidth * zoom}px` : 'auto',
              height:
                width && height ? `${totalCanvasHeight * zoom}px` : 'auto',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              transition: 'width 0.2s ease-out, height 0.2s ease-out',
            }}
          >
            <div
              style={{
                transform: `scale(${zoom})`,
                transformOrigin: 'center center',
                transition: 'transform 0.2s ease-out',
              }}
            >
              <EditorCanvas
                ref={canvasRef}
                image={imageElement}
                imageWidth={width}
                imageHeight={height}
                extraLayers={extraLayers}
                extraLayerImages={extraLayerImages}
                annotations={annotations}
                activeTool={activeTool}
                selectedColor={selectedColor}
                strokeWidth={strokeWidth}
                arrowStyle={arrowStyle}
                highlightColor={highlightColor}
                highlightOpacity={highlightOpacity}
                numberStyle={numberStyle}
                numberSize={numberSize}
                nextNumberValue={nextNumberValue}
                onAnnotationAdd={handleAnnotationAdd}
                onAnnotationUpdate={handleAnnotationUpdate}
                onAnnotationsUpdateMultiple={handleAnnotationsUpdateMultiple}
                onAnnotationDragEnd={handleAnnotationDragEnd}
                onAnnotationDelete={handleAnnotationDelete}
                onAnnotationsDeleteMultiple={handleAnnotationsDeleteMultiple}
                onCrop={handleCrop}
                onSelectionChange={handleSelectionChange}
                wallpaper={wallpaper}
                zoom={zoom}
                textBackground={textBackground}
                textFontSize={textFontSize}
                textFontFamily={textFontFamily}
                redactStyle={redactStyle}
                redactIntensity={redactIntensity}
                shapeFillMode={shapeFillMode}
              />
            </div>
          </div>
          <DropZoneOverlay isDragging={isDragging} dropEdge={dropEdge} />
          <CaptureEdgeOverlay
            isActive={showCaptureOverlay}
            onEdgeClick={handleCaptureAndAttach}
          />
          <ZoomControl
            zoom={zoom}
            onZoomIn={handleZoomIn}
            onZoomOut={handleZoomOut}
            onZoomReset={handleZoomReset}
          />
        </div>
      </div>
    </div>
  );
}
