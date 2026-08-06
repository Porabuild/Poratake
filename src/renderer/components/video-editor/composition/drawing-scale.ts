import type { Annotation } from '@/types/editor';

export function scaleAnnotationToComposition(
  annotation: Annotation,
  scaleX: number,
  scaleY: number
): Annotation {
  const scale = (scaleX + scaleY) / 2;

  switch (annotation.type) {
    case 'pen':
    case 'highlight':
      return {
        ...annotation,
        points: annotation.points.map((point, index) =>
          index % 2 === 0 ? point * scaleX : point * scaleY
        ),
        strokeWidth: annotation.strokeWidth * scale,
      };
    case 'rectangle':
    case 'redact':
      return {
        ...annotation,
        x: annotation.x * scaleX,
        y: annotation.y * scaleY,
        width: annotation.width * scaleX,
        height: annotation.height * scaleY,
      };
    case 'circle':
      return {
        ...annotation,
        x: annotation.x * scaleX,
        y: annotation.y * scaleY,
        radius: annotation.radius * scale,
      };
    case 'line':
    case 'arrow':
      return {
        ...annotation,
        points: annotation.points.map((point, index) =>
          index % 2 === 0 ? point * scaleX : point * scaleY
        ) as [number, number, number, number],
        strokeWidth: annotation.strokeWidth * scale,
        ...(annotation.type === 'arrow' && annotation.bendOffset
          ? {
              bendOffset: {
                x: annotation.bendOffset.x * scaleX,
                y: annotation.bendOffset.y * scaleY,
              },
            }
          : {}),
      };
    case 'text':
      return {
        ...annotation,
        x: annotation.x * scaleX,
        y: annotation.y * scaleY,
        fontSize: annotation.fontSize * scale,
        backgroundPadding: annotation.backgroundPadding
          ? {
              x: annotation.backgroundPadding.x * scale,
              y: annotation.backgroundPadding.y * scale,
            }
          : undefined,
        backgroundRadius:
          annotation.backgroundRadius === undefined
            ? undefined
            : annotation.backgroundRadius * scale,
      };
    case 'number':
      return {
        ...annotation,
        x: annotation.x * scaleX,
        y: annotation.y * scaleY,
      };
    default:
      return annotation;
  }
}

export function inverseScaleAnnotationUpdates(
  updates: Partial<Annotation>,
  scaleX: number,
  scaleY: number
): Partial<Annotation> {
  const scale = (scaleX + scaleY) / 2;
  const result: Record<string, unknown> = { ...updates };
  const source = updates as Record<string, unknown>;

  if (typeof source.x === 'number') result.x = source.x / scaleX;
  if (typeof source.y === 'number') result.y = source.y / scaleY;
  if (typeof source.width === 'number') result.width = source.width / scaleX;
  if (typeof source.height === 'number') result.height = source.height / scaleY;
  if (typeof source.radius === 'number') result.radius = source.radius / scale;
  if (typeof source.fontSize === 'number') {
    result.fontSize = source.fontSize / scale;
  }
  if (typeof source.strokeWidth === 'number') {
    result.strokeWidth = source.strokeWidth / scale;
  }
  if (Array.isArray(source.points)) {
    result.points = source.points.map((point, index) =>
      index % 2 === 0 ? point / scaleX : point / scaleY
    );
  }
  if (
    source.bendOffset &&
    typeof source.bendOffset === 'object' &&
    'x' in source.bendOffset &&
    'y' in source.bendOffset
  ) {
    const bend = source.bendOffset as { x: number; y: number };
    result.bendOffset = { x: bend.x / scaleX, y: bend.y / scaleY };
  }

  return result as Partial<Annotation>;
}
