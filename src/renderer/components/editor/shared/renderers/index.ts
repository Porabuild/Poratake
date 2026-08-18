import type { JSX } from 'react';
import type {
  RectAnnotation,
  CircleAnnotation,
  LineAnnotation,
} from '@/types/editor';
import { createShapeRenderer } from './base-renderer';
import { rectangleConfig } from './rectangle-config';
import { circleConfig } from './circle-config';
import { lineConfig } from './line-config';
import type {
  ShapeRenderProps,
  ShapeHandleProps,
  ShapeExportProps,
} from './types';

const rectangleRenderer = createShapeRenderer(rectangleConfig);
const circleRenderer = createShapeRenderer(circleConfig);
const lineRenderer = createShapeRenderer(lineConfig);

export function renderRectangle(
  props: ShapeRenderProps<RectAnnotation>
): JSX.Element {
  return rectangleRenderer.render(props);
}

export function renderRectangleHandles(
  props: ShapeHandleProps<RectAnnotation>
): JSX.Element {
  return rectangleRenderer.renderHandles(props);
}

export function exportRectangle(
  props: ShapeExportProps<RectAnnotation>
): string {
  return rectangleRenderer.exportShape(props);
}

export function renderCircle(
  props: ShapeRenderProps<CircleAnnotation>
): JSX.Element {
  return circleRenderer.render(props);
}

export function renderCircleHandles(
  props: ShapeHandleProps<CircleAnnotation>
): JSX.Element {
  return circleRenderer.renderHandles(props);
}

export function exportCircle(
  props: ShapeExportProps<CircleAnnotation>
): string {
  return circleRenderer.exportShape(props);
}

export function renderLine(
  props: ShapeRenderProps<LineAnnotation>
): JSX.Element {
  return lineRenderer.render(props);
}

export function renderLineHandles(
  props: ShapeHandleProps<LineAnnotation>
): JSX.Element {
  return lineRenderer.renderHandles(props);
}

export function exportLine(props: ShapeExportProps<LineAnnotation>): string {
  return lineRenderer.exportShape(props);
}

export type {
  ShapeRenderProps,
  ShapeHandleProps,
  ShapeExportProps,
} from './types';
