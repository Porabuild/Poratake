import {
  MousePointer2,
  Pencil,
  Highlighter,
  Square,
  Circle,
  Minus,
  ArrowUp,
  Type,
  ListOrdered,
  EyeOff,
  Crop,
  Camera,
} from 'lucide-react';
import UndoRedoButtons from '@/renderer/components/editor/undo-redo';
import { WallpaperSheetTrigger } from '@/renderer/components/editor/wallpaper';
import type { ToolType } from '@/types/editor';
import type { EditorShortcuts } from '@/types/settings';
import { DEFAULT_SETTINGS } from '@/types/settings';
import { Button } from '@/renderer/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/renderer/components/ui/tooltip';

interface ToolbarProps {
  activeTool: ToolType;
  onToolChange: (tool: ToolType) => void;
  onUndo: () => void;
  onRedo: () => void;
  canUndo: boolean;
  canRedo: boolean;
  shortcuts?: EditorShortcuts;
  isCaptureMode?: boolean;
  onCaptureClick?: () => void;
}

interface ToolButtonProps {
  tool: ToolType;
  activeTool: ToolType;
  onToolChange: (tool: ToolType) => void;
  name: string;
  shortcut: string;
  icon: React.ReactNode;
}

function ToolButton({
  tool,
  activeTool,
  onToolChange,
  name,
  shortcut,
  icon,
}: ToolButtonProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant={activeTool === tool ? 'default' : 'ghost'}
          className="size-7"
          onClick={() => onToolChange(tool)}
        >
          {icon}
        </Button>
      </TooltipTrigger>
      <TooltipContent>
        {name} ({shortcut.toUpperCase()})
      </TooltipContent>
    </Tooltip>
  );
}

export default function Toolbar({
  activeTool,
  onToolChange,
  onUndo,
  onRedo,
  canUndo,
  canRedo,
  shortcuts,
  isCaptureMode,
  onCaptureClick,
}: ToolbarProps) {
  const s = shortcuts ?? DEFAULT_SETTINGS.shortcuts.editor;

  return (
    <div className="flex items-center gap-1">
      <ToolButton
        tool="pen"
        activeTool={activeTool}
        onToolChange={onToolChange}
        name="Pen"
        shortcut={s.pen}
        icon={<Pencil className="size-4" />}
      />
      <ToolButton
        tool="highlight"
        activeTool={activeTool}
        onToolChange={onToolChange}
        name="Highlight"
        shortcut={s.highlight}
        icon={<Highlighter className="size-4" />}
      />
      <ToolButton
        tool="rectangle"
        activeTool={activeTool}
        onToolChange={onToolChange}
        name="Rectangle"
        shortcut={s.rectangle}
        icon={<Square className="size-4" />}
      />
      <ToolButton
        tool="circle"
        activeTool={activeTool}
        onToolChange={onToolChange}
        name="Circle"
        shortcut={s.circle}
        icon={<Circle className="size-4" />}
      />
      <ToolButton
        tool="line"
        activeTool={activeTool}
        onToolChange={onToolChange}
        name="Line"
        shortcut={s.line}
        icon={<Minus className="size-4" />}
      />
      <ToolButton
        tool="arrow"
        activeTool={activeTool}
        onToolChange={onToolChange}
        name="Arrow"
        shortcut={s.arrow}
        icon={<ArrowUp className="size-4" />}
      />
      <ToolButton
        tool="text"
        activeTool={activeTool}
        onToolChange={onToolChange}
        name="Text"
        shortcut={s.text}
        icon={<Type className="size-4" />}
      />
      <ToolButton
        tool="number"
        activeTool={activeTool}
        onToolChange={onToolChange}
        name="Number"
        shortcut={s.number}
        icon={<ListOrdered className="size-4" />}
      />
      <ToolButton
        tool="redact"
        activeTool={activeTool}
        onToolChange={onToolChange}
        name="Redact"
        shortcut={s.redact}
        icon={<EyeOff className="size-4" />}
      />
      <ToolButton
        tool="select"
        activeTool={activeTool}
        onToolChange={onToolChange}
        name="Select"
        shortcut={s.select}
        icon={<MousePointer2 className="size-4" />}
      />

      <div className="bg-border mx-1 h-[18px] w-px" />

      <ToolButton
        tool="crop"
        activeTool={activeTool}
        onToolChange={onToolChange}
        name="Crop"
        shortcut={s.crop}
        icon={<Crop className="size-4" />}
      />
      <WallpaperSheetTrigger
        onClick={() => onToolChange('wallpaper')}
        isOpen={activeTool === 'wallpaper'}
        shortcut={s.wallpaper}
      />
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant={isCaptureMode ? 'default' : 'ghost'}
            className="size-7"
            onClick={onCaptureClick}
          >
            <Camera className="size-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          Capture & Attach (hold ⌘ for edge picker)
        </TooltipContent>
      </Tooltip>

      <div className="bg-border mx-1 h-[18px] w-px" />

      <UndoRedoButtons
        onUndo={onUndo}
        onRedo={onRedo}
        canUndo={canUndo}
        canRedo={canRedo}
      />
    </div>
  );
}
