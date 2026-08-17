import type { Annotation } from '@/types/editor';
import { exportArrow } from './arrow-renderer';
import { exportNumber } from './number-renderer';
import { exportPen } from './pen-renderer';
import { exportText } from './text-renderer';
import { exportRedact } from '../redact';
import { exportCircle, exportLine, exportRectangle } from '../shared/renderers';

export function exportAnnotationToSvg(
  annotation: Annotation,
  props: { offsetX: number; offsetY: number; scale: number }
): string {
  switch (annotation.type) {
    case 'pen':
      return exportPen({ annotation, ...props });
    case 'highlight':
      return '';
    case 'rectangle':
      return exportRectangle({ annotation, ...props });
    case 'circle':
      return exportCircle({ annotation, ...props });
    case 'line':
      return exportLine({ annotation, ...props });
    case 'arrow':
      return exportArrow({ annotation, ...props });
    case 'text':
      return exportText({ annotation, ...props });
    case 'number':
      return exportNumber({ annotation, ...props });
    case 'redact':
      return exportRedact({ annotation, ...props });
    default:
      return '';
  }
}
