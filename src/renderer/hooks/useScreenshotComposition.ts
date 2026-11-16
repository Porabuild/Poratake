import { useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { Annotation } from '@/types/editor';

export type ScreenshotCompositionRenderer = (
  annotations?: Annotation[]
) => Promise<HTMLCanvasElement>;

interface CompositionRequest {
  render: () => Promise<HTMLCanvasElement>;
  apply: (canvas: HTMLCanvasElement) => void;
  fail: (error: unknown) => void;
}

export function useScreenshotComposition({
  image,
  annotations,
  currentAnnotation,
  editingTextId,
  renderToCanvas,
}: {
  image: HTMLImageElement | null;
  annotations: Annotation[];
  currentAnnotation: Annotation | null;
  editingTextId: string | null;
  renderToCanvas: ScreenshotCompositionRenderer;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [completed, setCompleted] = useState<{
    image: HTMLImageElement;
    annotations: Annotation[];
    render: ScreenshotCompositionRenderer;
    failed: boolean;
  } | null>(null);
  const queue = useRef<{
    running: boolean;
    pending: CompositionRequest | null;
  }>({
    running: false,
    pending: null,
  });
  const previewAnnotations = useMemo(() => {
    const visible = editingTextId
      ? annotations.filter(annotation => annotation.id !== editingTextId)
      : annotations;
    return currentAnnotation ? [...visible, currentAnnotation] : visible;
  }, [annotations, currentAnnotation, editingTextId]);
  const isCurrent =
    completed?.image === image &&
    completed.annotations === previewAnnotations &&
    completed.render === renderToCanvas;

  useLayoutEffect(() => {
    if (!image) return;
    let active = true;
    const state = queue.current;
    const request: CompositionRequest = {
      render: () => renderToCanvas(previewAnnotations),
      apply: composition => {
        const canvas = canvasRef.current;
        if (!active || !canvas) return;
        canvas.width = composition.width;
        canvas.height = composition.height;
        const context = canvas.getContext('2d');
        if (!context) throw new Error('Failed to get preview canvas context');
        context.drawImage(composition, 0, 0);
        setCompleted({
          image,
          annotations: previewAnnotations,
          render: renderToCanvas,
          failed: false,
        });
      },
      fail: error => {
        if (!active) return;
        console.error('Failed to render screenshot preview:', error);
        setCompleted({
          image,
          annotations: previewAnnotations,
          render: renderToCanvas,
          failed: true,
        });
      },
    };
    state.pending = request;
    if (!state.running) {
      state.running = true;
      void (async () => {
        while (state.pending) {
          const next = state.pending;
          state.pending = null;
          try {
            const composition = await next.render();
            if (!state.pending) next.apply(composition);
          } catch (error) {
            next.fail(error);
          }
        }
        state.running = false;
      })();
    }
    return () => {
      active = false;
      if (state.pending === request) state.pending = null;
    };
  }, [image, previewAnnotations, renderToCanvas]);

  return {
    canvasRef,
    hasPreview: completed?.image === image && !completed?.failed,
    ready: isCurrent && !completed.failed,
    failed: isCurrent && completed.failed,
  };
}
