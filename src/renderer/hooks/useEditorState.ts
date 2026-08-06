import { useState, useCallback, useEffect, useRef } from 'react';
import type {
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
} from '@/types/editor';
import type { EditorPreferences } from '@/types/settings';

const TAILWIND_COLORS = {
  ROSE: '#f43f5e',
  ORANGE: '#f97316',
  AMBER: '#f59e0b',
  EMERALD: '#10b981',
  SKY: '#0ea5e9',
  VIOLET: '#8b5cf6',
  SLATE: '#1e293b',
  WHITE: '#ffffff',
};

interface EditorState {
  activeTool: ToolType;
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
  setActiveTool: (tool: ToolType) => void;
  setSelectedColor: (color: string) => void;
  setStrokeWidth: (width: number) => void;
  setArrowStyle: (style: ArrowStyle) => void;
  setHighlightColor: (color: HighlightColor) => void;
  setHighlightOpacity: (opacity: HighlightOpacity) => void;
  setNumberStyle: (style: NumberStyle) => void;
  setNumberSize: (size: NumberSize) => void;
  setNumberStartValue: (value: number) => void;
  setTextBackground: (enabled: boolean) => void;
  setTextFontSize: (size: TextFontSize) => void;
  setTextFontFamily: (family: TextFontFamily) => void;
  setRedactStyle: (style: RedactStyle) => void;
  setRedactIntensity: (intensity: RedactIntensity) => void;
  setShapeFillMode: (mode: ShapeFillMode) => void;
}

interface UseEditorStateOptions {
  initialPreferences?: EditorPreferences;
}

