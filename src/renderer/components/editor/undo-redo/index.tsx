import { RotateCcw, RotateCw } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/renderer/components/ui/tooltip';

interface UndoRedoButtonsProps {
  onUndo: () => void;
  onRedo: () => void;
  canUndo: boolean;
  canRedo: boolean;
}

export default function UndoRedoButtons({
  onUndo,
  onRedo,
  canUndo,
  canRedo,
}: UndoRedoButtonsProps) {
  return (
    <div className="flex gap-1">
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            className="size-7"
            onClick={onUndo}
            disabled={!canUndo}
          >
            <RotateCcw className="size-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Undo (⌘Z)</TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            className="size-7"
            onClick={onRedo}
            disabled={!canRedo}
          >
            <RotateCw className="size-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Redo (⌘⇧Z)</TooltipContent>
      </Tooltip>
    </div>
  );
}
