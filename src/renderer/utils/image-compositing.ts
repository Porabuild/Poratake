import type { DropEdge } from '@/renderer/hooks/useImageDrop';

interface ImageNaturalSize {
  naturalWidth: number;
  naturalHeight: number;
}

export interface LayerScaledSize {
  width: number;
  height: number;
}

export function scaleLayerToEdge(
  anchor: ImageNaturalSize,
  layer: ImageNaturalSize,
  edge: NonNullable<DropEdge>
): LayerScaledSize {
  const isHorizontal = edge === 'left' || edge === 'right';

  if (isHorizontal) {
    const scale = anchor.naturalHeight / layer.naturalHeight;
    return {
      width: Math.round(layer.naturalWidth * scale),
      height: anchor.naturalHeight,
    };
  }

  const scale = anchor.naturalWidth / layer.naturalWidth;
  return {
    width: anchor.naturalWidth,
    height: Math.round(layer.naturalHeight * scale),
  };
}
