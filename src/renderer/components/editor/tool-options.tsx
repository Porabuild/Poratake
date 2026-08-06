import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/renderer/components/ui/select';
import type {
  ArrowStyle,
  HighlightOpacity,
  NumberSize,
  NumberStyle,
  RedactIntensity,
  RedactStyle,
  ShapeFillMode,
  TextFontFamily,
  TextFontSize,
  ToolType,
} from '@/types/editor';
import ArrowOptions from './arrow/arrow-options';
import { HighlightOptions } from './highlight';
import NumberOptions from './number/number-options';
import TextOptions from './text/text-options';
import RedactOptions from './redact/redact-options';
import ShapeOptions from './shapes/shape-options';

interface ToolOptionsProps {
  activeTool: ToolType;
  strokeWidth: number;
  onStrokeWidthChange: (width: number) => void;
  arrowStyle?: ArrowStyle;
  onArrowStyleChange?: (style: ArrowStyle) => void;
  highlightOpacity?: HighlightOpacity;
  onHighlightOpacityChange?: (opacity: HighlightOpacity) => void;
  numberStyle?: NumberStyle;
  onNumberStyleChange?: (style: NumberStyle) => void;
  numberSize?: NumberSize;
  onNumberSizeChange?: (size: NumberSize) => void;
  numberStartValue?: number;
  onNumberStartValueChange?: (value: number) => void;
  textBackground?: boolean;
  onTextBackgroundChange?: (enabled: boolean) => void;
  textFontSize?: TextFontSize;
  onTextFontSizeChange?: (size: TextFontSize) => void;
  textFontFamily?: TextFontFamily;
  onTextFontFamilyChange?: (family: TextFontFamily) => void;
  redactStyle?: RedactStyle;
  onRedactStyleChange?: (style: RedactStyle) => void;
  redactIntensity?: RedactIntensity;
  onRedactIntensityChange?: (intensity: RedactIntensity) => void;
  shapeFillMode?: ShapeFillMode;
  onShapeFillModeChange?: (mode: ShapeFillMode) => void;
  selectedColor?: string;
}

const TOOLS_WITH_THICKNESS = ['pen', 'rectangle', 'circle', 'line', 'arrow'];
const THICKNESS_OPTIONS = [1, 3, 8, 13, 21];
const THICKNESS_OPTIONS_SIZE = [2, 4, 6, 8, 10];

export default function ToolOptions({
  activeTool,
  strokeWidth,
  onStrokeWidthChange,
  arrowStyle = 'standard',
  onArrowStyleChange,
  highlightOpacity = 0.4,
  onHighlightOpacityChange,
  numberStyle = 'numeric',
  onNumberStyleChange,
  numberSize = 'medium',
  onNumberSizeChange,
  numberStartValue = 1,
  onNumberStartValueChange,
  textBackground = true,
  onTextBackgroundChange,
  textFontSize = 20,
  onTextFontSizeChange,
  textFontFamily = 'sans',
  onTextFontFamilyChange,
  redactStyle = 'pixelate',
  onRedactStyleChange,
  redactIntensity = 5,
  onRedactIntensityChange,
  shapeFillMode = 'outline',
  onShapeFillModeChange,
  selectedColor = '#FF3B30',
}: ToolOptionsProps) {
  const showThickness = TOOLS_WITH_THICKNESS.includes(activeTool);
  const showArrowStyle = activeTool === 'arrow';
  const showHighlightOptions = activeTool === 'highlight';
  const showNumberOptions = activeTool === 'number';
  const showTextOptions = activeTool === 'text';
  const showRedactOptions = activeTool === 'redact';
  const showShapeOptions =
    activeTool === 'rectangle' || activeTool === 'circle';

  if (
    !showThickness &&
    !showArrowStyle &&
    !showHighlightOptions &&
    !showNumberOptions &&
    !showTextOptions &&
    !showRedactOptions &&
    !showShapeOptions
  ) {
    return null;
  }

  return (
    <>
      {showThickness && (
        <>
          <Select
            value={String(strokeWidth)}
            onValueChange={value => onStrokeWidthChange(Number(value))}
          >
            <SelectTrigger size="sm" className="h-7!">
              <SelectValue>
                <div
                  className="bg-muted-foreground rounded-full"
                  style={{
                    width: '16px',
                    height: `${THICKNESS_OPTIONS_SIZE[THICKNESS_OPTIONS.indexOf(strokeWidth)]}px`,
                  }}
                />
              </SelectValue>
            </SelectTrigger>
            <SelectContent align="center">
              {THICKNESS_OPTIONS.map((thickness, index) => (
                <SelectItem key={thickness} value={String(thickness)}>
                  <div className="flex h-4 items-center">
                    <div
                      className="bg-muted-foreground rounded-full"
                      style={{
                        width: '32px',
                        height: `${THICKNESS_OPTIONS_SIZE[index]}px`,
                      }}
                    />
                  </div>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <div className="bg-border mx-1 h-[18px] w-px" />
        </>
      )}
      {showArrowStyle && onArrowStyleChange && (
        <ArrowOptions
          arrowStyle={arrowStyle}
          onArrowStyleChange={onArrowStyleChange}
        />
      )}
      {showHighlightOptions && onHighlightOpacityChange && (
        <HighlightOptions
          highlightOpacity={highlightOpacity}
          onHighlightOpacityChange={onHighlightOpacityChange}
        />
      )}
      {showNumberOptions &&
        onNumberStyleChange &&
        onNumberSizeChange &&
        onNumberStartValueChange && (
          <NumberOptions
            numberStyle={numberStyle}
            onNumberStyleChange={onNumberStyleChange}
            numberSize={numberSize}
            onNumberSizeChange={onNumberSizeChange}
            numberStartValue={numberStartValue}
            onNumberStartValueChange={onNumberStartValueChange}
          />
        )}
      {showTextOptions &&
        onTextBackgroundChange &&
        onTextFontSizeChange &&
        onTextFontFamilyChange && (
          <TextOptions
            textFontSize={textFontSize}
            onTextFontSizeChange={onTextFontSizeChange}
            textFontFamily={textFontFamily}
            onTextFontFamilyChange={onTextFontFamilyChange}
            textBackground={textBackground}
            onTextBackgroundChange={onTextBackgroundChange}
          />
        )}
      {showRedactOptions && onRedactStyleChange && onRedactIntensityChange && (
        <RedactOptions
          redactStyle={redactStyle}
          onRedactStyleChange={onRedactStyleChange}
          redactIntensity={redactIntensity}
          onRedactIntensityChange={onRedactIntensityChange}
        />
      )}
      {showShapeOptions && onShapeFillModeChange && (
        <ShapeOptions
          shapeFillMode={shapeFillMode}
          onShapeFillModeChange={onShapeFillModeChange}
          color={selectedColor}
        />
      )}
    </>
  );
}
