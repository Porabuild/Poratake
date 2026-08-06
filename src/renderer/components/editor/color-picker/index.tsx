import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/renderer/components/ui/select';
import {
  HIGHLIGHT_COLORS,
  type HighlightColor,
  type ToolType,
} from '@/types/editor';
import { COLOR_PALETTE, TAILWIND_COLORS } from '../shared';

interface ColorOption {
  name: string;
  value: string;
}

const HIGHLIGHT_COLOR_LABELS: Record<HighlightColor, string> = {
  '#FFFF00': 'Yellow',
  '#00FF00': 'Green',
  '#FF69B4': 'Pink',
  '#00BFFF': 'Blue',
  '#FFA500': 'Orange',
};

const HIGHLIGHT_PALETTE: ColorOption[] = HIGHLIGHT_COLORS.map(color => ({
  name: HIGHLIGHT_COLOR_LABELS[color],
  value: color,
}));

interface ColorPickerProps {
  selectedColor: string;
  onColorChange: (color: string) => void;
  activeTool?: ToolType;
  highlightOpacity?: number;
}

export default function ColorPicker({
  selectedColor,
  onColorChange,
  activeTool,
  highlightOpacity,
}: ColorPickerProps) {
  const isHighlight = activeTool === 'highlight';
  const palette = isHighlight ? HIGHLIGHT_PALETTE : COLOR_PALETTE;

  return (
    <Select value={selectedColor} onValueChange={onColorChange}>
      <SelectTrigger size="sm" className="h-7!">
        <SelectValue>
          <div
            className="size-4 rounded-full"
            style={{
              backgroundColor: selectedColor,
              opacity: isHighlight ? highlightOpacity : undefined,
            }}
          />
        </SelectValue>
      </SelectTrigger>
      <SelectContent align="center">
        {palette.map(color => (
          <SelectItem key={color.value} value={color.value}>
            <div className="flex items-center gap-2">
              <div
                className="size-4 rounded-full"
                style={{
                  backgroundColor: color.value,
                  opacity: isHighlight ? 0.6 : undefined,
                  borderColor: isHighlight
                    ? undefined
                    : color.value === TAILWIND_COLORS.WHITE
                      ? '#d1d5db'
                      : color.value,
                }}
              />
              <span className="text-sm">{color.name}</span>
            </div>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
