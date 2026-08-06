import type {
  Annotation,
  ArrowStyle,
  HighlightColor,
  HighlightOpacity,
  NumberSize,
  NumberStyle,
  RedactIntensity,
  RedactStyle,
  ShapeFillMode,
  TextFontFamily,
  TextFontSize,
  ToolType,
} from './editor';

export type VideoDrawingTool = Extract<
  ToolType,
  | 'select'
  | 'pen'
  | 'highlight'
  | 'rectangle'
  | 'circle'
  | 'line'
  | 'arrow'
  | 'text'
  | 'number'
  | 'redact'
>;

export interface DrawingSegment {
  id: string;
  startTime: number;
  endTime: number;
  canvasWidth: number;
  canvasHeight: number;
  annotations: Annotation[];
}

export interface DrawingToolSettings {
  activeTool: VideoDrawingTool;
  selectedColor: string;
  strokeWidth: number;
  arrowStyle: ArrowStyle;
  highlightColor: HighlightColor;
  highlightOpacity: HighlightOpacity;
  numberStyle: NumberStyle;
  numberSize: NumberSize;
  numberStartValue: number;
  textBackground: boolean;
  textFontSize: TextFontSize;
  textFontFamily: TextFontFamily;
  redactStyle: RedactStyle;
  redactIntensity: RedactIntensity;
  shapeFillMode: ShapeFillMode;
}

export const DEFAULT_DRAWING_TOOL_SETTINGS: DrawingToolSettings = {
  activeTool: 'select',
  selectedColor: '#FF3B30',
  strokeWidth: 4,
  arrowStyle: 'standard',
  highlightColor: '#FFFF00',
  highlightOpacity: 0.4,
  numberStyle: 'numeric',
  numberSize: 'medium',
  numberStartValue: 1,
  textBackground: true,
  textFontSize: 24,
  textFontFamily: 'sans',
  redactStyle: 'pixelate',
  redactIntensity: 5,
  shapeFillMode: 'outline',
};

export const DEFAULT_DRAWING_SEGMENT_DURATION = 3;
export const MIN_DRAWING_SEGMENT_DURATION = 0.3;
