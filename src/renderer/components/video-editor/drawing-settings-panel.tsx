import { useCallback, useEffect, useMemo, useRef } from 'react';
import {
  ArrowUpRight,
  Circle,
  Eraser,
  Hash,
  Highlighter,
  Minus,
  MousePointer2,
  PenLine,
  Square,
  Trash2,
  Type,
} from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import { Label } from '@/renderer/components/ui/label';
import { Slider } from '@/renderer/components/ui/slider';
import { Textarea } from '@/renderer/components/ui/textarea';
import ColorPicker from '@/renderer/components/editor/color-picker';
import ArrowOptions from '@/renderer/components/editor/arrow/arrow-options';
import HighlightOptions from '@/renderer/components/editor/highlight/highlight-options';
import ShapeOptions from '@/renderer/components/editor/shapes/shape-options';
import NumberOptions from '@/renderer/components/editor/number/number-options';
import TextOptions from '@/renderer/components/editor/text/text-options';
import RedactOptions from '@/renderer/components/editor/redact/redact-options';
import { getFontFamilyCSS } from '@/renderer/components/editor/text/text-utils';
import type { Annotation } from '@/types/editor';
import type {
  DrawingSegment,
  DrawingToolSettings,
  VideoDrawingTool,
} from '@/types/drawing';
import { SettingsPanelHeader } from './components';

interface DrawingSettingsPanelProps {
  drawingSegments: DrawingSegment[];
  selectedDrawingId: string | null;
  toolSettings: DrawingToolSettings;
  textFocusNonce: number;
  onToolSettingsChange: (settings: DrawingToolSettings) => void;
  onUpdateDrawingAnnotation: (id: string, updates: Partial<Annotation>) => void;
  onDeleteDrawingSegment: (id: string) => void;
}

const TOOL_ITEMS: {
  id: VideoDrawingTool;
  label: string;
  icon: typeof PenLine;
}[] = [
  { id: 'select', label: 'Select', icon: MousePointer2 },
  { id: 'pen', label: 'Pen', icon: PenLine },
  { id: 'highlight', label: 'Highlight', icon: Highlighter },
  { id: 'rectangle', label: 'Rectangle', icon: Square },
  { id: 'circle', label: 'Circle', icon: Circle },
  { id: 'line', label: 'Line', icon: Minus },
  { id: 'arrow', label: 'Arrow', icon: ArrowUpRight },
  { id: 'text', label: 'Text', icon: Type },
  { id: 'number', label: 'Number', icon: Hash },
  { id: 'redact', label: 'Redact', icon: Eraser },
];

function getColorUpdates(
  annotation: Annotation,
  color: string
): Partial<Annotation> {
  switch (annotation.type) {
    case 'pen':
    case 'line':
    case 'arrow':
      return { stroke: color } as Partial<Annotation>;
    case 'rectangle':
    case 'circle':
      return {
        stroke: color,
        fill: annotation.fill ? color : undefined,
      } as Partial<Annotation>;
    case 'text':
    case 'number':
      return { fill: color } as Partial<Annotation>;
    default:
      return {};
  }
}

function getAnnotationColor(
  annotation: Annotation | undefined,
  toolSettings: DrawingToolSettings
): string {
  if (!annotation) {
    return toolSettings.activeTool === 'highlight'
      ? toolSettings.highlightColor
      : toolSettings.selectedColor;
  }

  switch (annotation.type) {
    case 'pen':
    case 'rectangle':
    case 'circle':
    case 'line':
    case 'arrow':
      return annotation.stroke;
    case 'text':
    case 'number':
    case 'highlight':
      return annotation.fill;
    default:
      return toolSettings.selectedColor;
  }
}

function supportsStrokeWidth(annotation: Annotation | undefined): boolean {
  return (
    annotation?.type === 'pen' ||
    annotation?.type === 'highlight' ||
    annotation?.type === 'rectangle' ||
    annotation?.type === 'circle' ||
    annotation?.type === 'line' ||
    annotation?.type === 'arrow'
  );
}

