import { describe, expect, it } from 'vitest';
import { exportAnnotationToSvg } from '@/renderer/components/editor/annotations/export-annotation';
import { exportAnnotationToSvg as exportFromBarrel } from '@/renderer/components/editor/annotations';
import {
  exportCircle,
  exportLine,
  exportRectangle,
} from '@/renderer/components/editor/shared/renderers';
import type {
  Annotation,
  CircleAnnotation,
  HighlightAnnotation,
  LineAnnotation,
  RectAnnotation,
} from '@/types/editor';

const exportProps = { offsetX: 10, offsetY: 20, scale: 2 };

const highlight: HighlightAnnotation = {
  id: 'h1',
  type: 'highlight',
  points: [0, 0, 40, 0, 80, 20],
  fill: '#FFFF00',
  opacity: 0.4,
  strokeWidth: 16,
};

const tooShortHighlight: HighlightAnnotation = {
  id: 'h2',
  type: 'highlight',
  points: [4, 6],
  fill: '#00FF00',
  opacity: 0.3,
  strokeWidth: 12,
};

const rectangle: RectAnnotation = {
  id: 'r1',
  type: 'rectangle',
  x: 8,
  y: 12,
  width: 40,
  height: 24,
  stroke: '#111111',
  strokeWidth: 2,
};

const circle: CircleAnnotation = {
  id: 'c1',
  type: 'circle',
  x: 4,
  y: 6,
  radius: 15,
  stroke: '#222222',
  strokeWidth: 3,
};

const line: LineAnnotation = {
  id: 'l1',
  type: 'line',
  points: [2, 3, 52, 3],
  stroke: '#333333',
  strokeWidth: 4,
};

describe('exportAnnotationToSvg', () => {
  it('omits highlights so canvas multiply remains the only screenshot paint', () => {
    expect(exportAnnotationToSvg(highlight, exportProps)).toBe('');
    expect(exportAnnotationToSvg(tooShortHighlight, exportProps)).toBe('');
  });

  it('delegates each shape to the shipped shape exporter', () => {
    expect(exportAnnotationToSvg(rectangle, exportProps)).toBe(
      exportRectangle({ annotation: rectangle, ...exportProps })
    );
    expect(exportAnnotationToSvg(circle, exportProps)).toBe(
      exportCircle({ annotation: circle, ...exportProps })
    );
    expect(exportAnnotationToSvg(line, exportProps)).toBe(
      exportLine({ annotation: line, ...exportProps })
    );
  });

  it('yields nothing for an unknown annotation type', () => {
    const unknown = { id: 'x1', type: 'sticker' } as unknown as Annotation;

    expect(exportAnnotationToSvg(unknown, exportProps)).toBe('');
  });

  it('is reachable through the annotations barrel the overlay imports', () => {
    expect(exportFromBarrel).toBe(exportAnnotationToSvg);
  });
});
