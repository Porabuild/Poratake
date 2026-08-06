import { useMemo } from 'react';
import type {
  AspectRatioOption,
  ImageLayer,
  WallpaperSettings,
} from '@/types/editor';
import {
  detectDominantEdgeColor,
  detectContentBounds,
  sampleDominantInsetColor,
  type BalanceCrop,
} from '@/renderer/utils/color-detection';
import {
  computeLayerLayout,
  findPrimaryRect,
  type LayerRect,
} from '@/renderer/utils/layer-layout';

const WINDOW_FRAME_TITLE_BAR_HEIGHT = 28;

interface UseContentDimensionsProps {
  image: HTMLImageElement | null;
  imageWidth: number;
  imageHeight: number;
  wallpaper: WallpaperSettings;
  extraLayers?: ImageLayer[];
  extraLayerImages?: Record<string, HTMLImageElement>;
}

export interface ContentDimensions {
  originalWidth: number;
  originalHeight: number;

  contentWidth: number;
  contentHeight: number;

  balanceCrop: BalanceCrop;

  nativeBalanceCrop: BalanceCrop;

  titleBarHeight: number;

  contentOffsetX: number;
  contentOffsetY: number;

  frameWidth: number;
  frameHeight: number;

  canvasWidth: number;
  canvasHeight: number;

  displayScaleX: number;
  displayScaleY: number;

  aspectRatioPaddingX: number;
  aspectRatioPaddingY: number;

  layerRects: LayerRect[];
  primaryRect: LayerRect;

  primaryInsetColor: string | null;
  layerInsetColors: Record<string, string | null>;
}

const ASPECT_RATIO_VALUES: Record<
  Exclude<AspectRatioOption, 'auto'>,
  number
> = {
  '1:1': 1,
  '4:3': 4 / 3,
  '3:2': 3 / 2,
  '16:9': 16 / 9,
  '16:10': 16 / 10,
  '21:9': 21 / 9,
  '9:16': 9 / 16,
  '3:4': 3 / 4,
  '2:3': 2 / 3,
};

export function getAspectRatioValue(
  aspectRatio: AspectRatioOption
): number | null {
  if (aspectRatio === 'auto') return null;
  return ASPECT_RATIO_VALUES[aspectRatio] ?? null;
}

export function useContentDimensions({
  image,
  imageWidth,
  imageHeight,
  wallpaper,
  extraLayers = [],
  extraLayerImages = {},
}: UseContentDimensionsProps): ContentDimensions {
  const { padding, inset, balance, windowFrame, aspectRatio, spacing } =
    wallpaper;

  const titleBarHeight =
    windowFrame?.style !== 'none' ? WINDOW_FRAME_TITLE_BAR_HEIGHT : 0;

  const displayScaleX = image ? imageWidth / image.naturalWidth : 1;
  const displayScaleY = image ? imageHeight / image.naturalHeight : 1;

  const originalEdgeColor = useMemo(() => {
    if (!image || !balance) return null;
    return detectDominantEdgeColor(image);
  }, [image, balance]);

  const nativeBalanceCrop: BalanceCrop = useMemo(() => {
    if (!balance || !image || !originalEdgeColor) {
      return { left: 0, top: 0, right: 0, bottom: 0 };
    }

    const bounds = detectContentBounds(image, originalEdgeColor);
    if (!bounds) return { left: 0, top: 0, right: 0, bottom: 0 };

    return bounds;
  }, [image, balance, originalEdgeColor]);

  const balanceCrop: BalanceCrop = useMemo(
    () => ({
      left: nativeBalanceCrop.left * displayScaleX,
      right: nativeBalanceCrop.right * displayScaleX,
      top: nativeBalanceCrop.top * displayScaleY,
      bottom: nativeBalanceCrop.bottom * displayScaleY,
    }),
    [nativeBalanceCrop, displayScaleX, displayScaleY]
  );

  const contentWidth = imageWidth - balanceCrop.left - balanceCrop.right;
  const contentHeight = imageHeight - balanceCrop.top - balanceCrop.bottom;

  const primaryFramedWidth = contentWidth + inset * 2;
  const primaryFramedHeight = contentHeight + titleBarHeight + inset * 2;

  const layout = useMemo(() => {
    const layoutInput = {
      primaryWidth: primaryFramedWidth,
      primaryHeight: primaryFramedHeight,
      extraLayers: extraLayers.map(l => ({
        ...l,
        naturalWidth: l.naturalWidth + inset * 2,
        naturalHeight: l.naturalHeight + titleBarHeight + inset * 2,
      })),
      spacing,
    };
    return computeLayerLayout(layoutInput);
  }, [
    primaryFramedWidth,
    primaryFramedHeight,
    extraLayers,
    spacing,
    inset,
    titleBarHeight,
  ]);

  const frameWidth = layout.width;
  const frameHeight = layout.height;

  const baseCanvasWidth = frameWidth + padding * 2;
  const baseCanvasHeight = frameHeight + padding * 2;

  const targetRatio = getAspectRatioValue(aspectRatio ?? 'auto');
  let aspectRatioPaddingX = 0;
  let aspectRatioPaddingY = 0;

  if (targetRatio !== null && baseCanvasWidth > 0 && baseCanvasHeight > 0) {
    const currentRatio = baseCanvasWidth / baseCanvasHeight;

    if (currentRatio < targetRatio) {
      const newWidth = baseCanvasHeight * targetRatio;
      aspectRatioPaddingX = (newWidth - baseCanvasWidth) / 2;
    } else if (currentRatio > targetRatio) {
      const newHeight = baseCanvasWidth / targetRatio;
      aspectRatioPaddingY = (newHeight - baseCanvasHeight) / 2;
    }
  }

  const canvasWidth = baseCanvasWidth + aspectRatioPaddingX * 2;
  const canvasHeight = baseCanvasHeight + aspectRatioPaddingY * 2;

  const primaryRect = findPrimaryRect(layout.rects);

  const contentOffsetX = padding + aspectRatioPaddingX + primaryRect.x + inset;
  const contentOffsetY =
    padding + aspectRatioPaddingY + primaryRect.y + inset + titleBarHeight;

  const primaryInsetColor = useMemo(() => {
    if (!image || inset <= 0) return null;
    return sampleDominantInsetColor(image, nativeBalanceCrop);
  }, [image, inset, nativeBalanceCrop]);

  const layerInsetColors = useMemo(() => {
    if (inset <= 0) return {} as Record<string, string | null>;
    const out: Record<string, string | null> = {};
    for (const layer of extraLayers) {
      const img = extraLayerImages[layer.id];
      out[layer.id] = img ? sampleDominantInsetColor(img) : null;
    }
    return out;
  }, [extraLayers, extraLayerImages, inset]);

  return {
    originalWidth: imageWidth,
    originalHeight: imageHeight,
    contentWidth,
    contentHeight,
    balanceCrop,
    nativeBalanceCrop,
    titleBarHeight,
    contentOffsetX,
    contentOffsetY,
    frameWidth,
    frameHeight,
    canvasWidth,
    canvasHeight,
    displayScaleX,
    displayScaleY,
    aspectRatioPaddingX,
    aspectRatioPaddingY,
    layerRects: layout.rects,
    primaryRect,
    primaryInsetColor,
    layerInsetColors,
  };
}