function isShape(annotation: Annotation | undefined): boolean {
  return annotation?.type === 'rectangle' || annotation?.type === 'circle';
}

function SettingRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-2">
      <Label className="text-sm text-muted-foreground">{label}</Label>
      <div className="flex items-center gap-2 [&>div.w-px]:hidden">
        {children}
      </div>
    </div>
  );
}

export default function DrawingSettingsPanel({
  drawingSegments,
  selectedDrawingId,
  toolSettings,
  textFocusNonce,
  onToolSettingsChange,
  onUpdateDrawingAnnotation,
  onDeleteDrawingSegment,
}: DrawingSettingsPanelProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const selectedDrawing = useMemo(
    () => drawingSegments.find(segment => segment.id === selectedDrawingId),
    [drawingSegments, selectedDrawingId]
  );
  const selectedAnnotation = selectedDrawing?.annotations[0];

  useEffect(() => {
    if (textFocusNonce === 0) return;
    if (selectedAnnotation?.type !== 'text') return;

    const textarea = textareaRef.current;
    if (!textarea) return;

    textarea.focus();
    textarea.select();
  }, [textFocusNonce, selectedAnnotation?.type]);

  const configType: VideoDrawingTool =
    selectedAnnotation?.type ?? toolSettings.activeTool;

  const displayColor = getAnnotationColor(selectedAnnotation, toolSettings);
  const displayStrokeWidth =
    selectedAnnotation && 'strokeWidth' in selectedAnnotation
      ? selectedAnnotation.strokeWidth
      : toolSettings.strokeWidth;

  const updateSettings = useCallback(
    (updates: Partial<DrawingToolSettings>) => {
      onToolSettingsChange({ ...toolSettings, ...updates });
    },
    [onToolSettingsChange, toolSettings]
  );

  const applyToSelected = useCallback(
    (updates: Partial<Annotation>) => {
      if (!selectedDrawingId || !selectedAnnotation) return;
      onUpdateDrawingAnnotation(selectedDrawingId, updates);
    },
    [onUpdateDrawingAnnotation, selectedAnnotation, selectedDrawingId]
  );

  const handleColorChange = useCallback(
    (color: string) => {
      updateSettings({ selectedColor: color });
      if (selectedAnnotation) {
        applyToSelected(getColorUpdates(selectedAnnotation, color));
      }
    },
    [applyToSelected, selectedAnnotation, updateSettings]
  );

  const handleStrokeWidthChange = useCallback(
    (value: number) => {
      updateSettings({ strokeWidth: value });
      if (supportsStrokeWidth(selectedAnnotation)) {
        applyToSelected({ strokeWidth: value } as Partial<Annotation>);
      }
    },
    [applyToSelected, selectedAnnotation, updateSettings]
  );

  const handleShapeFillModeChange = useCallback(
    (mode: DrawingToolSettings['shapeFillMode']) => {
      updateSettings({ shapeFillMode: mode });
      if (!isShape(selectedAnnotation)) return;

      const fillColor =
        selectedAnnotation && 'stroke' in selectedAnnotation
          ? selectedAnnotation.stroke
          : toolSettings.selectedColor;

      applyToSelected({
        fill: mode === 'filled' ? fillColor : undefined,
      } as Partial<Annotation>);
    },
    [
      applyToSelected,
      selectedAnnotation,
      toolSettings.selectedColor,
      updateSettings,
    ]
  );

  const handleDelete = useCallback(() => {
    if (!selectedDrawingId) return;
    onDeleteDrawingSegment(selectedDrawingId);
  }, [onDeleteDrawingSegment, selectedDrawingId]);

  return (
    <div className="space-y-4 p-4">
      <SettingsPanelHeader
        title="Drawing"
        description="Draw annotations directly on the video preview"
      />

      <div className="grid grid-cols-5 gap-1">
        {TOOL_ITEMS.map(item => {
          const Icon = item.icon;
          const isActive = toolSettings.activeTool === item.id;

          return (
            <Button
              key={item.id}
              type="button"
              variant={isActive ? 'tertiary' : 'ghost'}
              size="icon-xs"
              className="size-8!"
              onClick={() => updateSettings({ activeTool: item.id })}
              title={item.label}
            >
              <Icon className="size-4" />
            </Button>
          );
        })}
      </div>

      <div className="space-y-3">
        <Label className="text-sm">Style</Label>

        <SettingRow label={configType === 'highlight' ? 'Highlight' : 'Color'}>
          <ColorPicker
            selectedColor={displayColor}
            onColorChange={color => {
              if (configType === 'highlight') {
                updateSettings({
                  highlightColor: color as typeof toolSettings.highlightColor,
                });
                if (selectedAnnotation?.type === 'highlight') {
                  applyToSelected({ fill: color } as Partial<Annotation>);
                }
                return;
              }

              handleColorChange(color);
            }}
            activeTool={configType}
            highlightOpacity={
              selectedAnnotation?.type === 'highlight'
                ? selectedAnnotation.opacity
                : toolSettings.highlightOpacity
            }
          />
        </SettingRow>

        {(configType === 'pen' ||
          configType === 'rectangle' ||
          configType === 'circle' ||
          configType === 'line' ||
          configType === 'arrow') && (
          <SettingRow label="Thickness">
            <Slider
              size="sm"
              className="w-32"
              value={[displayStrokeWidth]}
              min={1}
              max={16}
              step={1}
              onValueChange={([value]) => handleStrokeWidthChange(value)}
            />
            <span className="w-5 text-right text-xs text-muted-foreground">
              {displayStrokeWidth}
            </span>
          </SettingRow>
        )}

        {configType === 'arrow' && (
          <SettingRow label="Arrow">
            <ArrowOptions
              arrowStyle={
                selectedAnnotation?.type === 'arrow'
                  ? (selectedAnnotation.arrowStyle ?? toolSettings.arrowStyle)
                  : toolSettings.arrowStyle
              }
              onArrowStyleChange={arrowStyle => {
                updateSettings({ arrowStyle });
                if (selectedAnnotation?.type === 'arrow') {
                  applyToSelected({ arrowStyle } as Partial<Annotation>);
                }
              }}
            />
          </SettingRow>
        )}

        {configType === 'highlight' && (
          <SettingRow label="Opacity">
            <HighlightOptions
              highlightOpacity={
                selectedAnnotation?.type === 'highlight'
                  ? selectedAnnotation.opacity
                  : toolSettings.highlightOpacity
              }
              onHighlightOpacityChange={highlightOpacity => {
                updateSettings({ highlightOpacity });
                if (selectedAnnotation?.type === 'highlight') {
                  applyToSelected({
                    opacity: highlightOpacity,
                  } as Partial<Annotation>);
                }
              }}
            />
          </SettingRow>
        )}

        {(configType === 'rectangle' || configType === 'circle') && (
          <SettingRow label="Fill">
            <ShapeOptions
              shapeFillMode={
                isShape(selectedAnnotation)
                  ? (selectedAnnotation as { fill?: string }).fill
                    ? 'filled'
                    : 'outline'
                  : toolSettings.shapeFillMode
              }
              onShapeFillModeChange={handleShapeFillModeChange}
              color={displayColor}
            />
          </SettingRow>
        )}

        {configType === 'number' && (
          <SettingRow label="Number">
            <NumberOptions
              numberStyle={toolSettings.numberStyle}
              onNumberStyleChange={numberStyle =>
                updateSettings({ numberStyle })
              }
              numberSize={
                selectedAnnotation?.type === 'number'
                  ? selectedAnnotation.size
                  : toolSettings.numberSize
              }
              onNumberSizeChange={numberSize => {
                updateSettings({ numberSize });
                if (selectedAnnotation?.type === 'number') {
                  applyToSelected({ size: numberSize } as Partial<Annotation>);
                }
              }}
              numberStartValue={toolSettings.numberStartValue}
              onNumberStartValueChange={numberStartValue =>
                updateSettings({ numberStartValue })
              }
            />
          </SettingRow>
        )}

        {configType === 'text' && (
          <SettingRow label="Text">
            <TextOptions
              textFontSize={
                selectedAnnotation?.type === 'text'
                  ? (selectedAnnotation.fontSize as typeof toolSettings.textFontSize)
                  : toolSettings.textFontSize
              }
              onTextFontSizeChange={textFontSize => {
                updateSettings({ textFontSize });
                if (selectedAnnotation?.type === 'text') {
                  applyToSelected({
                    fontSize: textFontSize,
                  } as Partial<Annotation>);
                }
              }}
              textFontFamily={toolSettings.textFontFamily}
              onTextFontFamilyChange={textFontFamily => {
                updateSettings({ textFontFamily });
                if (selectedAnnotation?.type === 'text') {
                  applyToSelected({
                    fontFamily: getFontFamilyCSS(textFontFamily),
                  } as Partial<Annotation>);
                }
              }}
              textBackground={
                selectedAnnotation?.type === 'text'
                  ? !!selectedAnnotation.backgroundColor
                  : toolSettings.textBackground
              }
              onTextBackgroundChange={textBackground => {
                updateSettings({ textBackground });
                if (selectedAnnotation?.type === 'text') {
                  applyToSelected({
                    backgroundColor: textBackground
                      ? 'rgba(0, 0, 0, 0.75)'
                      : undefined,
                    backgroundPadding: textBackground
                      ? { x: 8, y: 4 }
                      : undefined,
                    backgroundRadius: textBackground ? 4 : undefined,
                  } as Partial<Annotation>);
                }
              }}
            />
          </SettingRow>
        )}

        {configType === 'redact' && (
          <SettingRow label="Redact">
            <RedactOptions
              redactStyle={
                selectedAnnotation?.type === 'redact'
                  ? selectedAnnotation.style
                  : toolSettings.redactStyle
              }
              onRedactStyleChange={redactStyle => {
                updateSettings({ redactStyle });
                if (selectedAnnotation?.type === 'redact') {
                  applyToSelected({
                    style: redactStyle,
                  } as Partial<Annotation>);
                }
              }}
              redactIntensity={
                selectedAnnotation?.type === 'redact'
                  ? selectedAnnotation.intensity
                  : toolSettings.redactIntensity
              }
              onRedactIntensityChange={redactIntensity => {
                updateSettings({ redactIntensity });
                if (selectedAnnotation?.type === 'redact') {
                  applyToSelected({
                    intensity: redactIntensity,
                  } as Partial<Annotation>);
                }
              }}
            />
          </SettingRow>
        )}
      </div>

      {selectedDrawing && selectedAnnotation ? (
        <div className="space-y-3 border-t border-border pt-4">
          <div className="flex items-center justify-between">
            <Label className="text-sm">Selected Drawing</Label>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              className="text-destructive hover:text-destructive"
              onClick={handleDelete}
            >
              <Trash2 className="size-4" />
            </Button>
          </div>

          {selectedAnnotation.type === 'text' && (
            <Textarea
              ref={textareaRef}
              value={selectedAnnotation.text}
              onChange={event =>
                applyToSelected({
                  text: event.target.value,
                } as Partial<Annotation>)
              }
              className="min-h-20 resize-none"
            />
          )}
        </div>
      ) : (
        <p className="border-t border-border pt-4 text-sm text-muted-foreground">
          Select a drawing segment on the timeline to edit it.
        </p>
      )}
    </div>
  );
}
