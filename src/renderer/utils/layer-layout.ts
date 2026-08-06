import type { ImageLayer, ImageLayerEdge } from '@/types/editor';
import { scaleLayerToEdge } from './image-compositing';

export interface LayerRect {
  id: string;
  edge: ImageLayerEdge;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface LayerLayoutResult {
  rects: LayerRect[];
  width: number;
  height: number;
}

interface LayoutInput {
  primaryWidth: number;
  primaryHeight: number;
  extraLayers: ImageLayer[];
  spacing: number;
}

export function computeLayerLayout({
  primaryWidth,
  primaryHeight,
  extraLayers,
  spacing,
}: LayoutInput): LayerLayoutResult {
  const primaryRect: LayerRect = {
    id: 'primary',
    edge: 'self',
    x: 0,
    y: 0,
    width: primaryWidth,
    height: primaryHeight,
  };

  const rects: LayerRect[] = [primaryRect];
  const edgeOffsets: Record<ImageLayerEdge, number> = {
    self: 0,
    top: 0,
    bottom: 0,
    left: 0,
    right: 0,
  };

  for (const layer of extraLayers) {
    if (layer.edge === 'self') continue;

    const scaled = scaleLayerToEdge(
      { naturalWidth: primaryRect.width, naturalHeight: primaryRect.height },
      {
        naturalWidth: layer.naturalWidth,
        naturalHeight: layer.naturalHeight,
      },
      layer.edge
    );

    const offset = edgeOffsets[layer.edge];
    let x = 0;
    let y = 0;

    switch (layer.edge) {
      case 'right':
        x = primaryRect.x + primaryRect.width + spacing + offset;
        y = primaryRect.y;
        edgeOffsets.right = offset + scaled.width + spacing;
        break;
      case 'left':
        x = primaryRect.x - spacing - offset - scaled.width;
        y = primaryRect.y;
        edgeOffsets.left = offset + scaled.width + spacing;
        break;
      case 'bottom':
        x = primaryRect.x;
        y = primaryRect.y + primaryRect.height + spacing + offset;
        edgeOffsets.bottom = offset + scaled.height + spacing;
        break;
      case 'top':
        x = primaryRect.x;
        y = primaryRect.y - spacing - offset - scaled.height;
        edgeOffsets.top = offset + scaled.height + spacing;
        break;
    }

    rects.push({
      id: layer.id,
      edge: layer.edge,
      x,
      y,
      width: scaled.width,
      height: scaled.height,
    });
  }

  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;

  for (const r of rects) {
    if (r.x < minX) minX = r.x;
    if (r.y < minY) minY = r.y;
    if (r.x + r.width > maxX) maxX = r.x + r.width;
    if (r.y + r.height > maxY) maxY = r.y + r.height;
  }

  const shifted = rects.map(r => ({
    ...r,
    x: r.x - minX,
    y: r.y - minY,
  }));

  return {
    rects: shifted,
    width: maxX - minX,
    height: maxY - minY,
  };
}

export function findPrimaryRect(rects: LayerRect[]): LayerRect {
  return rects.find(r => r.id === 'primary') ?? rects[0];
}
