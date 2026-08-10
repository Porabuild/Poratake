import {
  ColorArea,
  ColorField,
  ColorPicker as HeroColorPicker,
  ColorSlider,
  ColorSwatch,
  ColorSwatchPicker,
} from '@heroui/react';
import { ChevronDown, Shuffle } from 'lucide-react';

import { Button } from '@/renderer/components/ui/button';
import { HIGHLIGHT_COLORS, type ToolType } from '@/types/editor';
import { COLOR_PALETTE, TAILWIND_COLORS } from '../shared';

const HIGHLIGHT_PALETTE = [
  { name: 'Yellow', value: HIGHLIGHT_COLORS[0] },
  { name: 'Green', value: HIGHLIGHT_COLORS[1] },
  { name: 'Pink', value: HIGHLIGHT_COLORS[2] },
  { name: 'Blue', value: HIGHLIGHT_COLORS[3] },
  { name: 'Orange', value: HIGHLIGHT_COLORS[4] },
] as const;

const LEGACY_COLOR_VALUES: Record<string, string> = {
  'oklch(64.5% 0.246 16.439)': TAILWIND_COLORS.ROSE,
  'oklch(70.5% 0.213 47.604)': TAILWIND_COLORS.ORANGE,
  'oklch(76.9% 0.188 70.08)': TAILWIND_COLORS.AMBER,
  'oklch(72.3% 0.219 149.579)': TAILWIND_COLORS.GREEN,
  'oklch(69.6% 0.17 162.48)': TAILWIND_COLORS.EMERALD,
  'oklch(62.3% 0.214 259.815)': TAILWIND_COLORS.SKY,
  'oklch(60.6% 0.25 292.717)': TAILWIND_COLORS.VIOLET,
  'oklch(62.7% 0.265 303.9)': TAILWIND_COLORS.PURPLE,
  'oklch(58.5% 0.233 277.117)': TAILWIND_COLORS.INDIGO,
  'oklch(55.4% 0.046 257.417)': TAILWIND_COLORS.SLATE,
};

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
  const pickerColor = LEGACY_COLOR_VALUES[selectedColor] ?? selectedColor;

  const handleRandomColor = () => {
    const channels = crypto.getRandomValues(new Uint8Array(3));
    const color = Array.from(channels, channel =>
      channel.toString(16).padStart(2, '0')
    ).join('');
    onColorChange(`#${color}`);
  };

  return (
    <HeroColorPicker
      value={pickerColor}
      onChange={color => onColorChange(color.toString('hex').toLowerCase())}
    >
      <HeroColorPicker.Trigger
        aria-label="Choose color"
        className="group bg-default hover:bg-default-hover flex h-7 items-center gap-2 rounded-3xl px-2 outline-none"
        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
      >
        <ColorSwatch
          size="xs"
          style={{ opacity: isHighlight ? highlightOpacity : undefined }}
        />
        <ChevronDown className="text-muted-foreground size-3.5 transition-transform group-aria-expanded:rotate-180" />
      </HeroColorPicker.Trigger>
      <HeroColorPicker.Popover
        placement="bottom left"
        className="border-border bg-overlay w-64 rounded-2xl! border p-3! shadow-xl"
      >
        <ColorSwatchPicker
          aria-label="Preset colors"
          size="xs"
          className="flex-nowrap justify-between gap-1"
        >
          {palette.map(color => (
            <ColorSwatchPicker.Item
              key={color.value}
              color={color.value}
              aria-label={color.name}
            >
              <ColorSwatchPicker.Swatch />
              <ColorSwatchPicker.Indicator />
            </ColorSwatchPicker.Item>
          ))}
        </ColorSwatchPicker>
        <ColorArea
          aria-label="Saturation and brightness"
          colorSpace="hsb"
          xChannel="saturation"
          yChannel="brightness"
          className="aspect-4/3 max-w-none rounded-2xl"
        >
          <ColorArea.Thumb />
        </ColorArea>
        <div className="flex items-center gap-2">
          <ColorSlider
            aria-label="Hue"
            channel="hue"
            colorSpace="hsb"
            className="min-w-0 flex-1"
          >
            <ColorSlider.Track>
              <ColorSlider.Thumb />
            </ColorSlider.Track>
          </ColorSlider>
          <Button
            type="button"
            variant="tertiary"
            size="icon-sm"
            aria-label="Choose a random color"
            onClick={handleRandomColor}
            className="rounded-full!"
          >
            <Shuffle className="size-3.5" />
          </Button>
        </div>
        <ColorField aria-label="Hex color" fullWidth>
          <ColorField.Group
            variant="secondary"
            fullWidth
            className="rounded-xl!"
          >
            <ColorField.Prefix>
              <ColorSwatch size="xs" />
            </ColorField.Prefix>
            <ColorField.Input />
          </ColorField.Group>
        </ColorField>
      </HeroColorPicker.Popover>
    </HeroColorPicker>
  );
}
