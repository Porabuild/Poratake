import type {
  AspectRatioOption,
  ImageLayer,
  WindowFrameStyle,
} from '@/types/editor';
import type { BalanceCrop } from '@/renderer/utils/color-detection';
import {
  computeLayerLayout,
  findPrimaryRect,
} from '@/renderer/utils/layer-layout';
import { WINDOW_FRAME_TITLE_BAR_HEIGHT } from '@/renderer/utils/window-frame';

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

export function computeScreenshotLayout({
  nativeWidth,
  nativeHeight,
  imageWidth,
  imageHeight,
  crop,
  padding,
  inset,
  spacing,
  windowFrame,
  aspectRatio,
  extraLayers,
}: {
  nativeWidth: number;
  nativeHeight: number;
  imageWidth: number;
  imageHeight: number;
  crop: BalanceCrop;
  padding: number;
  inset: number;
  spacing: number;
  windowFrame: WindowFrameStyle;
  aspectRatio: AspectRatioOption;
  extraLayers: ImageLayer[];
}) {
  const scale =
    nativeWidth > 0 && nativeHeight > 0 && imageWidth > 0 && imageHeight > 0
      ? Math.max(nativeWidth / imageWidth, nativeHeight / imageHeight)
      : 1;
  const nativePadding = Math.round(padding * scale);
  const nativeInset = Math.round(inset * scale);
  const titleBarHeight =
    windowFrame === 'none'
      ? 0
      : Math.round(WINDOW_FRAME_TITLE_BAR_HEIGHT * scale);
  const contentWidth = nativeWidth - crop.left - crop.right;
  const contentHeight = nativeHeight - crop.top - crop.bottom;
  const layout = computeLayerLayout({
    primaryWidth: contentWidth + nativeInset * 2,
    primaryHeight: contentHeight + titleBarHeight + nativeInset * 2,
    extraLayers: extraLayers.map(layer => ({
      ...layer,
      naturalWidth: layer.naturalWidth + nativeInset * 2,
      naturalHeight: layer.naturalHeight + titleBarHeight + nativeInset * 2,
    })),
    spacing: Math.round(spacing * scale),
  });
  const baseWidth = layout.width + nativePadding * 2;
  const baseHeight = layout.height + nativePadding * 2;
  const ratio = getAspectRatioValue(aspectRatio);
  const canvasWidth =
    ratio && baseHeight > 0 && baseWidth / baseHeight < ratio
      ? Math.round(baseHeight * ratio)
      : baseWidth;
  const canvasHeight =
    ratio && baseHeight > 0 && baseWidth / baseHeight > ratio
      ? Math.round(baseWidth / ratio)
      : baseHeight;
  const offsetX = nativePadding + (canvasWidth - baseWidth) / 2;
  const offsetY = nativePadding + (canvasHeight - baseHeight) / 2;
  const primaryRect = findPrimaryRect(layout.rects);
  return {
    scale,
    nativePadding,
    nativeInset,
    titleBarHeight,
    contentWidth,
    contentHeight,
    layout,
    primaryRect,
    canvasWidth,
    canvasHeight,
    offsetX,
    offsetY,
    contentOffsetX: offsetX + primaryRect.x + nativeInset,
    contentOffsetY: offsetY + primaryRect.y + nativeInset + titleBarHeight,
  };
}
