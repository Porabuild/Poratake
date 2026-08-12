import React, {
  useRef,
  useEffect,
  forwardRef,
  useCallback,
  useImperativeHandle,
} from 'react';
import type {
  Annotation,
  ArrowStyle,
  HighlightColor,
  HighlightOpacity,
  ImageLayer,
  NumberSize,
  NumberStyle,
  RedactIntensity,
  RedactStyle,
  ShapeFillMode,
  TextFontFamily,
  TextFontSize,
  ToolType,
  WallpaperSettings,
} from '@/types/editor';
import { TextEditInput } from '@/renderer/components/editor/text';
import CanvasRenderer, {
  type CanvasRendererHandle,
} from '@/renderer/components/editor/canvas-renderer';
import SvgAnnotationsOverlay, {
  type SvgAnnotationsOverlayHandle,
} from '@/renderer/components/editor/svg-annotations-overlay';
import SvgCropOverlay from '@/renderer/components/editor/svg-crop-overlay';
import RedactOverlay from '@/renderer/components/editor/redact/redact-overlay';
import { useTextEditing } from '@/renderer/hooks/useTextEditing';
import { useAnnotationSelection } from '@/renderer/hooks/useAnnotationSelection';
import { useCropTool } from '@/renderer/hooks/useCropTool';
import { useDrawingTools } from '@/renderer/hooks/useDrawingTools';
import { useKeyboardShortcuts } from '@/renderer/hooks/useKeyboardShortcuts';
import { useBrushCursor } from '@/renderer/hooks/useBrushCursor';
import { useContentDimensions } from '@/renderer/hooks/useContentDimensions';
import { getFontFamilyCSS } from '@/renderer/components/editor/text/text-utils';
import { getDisplayValue } from '@/renderer/components/editor/number/number-utils';

interface EditorCanvasProps {
  image: HTMLImageElement | null;
  annotations: Annotation[];
  activeTool: ToolType;
  selectedColor: string;
  strokeWidth: number;
  arrowStyle?: ArrowStyle;
  highlightColor?: HighlightColor;
  highlightOpacity?: HighlightOpacity;
  numberStyle?: NumberStyle;
  numberSize?: NumberSize;
  nextNumberValue?: number;
  onAnnotationAdd: (annotation: Annotation) => void;
  onAnnotationUpdate?: (id: string, updates: Partial<Annotation>) => void;
  onAnnotationsUpdateMultiple?: (
    updates: Array<{ id: string; updates: Partial<Annotation> }>
  ) => void;
  onAnnotationDragEnd?: () => void;
  onAnnotationDelete?: (id: string) => void;
  onAnnotationsDeleteMultiple?: (ids: string[]) => void;
  onCrop?: (
    cropData: { x: number; y: number; width: number; height: number },
    adjustedAnnotations: Annotation[]
  ) => void;
  onSelectionChange?: (selectedIds: string[]) => void;
  imageWidth?: number;
  imageHeight?: number;
  extraLayers?: ImageLayer[];
  extraLayerImages?: Record<string, HTMLImageElement>;
  wallpaper: WallpaperSettings;
  zoom?: number;
  textBackground?: boolean;
  textFontSize?: TextFontSize;
  textFontFamily?: TextFontFamily;
  redactStyle?: RedactStyle;
  redactIntensity?: RedactIntensity;
  shapeFillMode?: ShapeFillMode;
}

export interface EditorCanvasHandle {
  getCanvas: () => HTMLCanvasElement | null;
  getSvgForExport: (scale: number) => string;
  selectAnnotations: (ids: string[]) => void;
}

