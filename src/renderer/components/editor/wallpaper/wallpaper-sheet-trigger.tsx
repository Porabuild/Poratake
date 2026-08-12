import { Wallpaper } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/renderer/components/ui/tooltip';

interface WallpaperSheetTriggerProps {
  onClick: () => void;
  isOpen: boolean;
  shortcut?: string;
  onIntent?: () => void;
}

export default function WallpaperSheetTrigger({
  onClick,
  isOpen,
  shortcut,
  onIntent,
}: WallpaperSheetTriggerProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant={isOpen ? 'tertiary' : 'ghost'}
          size="icon-sm"
          className="size-7!"
          onMouseEnter={onIntent}
          onFocus={onIntent}
          onClick={onClick}
        >
          <Wallpaper className="size-4" />
        </Button>
      </TooltipTrigger>
      <TooltipContent>
        Wallpaper {shortcut ? `(${shortcut.toUpperCase()})` : ''}
      </TooltipContent>
    </Tooltip>
  );
}