export const useEditorState = (
  options?: UseEditorStateOptions
): EditorState => {
  const { initialPreferences } = options ?? {};

  const savedTool = initialPreferences?.lastTool;
  const initialTool =
    savedTool && savedTool !== 'wallpaper' ? savedTool : 'select';
  const [activeTool, setActiveTool] = useState<ToolType>(initialTool);
  const [selectedColor, setSelectedColor] = useState<string>(
    initialPreferences?.color ?? TAILWIND_COLORS.ROSE
  );
  const [strokeWidth, setStrokeWidth] = useState<number>(
    initialPreferences?.strokeWidth ?? 3
  );
  const [arrowStyle, setArrowStyle] = useState<ArrowStyle>(
    initialPreferences?.arrowStyle ?? 'standard'
  );
  const [highlightColor, setHighlightColor] = useState<HighlightColor>(
    (initialPreferences?.highlightColor as HighlightColor) ?? '#FFFF00'
  );
  const [highlightOpacity, setHighlightOpacity] = useState<HighlightOpacity>(
    (initialPreferences?.highlightOpacity as HighlightOpacity) ?? 0.4
  );
  const [numberStyle, setNumberStyle] = useState<NumberStyle>(
    initialPreferences?.numberStyle ?? 'numeric'
  );
  const [numberSize, setNumberSize] = useState<NumberSize>(
    initialPreferences?.numberSize ?? 'medium'
  );
  const [numberStartValue, setNumberStartValue] = useState<number>(
    initialPreferences?.numberStartValue ?? 1
  );
  const [textBackground, setTextBackground] = useState<boolean>(
    initialPreferences?.textBackground ?? true
  );
  const [textFontSize, setTextFontSize] = useState<TextFontSize>(
    initialPreferences?.textFontSize ?? 20
  );
  const [textFontFamily, setTextFontFamily] = useState<TextFontFamily>(
    initialPreferences?.textFontFamily ?? 'sans'
  );
  const [redactStyle, setRedactStyle] = useState<RedactStyle>(
    initialPreferences?.redactStyle ?? 'pixelate'
  );
  const [redactIntensity, setRedactIntensity] = useState<RedactIntensity>(
    initialPreferences?.redactIntensity ?? 5
  );
  const [shapeFillMode, setShapeFillMode] = useState<ShapeFillMode>(
    (initialPreferences?.shapeFillMode as ShapeFillMode) ?? 'outline'
  );

  const isInitialMount = useRef(true);

  useEffect(() => {
    if (isInitialMount.current) {
      isInitialMount.current = false;
      return;
    }

    const timeoutId = setTimeout(() => {
      const toolToSave =
        activeTool === 'crop' || activeTool === 'wallpaper'
          ? undefined
          : activeTool;

      const updates: Partial<EditorPreferences> = {
        color: selectedColor,
        strokeWidth,
        arrowStyle,
        highlightColor,
        highlightOpacity,
        numberStyle,
        numberSize,
        numberStartValue,
        textBackground,
        textFontSize,
        textFontFamily,
        redactStyle,
        redactIntensity,
        shapeFillMode,
      };

      if (toolToSave) {
        updates.lastTool = toolToSave;
      }

      window.ipcRenderer.invoke('editor:updatePreferences', updates);
    }, 500);

    return () => clearTimeout(timeoutId);
  }, [
    activeTool,
    selectedColor,
    strokeWidth,
    arrowStyle,
    highlightColor,
    highlightOpacity,
    numberStyle,
    numberSize,
    numberStartValue,
    textBackground,
    textFontSize,
    textFontFamily,
    redactStyle,
    redactIntensity,
    shapeFillMode,
  ]);

  const handleSetActiveTool = useCallback((tool: ToolType) => {
    setActiveTool(tool);
  }, []);

  const handleSetSelectedColor = useCallback((color: string) => {
    setSelectedColor(color);
  }, []);

  const handleSetStrokeWidth = useCallback((width: number) => {
    setStrokeWidth(width);
  }, []);

  const handleSetArrowStyle = useCallback((style: ArrowStyle) => {
    setArrowStyle(style);
  }, []);

  const handleSetHighlightColor = useCallback((color: HighlightColor) => {
    setHighlightColor(color);
  }, []);

  const handleSetHighlightOpacity = useCallback((opacity: HighlightOpacity) => {
    setHighlightOpacity(opacity);
  }, []);

  const handleSetNumberStyle = useCallback((style: NumberStyle) => {
    setNumberStyle(style);
  }, []);

  const handleSetNumberSize = useCallback((size: NumberSize) => {
    setNumberSize(size);
  }, []);

  const handleSetNumberStartValue = useCallback((value: number) => {
    setNumberStartValue(value);
  }, []);

  const handleSetTextBackground = useCallback((enabled: boolean) => {
    setTextBackground(enabled);
  }, []);

  const handleSetTextFontSize = useCallback((size: TextFontSize) => {
    setTextFontSize(size);
  }, []);

  const handleSetTextFontFamily = useCallback((family: TextFontFamily) => {
    setTextFontFamily(family);
  }, []);

  const handleSetRedactStyle = useCallback((style: RedactStyle) => {
    setRedactStyle(style);
  }, []);

  const handleSetRedactIntensity = useCallback((intensity: RedactIntensity) => {
    setRedactIntensity(intensity);
  }, []);

  const handleSetShapeFillMode = useCallback((mode: ShapeFillMode) => {
    setShapeFillMode(mode);
  }, []);

  return {
    activeTool,
    selectedColor,
    strokeWidth,
    arrowStyle,
    highlightColor,
    highlightOpacity,
    numberStyle,
    numberSize,
    numberStartValue,
    setActiveTool: handleSetActiveTool,
    setSelectedColor: handleSetSelectedColor,
    setStrokeWidth: handleSetStrokeWidth,
    setArrowStyle: handleSetArrowStyle,
    setHighlightColor: handleSetHighlightColor,
    setHighlightOpacity: handleSetHighlightOpacity,
    setNumberStyle: handleSetNumberStyle,
    setNumberSize: handleSetNumberSize,
    setNumberStartValue: handleSetNumberStartValue,
    textBackground,
    setTextBackground: handleSetTextBackground,
    textFontSize,
    setTextFontSize: handleSetTextFontSize,
    textFontFamily,
    setTextFontFamily: handleSetTextFontFamily,
    redactStyle,
    setRedactStyle: handleSetRedactStyle,
    redactIntensity,
    setRedactIntensity: handleSetRedactIntensity,
    shapeFillMode,
    setShapeFillMode: handleSetShapeFillMode,
  };
};

export { TAILWIND_COLORS };
