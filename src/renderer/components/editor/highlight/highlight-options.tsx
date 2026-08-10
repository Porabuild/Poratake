import { Highlighter } from 'lucide-react';
import { ListBox, Select } from '@heroui/react';
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
      aria-label="Highlighter opacity"
      variant="secondary"
      value={String(highlightOpacity)}
      onChange={value => {
        if (value === null) {
          return;
        }

        onHighlightOpacityChange(Number(value) as HighlightOpacity);
      }}
    >
      <Select.Trigger className="h-7 min-h-7 items-center rounded-3xl py-0 ps-2">
        <Highlighter className="size-4" />
        <Select.Indicator />
      </Select.Trigger>
      <Select.Popover placement="bottom">
        <ListBox>
          {OPACITY_LEVELS.map(level => (
            <ListBox.Item
              key={level}
              id={String(level)}
              textValue={`${Math.round(level * 100)}%`}
            >
              <span className="flex-1 font-medium">
                {Math.round(level * 100)}%
              </span>
              <ListBox.ItemIndicator />
            </ListBox.Item>
          ))}
        </ListBox>
      </Select.Popover>
    </Select>
  );
}
