import { Highlighter } from 'lucide-react';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/renderer/components/ui/select';
import type { HighlightOpacity } from '@/types/editor';

interface HighlightOptionsProps {
  highlightOpacity: HighlightOpacity;
  onHighlightOpacityChange: (opacity: HighlightOpacity) => void;
}

const OPACITY_LEVELS: HighlightOpacity[] = [0.2, 0.3, 0.4, 0.5, 0.6];

export default function HighlightOptions({
  highlightOpacity,
  onHighlightOpacityChange,
}: HighlightOptionsProps) {
  return (
    <Select
      value={String(highlightOpacity)}
      onValueChange={value =>
        onHighlightOpacityChange(Number(value) as HighlightOpacity)
      }
    >
      <SelectTrigger size="sm" className="h-7! w-auto gap-1 px-2">
        <SelectValue>
          <Highlighter className="size-4" />
        </SelectValue>
      </SelectTrigger>
      <SelectContent align="center">
        {OPACITY_LEVELS.map(level => (
          <SelectItem key={level} value={String(level)}>
            <span className="font-medium">{Math.round(level * 100)}%</span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
