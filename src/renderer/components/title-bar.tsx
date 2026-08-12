import Toolbar from '@/renderer/components/editor/toolbar';
import ToolOptions from '@/renderer/components/editor/tool-options';
import ColorPicker from '@/renderer/components/editor/color-picker';
import WindowControlsSpacer from '@/renderer/components/window-controls-spacer';
import { isMacPlatform } from '@/renderer/utils/platform';
import { cn } from '@/renderer/lib/utils';
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
import type { EditorShortcuts } from '@/types/settings';
import type { CloudUploadState } from '@/types/cloud';
import { Button } from '@/renderer/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/renderer/components/ui/tooltip';
import {
  CopyIcon,
  SaveIcon,
  CheckIcon,
  PinIcon,
  CloudUploadIcon,
  Loader2Icon,
} from 'lucide-react';
import { formatAccelerator } from '@/renderer/utils/shortcuts';

interface TitleBarProps {
  activeTool: ToolType;
  onToolChange: (tool: ToolType) => void;
  color: string;
  onColorChange: (color: string) => void;
  strokeWidth: number;
  onStrokeWidthChange: (width: number) => void;
  arrowStyle: ArrowStyle;
  onArrowStyleChange: (style: ArrowStyle) => void;
  highlightColor: HighlightColor;
  onHighlightColorChange: (color: HighlightColor) => void;
  highlightOpacity: HighlightOpacity;
  onHighlightOpacityChange: (opacity: HighlightOpacity) => void;
  numberStyle: NumberStyle;
  onNumberStyleChange: (style: NumberStyle) => void;
  numberSize: NumberSize;
  onNumberSizeChange: (size: NumberSize) => void;
  numberStartValue: number;
  onNumberStartValueChange: (value: number) => void;
  textBackground: boolean;
  onTextBackgroundChange: (enabled: boolean) => void;
  textFontSize: TextFontSize;
  onTextFontSizeChange: (size: TextFontSize) => void;
  textFontFamily: TextFontFamily;
  onTextFontFamilyChange: (family: TextFontFamily) => void;
  redactStyle: RedactStyle;
  onRedactStyleChange: (style: RedactStyle) => void;
  redactIntensity: RedactIntensity;
  onRedactIntensityChange: (intensity: RedactIntensity) => void;
  shapeFillMode: ShapeFillMode;
  onShapeFillModeChange: (mode: ShapeFillMode) => void;
  onUndo: () => void;
  onRedo: () => void;
  canUndo: boolean;
  canRedo: boolean;
  onCopy: () => void;
  onSave: () => void;
  onCloudUpload: () => void;
  onPin: () => void;
  isCopied: boolean;
  cloudUploadState: CloudUploadState;
  cloudUploadShortcut?: string;
  editorShortcuts?: EditorShortcuts;
  isCaptureMode?: boolean;
  onCaptureClick?: () => void;
  onWallpaperIntent?: () => void;
}

