import { Minus, Plus } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/renderer/components/ui/tooltip';
import {
  PLAYBACK_SPEED_PRESETS,
  formatPlaybackSpeed,
} from '@/types/playback-speed';

interface SpeedSelectorProps {
  speed: number;
  onSpeedChange: (speed: number) => void;
  disabled?: boolean;
}

export default function SpeedSelector({
  speed,
  onSpeedChange,
  disabled = false,
}: SpeedSelectorProps) {
  const currentIndex = PLAYBACK_SPEED_PRESETS.indexOf(speed as never);
  const canDecrease = currentIndex > 0;
  const canIncrease = currentIndex < PLAYBACK_SPEED_PRESETS.length - 1;

  const handleDecrease = () => {
    if (!canDecrease) return;
    onSpeedChange(PLAYBACK_SPEED_PRESETS[currentIndex - 1]);
  };

  const handleIncrease = () => {
    if (!canIncrease) return;
    onSpeedChange(PLAYBACK_SPEED_PRESETS[currentIndex + 1]);
  };

  return (
    <div className="flex items-center gap-0.5">
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon-sm"
            className="size-7!"
            onClick={handleDecrease}
            disabled={disabled || !canDecrease}
          >
            <Minus className="size-3.5" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="top">Decrease Speed</TooltipContent>
      </Tooltip>

      <Tooltip>
        <TooltipTrigger asChild>
          <span className="text-muted-foreground w-10 text-center text-xs font-medium">
            {formatPlaybackSpeed(speed)}
          </span>
        </TooltipTrigger>
        <TooltipContent side="top">Playback Speed</TooltipContent>
      </Tooltip>

      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon-sm"
            className="size-7!"
            onClick={handleIncrease}
            disabled={disabled || !canIncrease}
          >
            <Plus className="size-3.5" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="top">Increase Speed</TooltipContent>
      </Tooltip>
    </div>
  );
}
