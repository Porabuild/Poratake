import { useMemo } from 'react';
import type { ImageLayer, WallpaperSettings } from '@/types/editor';
import {
  detectDominantEdgeColor,
  detectContentBounds,
  sampleDominantInsetColor,
  type BalanceCrop,
} from '@/renderer/utils/color-detection';
import { findPrimaryRect, type LayerRect } from '@/renderer/utils/layer-layout';
import { computeScreenshotLayout } from '@/renderer/utils/screenshot-layout';
export { getAspectRatioValue } from '@/renderer/utils/screenshot-layout';

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

  const composition = useMemo(
    () =>
      computeScreenshotLayout({
        nativeWidth: image?.naturalWidth ?? imageWidth,
        nativeHeight: image?.naturalHeight ?? imageHeight,
        imageWidth,
        imageHeight,
        crop: nativeBalanceCrop,
        padding,
        inset,
        spacing,
        windowFrame: windowFrame?.style ?? 'none',
        aspectRatio: aspectRatio ?? 'auto',
        extraLayers,
      }),
    [
      image,
      imageWidth,
      imageHeight,
      nativeBalanceCrop,
      padding,
      inset,
      spacing,
      windowFrame?.style,
      aspectRatio,
      extraLayers,
    ]
  );
  const scale = composition.scale;
  const balanceCrop: BalanceCrop = useMemo(
    () => ({
      left: nativeBalanceCrop.left / scale,
      right: nativeBalanceCrop.right / scale,
      top: nativeBalanceCrop.top / scale,
      bottom: nativeBalanceCrop.bottom / scale,
    }),
    [nativeBalanceCrop, scale]
  );
  const contentWidth = composition.contentWidth / scale;
  const contentHeight = composition.contentHeight / scale;
  const titleBarHeight = composition.titleBarHeight / scale;
  const layout = useMemo(
    () => ({
      width: composition.layout.width / scale,
      height: composition.layout.height / scale,
      rects: composition.layout.rects.map(rect => ({
        ...rect,
        x: rect.x / scale,
        y: rect.y / scale,
        width: rect.width / scale,
        height: rect.height / scale,
      })),
    }),
    [composition, scale]
  );
  const frameWidth = layout.width;
  const frameHeight = layout.height;
  const canvasWidth = composition.canvasWidth / scale;
  const canvasHeight = composition.canvasHeight / scale;
  const aspectRatioPaddingX =
    (composition.offsetX - composition.nativePadding) / scale;
  const aspectRatioPaddingY =
    (composition.offsetY - composition.nativePadding) / scale;
  const primaryRect = findPrimaryRect(layout.rects);
  const contentOffsetX = composition.contentOffsetX / scale;
  const contentOffsetY = composition.contentOffsetY / scale;

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