export default function TitleBar({
  activeTool,
  onToolChange,
  color,
  onColorChange,
  strokeWidth,
  onStrokeWidthChange,
  arrowStyle,
  onArrowStyleChange,
  highlightColor,
  onHighlightColorChange,
  highlightOpacity,
  onHighlightOpacityChange,
  numberStyle,
  onNumberStyleChange,
  numberSize,
  onNumberSizeChange,
  numberStartValue,
  onNumberStartValueChange,
  textBackground,
  onTextBackgroundChange,
  textFontSize,
  onTextFontSizeChange,
  textFontFamily,
  onTextFontFamilyChange,
  redactStyle,
  onRedactStyleChange,
  redactIntensity,
  onRedactIntensityChange,
  shapeFillMode,
  onShapeFillModeChange,
  onUndo,
  onRedo,
  canUndo,
  canRedo,
  onCopy,
  onSave,
  onCloudUpload,
  onPin,
  isCopied,
  cloudUploadState,
  cloudUploadShortcut,
  editorShortcuts,
  isCaptureMode,
  onCaptureClick,
  onWallpaperIntent,
}: TitleBarProps) {
  const isMac = isMacPlatform();
  const cloudUploadHint = cloudUploadShortcut
    ? ` (${formatAccelerator(cloudUploadShortcut)})`
    : '';
  const contextualControls = (
    <>
      <ToolOptions
        activeTool={activeTool}
        strokeWidth={strokeWidth}
        onStrokeWidthChange={onStrokeWidthChange}
        arrowStyle={arrowStyle}
        onArrowStyleChange={onArrowStyleChange}
        highlightOpacity={highlightOpacity}
        onHighlightOpacityChange={onHighlightOpacityChange}
        numberStyle={numberStyle}
        onNumberStyleChange={onNumberStyleChange}
        numberSize={numberSize}
        onNumberSizeChange={onNumberSizeChange}
        numberStartValue={numberStartValue}
        onNumberStartValueChange={onNumberStartValueChange}
        textBackground={textBackground}
        onTextBackgroundChange={onTextBackgroundChange}
        textFontSize={textFontSize}
        onTextFontSizeChange={onTextFontSizeChange}
        textFontFamily={textFontFamily}
        onTextFontFamilyChange={onTextFontFamilyChange}
        redactStyle={redactStyle}
        onRedactStyleChange={onRedactStyleChange}
        redactIntensity={redactIntensity}
        onRedactIntensityChange={onRedactIntensityChange}
        shapeFillMode={shapeFillMode}
        onShapeFillModeChange={onShapeFillModeChange}
        selectedColor={color}
      />
      <ColorPicker
        selectedColor={activeTool === 'highlight' ? highlightColor : color}
        onColorChange={
          activeTool === 'highlight'
            ? c => onHighlightColorChange(c as HighlightColor)
            : onColorChange
        }
        activeTool={activeTool}
        highlightOpacity={highlightOpacity}
      />
    </>
  );

  return (
    <div className="drag-region bg-card fixed top-0 right-0 left-0 z-9999 flex h-10 w-full flex-none items-center px-2">
      {isMac && <div className="w-[120px] flex-none" />}
      <div
        className={cn(
          'flex items-center gap-1',
          isMac && 'ml-auto justify-end'
        )}
      >
        {isMac && contextualControls}
        {isMac && <div className="bg-border mx-1 h-[18px] w-px" />}
        <Toolbar
          activeTool={activeTool}
          onToolChange={onToolChange}
          onUndo={onUndo}
          onRedo={onRedo}
          canUndo={canUndo}
          canRedo={canRedo}
          shortcuts={editorShortcuts}
          isCaptureMode={isCaptureMode}
          onCaptureClick={onCaptureClick}
          onWallpaperIntent={onWallpaperIntent}
        />
        <Tooltip>
          <TooltipTrigger asChild>
            <Button onClick={onCopy} size="icon-sm" variant="ghost">
              {isCopied ? (
                <CheckIcon className="size-4" />
              ) : (
                <CopyIcon className="size-4" />
              )}
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            Copy ({formatAccelerator('CommandOrControl+C')})
          </TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button onClick={onSave} size="icon-sm" variant="ghost">
              <SaveIcon className="size-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            Save ({formatAccelerator('CommandOrControl+S')})
          </TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              onClick={onCloudUpload}
              size="icon-sm"
              variant="ghost"
              disabled={cloudUploadState === 'uploading'}
            >
              {cloudUploadState === 'uploading' ? (
                <Loader2Icon className="size-4 animate-spin" />
              ) : cloudUploadState === 'success' ? (
                <CheckIcon className="size-4" />
              ) : (
                <CloudUploadIcon className="size-4" />
              )}
            </Button>
          </TooltipTrigger>
          <TooltipContent>{`Upload to Cloud${cloudUploadHint}`}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button onClick={onPin} size="icon-sm" variant="ghost">
              <PinIcon className="size-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Pin Screenshot</TooltipContent>
        </Tooltip>
        {!isMac && <div className="bg-border mx-1 h-[18px] w-px" />}
        {!isMac && contextualControls}
      </div>
      <WindowControlsSpacer />
    </div>
  );
}
