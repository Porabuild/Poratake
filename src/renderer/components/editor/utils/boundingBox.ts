import type { Annotation } from '@/types/editor';

export interface BoundingBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function getBoundingBox(annotation: Annotation): BoundingBox {
  switch (annotation.type) {
    case 'pen':
    case 'highlight': {
      if (annotation.points.length < 2) {
        return { x: 0, y: 0, width: 0, height: 0 };
      }
      let minX = Infinity;
      let minY = Infinity;
      let maxX = -Infinity;
      let maxY = -Infinity;
      for (let i = 0; i < annotation.points.length; i += 2) {
        const x = annotation.points[i];
        const y = annotation.points[i + 1];
        minX = Math.min(minX, x);
        minY = Math.min(minY, y);
        maxX = Math.max(maxX, x);
        maxY = Math.max(maxY, y);
      }
      return {
        x: minX,
        y: minY,
        width: maxX - minX,
        height: maxY - minY,
      };
    }

    case 'rectangle':
    case 'redact': {
      const w = annotation.width;
      const h = annotation.height;
      return {
        x: w < 0 ? annotation.x + w : annotation.x,
        y: h < 0 ? annotation.y + h : annotation.y,
        width: Math.abs(w),
        height: Math.abs(h),
      };
    }

    case 'circle': {
      return {
        x: annotation.x - annotation.radius,
        y: annotation.y - annotation.radius,
        width: annotation.radius * 2,
        height: annotation.radius * 2,
      };
    }

    case 'line':
    case 'arrow': {
      const [x1, y1, x2, y2] = annotation.points;
      return {
        x: Math.min(x1, x2),
        y: Math.min(y1, y2),
        width: Math.abs(x2 - x1),
        height: Math.abs(y2 - y1),
      };
    }

    case 'text': {
      const estimatedWidth = annotation.text.length * annotation.fontSize * 0.6;
      const estimatedHeight = annotation.fontSize * 1.2;
      return {
        x: annotation.x,
        y: annotation.y,
        width: estimatedWidth,
        height: estimatedHeight,
      };
    }

    case 'number': {
      const sizes = { small: 24, medium: 32, large: 40 };
      const size = sizes[annotation.size] || 32;
      return {
        x: annotation.x - size / 2,
        y: annotation.y - size / 2,
        width: size,
        height: size,
      };
    }

    default:
      return { x: 0, y: 0, width: 0, height: 0 };
  }
}

export function boxesIntersect(a: BoundingBox, b: BoundingBox): boolean {
  return (
    a.x < b.x + b.width &&
    a.x + a.width > b.x &&
    a.y < b.y + b.height &&
    a.y + a.height > b.y
  );
}

export function boxContains(container: BoundingBox, box: BoundingBox): boolean {
  return (
    box.x >= container.x &&
    box.y >= container.y &&
    box.x + box.width <= container.x + container.width &&
    box.y + box.height <= container.y + container.height
  );
}

export function getCombinedBoundingBox(annotations: Annotation[]): BoundingBox {
  if (annotations.length === 0) {
    return { x: 0, y: 0, width: 0, height: 0 };
  }

  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;

  for (const annotation of annotations) {
    const box = getBoundingBox(annotation);
    minX = Math.min(minX, box.x);
    minY = Math.min(minY, box.y);
    maxX = Math.max(maxX, box.x + box.width);
    maxY = Math.max(maxY, box.y + box.height);
  }

  return {
    x: minX,
    y: minY,
    width: maxX - minX,
    height: maxY - minY,
  };
}
