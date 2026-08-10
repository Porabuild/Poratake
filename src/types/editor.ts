export type ToolType =
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
  | 'crop'
  | 'wallpaper';

export type ArrowStyle = 'standard' | 'curved' | 'double' | 'double-curved';

export type NumberStyle = 'numeric' | 'alpha-upper' | 'alpha-lower' | 'roman';

export type NumberSize = 'small' | 'medium' | 'large';

export type RedactStyle = 'pixelate' | 'blur' | 'blackout';

export type RedactIntensity = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10;

export type ShapeFillMode = 'outline' | 'filled';

export type HighlightOpacity = 0.2 | 0.3 | 0.4 | 0.5 | 0.6;

export const HIGHLIGHT_COLORS = [
  '#FFFF00',
  '#00FF00',
  '#FF69B4',
  '#00BFFF',
  '#FFA500',
] as const;

export type HighlightColor = string;

export type TextFontSize = number;

export type WindowFrameStyle =
  'none' | 'macos-light' | 'macos-dark' | 'windows-light' | 'windows-dark';

export interface WindowFrameSettings {
  style: WindowFrameStyle;
}

export type TextFontFamily = string;

export const ASPECT_RATIO_OPTIONS = [
  'auto',
  '1:1',
  '4:3',
  '3:2',
  '16:9',
  '16:10',
  '21:9',
  '9:16',
  '3:4',
  '2:3',
] as const;

export type AspectRatioOption = (typeof ASPECT_RATIO_OPTIONS)[number];

export interface GradientOption {
  id: string;
  colors: string[];
  angle: number;
}

export interface WallpaperSettings {
  gradient: GradientOption | null;
  backgroundImage: string | null;
  backgroundBlur: number;
  noise: number;
  padding: number;
  inset: number;
  corners: number;
  shadow: number;
  spacing: number;
  windowFrame: WindowFrameSettings;
  balance: boolean;
  aspectRatio: AspectRatioOption;
}

export type ImageLayerEdge = 'self' | 'left' | 'right' | 'top' | 'bottom';

export interface ImageLayer {
  id: string;
  base64: string;
  naturalWidth: number;
  naturalHeight: number;
  edge: ImageLayerEdge;
}

export interface BaseAnnotation {
  id: string;
  type: ToolType;
}

export interface PenAnnotation extends BaseAnnotation {
  type: 'pen';
  points: number[];
  stroke: string;
  strokeWidth: number;
}

export interface RectAnnotation extends BaseAnnotation {
  type: 'rectangle';
  x: number;
  y: number;
  width: number;
  height: number;
  stroke: string;
  strokeWidth: number;
  fill?: string;
}

export interface CircleAnnotation extends BaseAnnotation {
  type: 'circle';
  x: number;
  y: number;
  radius: number;
  stroke: string;
  strokeWidth: number;
  fill?: string;
}

export interface LineAnnotation extends BaseAnnotation {
  type: 'line';
  points: [number, number, number, number];
  stroke: string;
  strokeWidth: number;
}

export interface ArrowAnnotation extends BaseAnnotation {
  type: 'arrow';
  points: [number, number, number, number];
  stroke: string;
  strokeWidth: number;
  arrowStyle?: ArrowStyle;
  bendOffset?: { x: number; y: number };
}

export interface TextAnnotation extends BaseAnnotation {
  type: 'text';
  x: number;
  y: number;
  text: string;
  fontSize: number;
  fill: string;
  fontFamily?: string;
  backgroundColor?: string;
  backgroundOpacity?: number;
  backgroundPadding?: { x: number; y: number };
  backgroundRadius?: number;
  rotation?: number;
}

export interface NumberAnnotation extends BaseAnnotation {
  type: 'number';
  x: number;
  y: number;
  value: number;
  displayValue: string;
  fill: string;
  size: NumberSize;
}

export interface RedactAnnotation extends BaseAnnotation {
  type: 'redact';
  x: number;
  y: number;
  width: number;
  height: number;
  style: RedactStyle;
  intensity: RedactIntensity;
}

export interface HighlightAnnotation extends BaseAnnotation {
  type: 'highlight';
  points: number[];
  fill: HighlightColor | string;
  opacity: HighlightOpacity;
  strokeWidth: number;
}

export type Annotation =
  | PenAnnotation
  | HighlightAnnotation
  | RectAnnotation
  | CircleAnnotation
  | LineAnnotation
  | ArrowAnnotation
  | TextAnnotation
  | NumberAnnotation
  | RedactAnnotation;