const EditorCanvas = forwardRef<EditorCanvasHandle, EditorCanvasProps>(
  (
    {
      image,
      annotations,
      activeTool,
      selectedColor,
      strokeWidth,
      arrowStyle = 'standard',
      highlightColor = '#FFFF00',
      highlightOpacity = 0.4,
      numberStyle = 'numeric',
      numberSize = 'medium',
      nextNumberValue = 1,
      onAnnotationAdd,
      onAnnotationUpdate,
      onAnnotationsUpdateMultiple,
      onAnnotationDragEnd,
      onAnnotationDelete,
      onAnnotationsDeleteMultiple,
      onCrop,
      onSelectionChange,
      imageWidth,
      imageHeight,
      extraLayers,
      extraLayerImages,
      wallpaper,
      zoom = 1,
      textBackground = true,
      textFontSize = 20,
      textFontFamily = 'sans',
      redactStyle = 'pixelate',
      redactIntensity = 5,
      shapeFillMode = 'outline',
    },
    ref
  ) => {
    const containerRef = useRef<HTMLDivElement>(null);
    const canvasRef = useRef<CanvasRendererHandle>(null);
    const svgRef = useRef<SvgAnnotationsOverlayHandle>(null);
    const interactionLayerRef = useRef<HTMLDivElement>(null);

    const imgWidth = imageWidth || image?.width || 0;
    const imgHeight = imageHeight || image?.height || 0;

    const contentDims = useContentDimensions({
      image,
      imageWidth: imgWidth,
      imageHeight: imgHeight,
      wallpaper,
      extraLayers,
      extraLayerImages,
    });

    const {
      editingTextId,
      textEditValue,
      textEditPosition,
      isTextEditingRef,
      startTextEditing,
      finishTextEditing,
      handleTextChange,
    } = useTextEditing({
      annotations,
      onAnnotationUpdate,
      onAnnotationDelete,
      selectedColor,
    });

    const {
      selectedAnnotationIds,
      selectAnnotation,
      selectMultiple,
      deselectAll,
    } = useAnnotationSelection();

    useImperativeHandle(
      ref,
      () => ({
        getCanvas: () => canvasRef.current?.getCanvas() ?? null,
        getSvgForExport: (scale: number) =>
          svgRef.current?.getSvgForExport(scale) ?? '',
        selectAnnotations: selectMultiple,
      }),
      [selectMultiple]
    );

    const { cropRect, startCrop, updateCrop, setCropRect } = useCropTool({
      annotations,
      onCrop,
      contentWidth: contentDims.contentWidth,
      contentHeight: contentDims.contentHeight,
    });

    const {
      isDrawing,
      currentAnnotation,
      startDrawing,
      updateAnnotation: updateDrawing,
      finishDrawing,
      setIsDrawing,
    } = useDrawingTools({
      activeTool,
      selectedColor,
      strokeWidth,
      arrowStyle,
      highlightColor,
      highlightOpacity,
      redactStyle,
      redactIntensity,
      shapeFillMode,
      onAnnotationAdd,
    });

    const brushCursor = useBrushCursor({
      activeTool,
      redactStyle,
      highlightColor,
    });

    useKeyboardShortcuts({
      selectedAnnotationIds,
      isTextEditing: isTextEditingRef.current,
      onDeleteMultiple: onAnnotationsDeleteMultiple,
      onDeselect: deselectAll,
      annotationIds: annotations.map(a => a.id),
      onSelectAll: selectMultiple,
    });

    useEffect(() => {
      onSelectionChange?.(selectedAnnotationIds);
    }, [selectedAnnotationIds, onSelectionChange]);

    const annotationOffsetX = contentDims.contentOffsetX;
    const annotationOffsetY = contentDims.contentOffsetY;

    const getScaledPosition = useCallback(
      (e: React.MouseEvent): { x: number; y: number } | null => {
        const container = interactionLayerRef.current;
        if (!container) return null;

        const rect = container.getBoundingClientRect();
        const x = (e.clientX - rect.left) / zoom - annotationOffsetX;
        const y = (e.clientY - rect.top) / zoom - annotationOffsetY;

        return { x, y };
      },
      [zoom, annotationOffsetX, annotationOffsetY]
    );

    const handleMouseDown = useCallback(
      (e: React.MouseEvent) => {
        if (isTextEditingRef.current && editingTextId) {
          finishTextEditing();
          return;
        }

        const pos = getScaledPosition(e);
        if (!pos) return;

        if (activeTool === 'select') {
          return;
        }

        if (!isDrawing) {
          deselectAll();
        }

        if (activeTool === 'crop') {
          startCrop(pos);
          setIsDrawing(true);
          return;
        }

        if (activeTool === 'text') {
          const textAnnotation = {
            id: `text-${Date.now()}`,
            type: 'text' as const,
            x: pos.x,
            y: pos.y,
            text: '',
            fontSize: textFontSize,
            fontFamily: getFontFamilyCSS(textFontFamily),
            fill: selectedColor,
            backgroundColor: textBackground ? 'rgba(0, 0, 0, 0.75)' : undefined,
            backgroundPadding: textBackground ? { x: 8, y: 4 } : undefined,
            backgroundRadius: textBackground ? 4 : undefined,
          };
          onAnnotationAdd(textAnnotation);
          startTextEditing(pos, textAnnotation.id, {
            x: annotationOffsetX,
            y: annotationOffsetY,
          });
          return;
        }

        if (activeTool === 'number') {
          const displayValue = getDisplayValue(nextNumberValue, numberStyle);
          const numberAnnotation = {
            id: `number-${Date.now()}`,
            type: 'number' as const,
            x: pos.x,
            y: pos.y,
            value: nextNumberValue,
            displayValue,
            fill: selectedColor,
            size: numberSize,
          };
          onAnnotationAdd(numberAnnotation);
          return;
        }

        startDrawing(pos);
      },
      [
        activeTool,
        isTextEditingRef,
        editingTextId,
        finishTextEditing,
        getScaledPosition,
        deselectAll,
        isDrawing,
        startCrop,
        setIsDrawing,
        selectedColor,
        onAnnotationAdd,
        startTextEditing,
        startDrawing,
        annotationOffsetX,
        annotationOffsetY,
        numberStyle,
        numberSize,
        nextNumberValue,
        textBackground,
        textFontSize,
        textFontFamily,
      ]
    );

    const handleMouseMove = useCallback(() => {}, []);

    const handleMouseUp = useCallback(() => {}, []);

    useEffect(() => {
      if (!isDrawing) return;

      const container = interactionLayerRef.current;
      if (!container) return;

      const handleWindowMouseMove = (e: MouseEvent) => {
        const rect = container.getBoundingClientRect();
        const x = (e.clientX - rect.left) / zoom - annotationOffsetX;
        const y = (e.clientY - rect.top) / zoom - annotationOffsetY;

        if (activeTool === 'crop') {
          updateCrop({ x, y });
          return;
        }

        updateDrawing({ x, y }, e.shiftKey);
      };

      const handleWindowMouseUp = () => {
        if (activeTool === 'crop') {
          setIsDrawing(false);
          return;
        }

        finishDrawing();
      };

      window.addEventListener('mousemove', handleWindowMouseMove);
      window.addEventListener('mouseup', handleWindowMouseUp);

      return () => {
        window.removeEventListener('mousemove', handleWindowMouseMove);
        window.removeEventListener('mouseup', handleWindowMouseUp);
      };
    }, [
      isDrawing,
      activeTool,
      zoom,
      annotationOffsetX,
      annotationOffsetY,
      updateCrop,
      updateDrawing,
      finishDrawing,
      setIsDrawing,
    ]);

    const handleTextDoubleClick = useCallback(
      (id: string) => {
        const textAnn = annotations.find(a => a.id === id && a.type === 'text');
        if (!textAnn || textAnn.type !== 'text') return;

        startTextEditing({ x: textAnn.x, y: textAnn.y }, id, {
          x: annotationOffsetX,
          y: annotationOffsetY,
        });
      },
      [annotations, startTextEditing, annotationOffsetX, annotationOffsetY]
    );

    if (!image) {
      return (
        <div className="flex h-full w-full items-center justify-center">
          <div>Loading...</div>
        </div>
      );
    }

    const {
      contentWidth,
      contentHeight,
      canvasWidth,
      canvasHeight,
      contentOffsetX,
      contentOffsetY,
      balanceCrop,
      nativeBalanceCrop,
      aspectRatioPaddingX,
      aspectRatioPaddingY,
      layerRects,
      primaryRect,
      primaryInsetColor,
      layerInsetColors,
    } = contentDims;

    const padding = wallpaper.padding;

    return (
      <div
        ref={containerRef}
        className="relative h-full w-full overflow-hidden"
      >
        <div className="relative inline-flex min-h-full min-w-full items-center justify-center">
          <div
            ref={interactionLayerRef}
            style={{
              position: 'relative',
              width: canvasWidth,
              height: canvasHeight,
              cursor: brushCursor,
            }}
            onMouseDown={handleMouseDown}
            onMouseMove={handleMouseMove}
            onMouseUp={handleMouseUp}
          >
            {}
            <CanvasRenderer
              ref={canvasRef}
              image={image}
              padding={padding}
              inset={wallpaper.inset}
              cornerRadius={wallpaper.corners}
              shadow={wallpaper.shadow}
              gradient={wallpaper.gradient}
              backgroundImage={wallpaper.backgroundImage}
              backgroundBlur={wallpaper.backgroundBlur ?? 0}
              noise={wallpaper.noise ?? 0}
              windowFrame={wallpaper.windowFrame?.style}
              aspectRatioPaddingX={aspectRatioPaddingX}
              aspectRatioPaddingY={aspectRatioPaddingY}
              extraLayers={extraLayers}
              extraLayerImages={extraLayerImages}
              layerRects={layerRects}
              primaryRect={primaryRect}
              nativeBalanceCrop={nativeBalanceCrop}
              primaryInsetColor={primaryInsetColor}
              layerInsetColors={layerInsetColors}
            />

            {}
            <RedactOverlay
              image={image}
              imageWidth={contentWidth}
              imageHeight={contentHeight}
              canvasWidth={canvasWidth}
              canvasHeight={canvasHeight}
              offsetX={contentOffsetX}
              offsetY={contentOffsetY}
              balanceCrop={balanceCrop}
              annotations={annotations}
              currentAnnotation={currentAnnotation}
            />

            {}
            <SvgAnnotationsOverlay
              ref={svgRef}
              annotations={annotations}
              currentAnnotation={currentAnnotation}
              width={canvasWidth}
              height={canvasHeight}
              offsetX={contentOffsetX}
              offsetY={contentOffsetY}
              selectedAnnotationIds={selectedAnnotationIds}
              onSelect={selectAnnotation}
              onSelectMultiple={selectMultiple}
              onAnnotationUpdate={onAnnotationUpdate}
              onAnnotationsUpdateMultiple={onAnnotationsUpdateMultiple}
              onDragEnd={onAnnotationDragEnd}
              onTextDoubleClick={handleTextDoubleClick}
              editingTextId={editingTextId}
              zoom={zoom}
              activeTool={activeTool}
            />

            {}
            {cropRect && activeTool === 'crop' && (
              <SvgCropOverlay
                cropRect={cropRect}
                canvasWidth={canvasWidth}
                canvasHeight={canvasHeight}
                onCropRectChange={setCropRect}
                offsetX={contentOffsetX}
                offsetY={contentOffsetY}
                imageWidth={contentWidth}
                imageHeight={contentHeight}
              />
            )}

            {}
            {editingTextId && textEditPosition && (
              <TextEditInput
                editingTextId={editingTextId}
                textEditValue={textEditValue}
                textEditPosition={textEditPosition}
                selectedColor={selectedColor}
                annotations={annotations}
                onTextEditChange={handleTextChange}
                onFinishEditing={finishTextEditing}
                textBackground={textBackground}
                textFontSize={textFontSize}
                textFontFamily={textFontFamily}
              />
            )}
          </div>
        </div>
      </div>
    );
  }
);

EditorCanvas.displayName = 'EditorCanvas';

export default EditorCanvas;
