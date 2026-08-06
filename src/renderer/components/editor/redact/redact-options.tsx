import { Grid3X3, Droplets, Square } from 'lucide-react';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from '@/renderer/components/ui/select';
import type { RedactIntensity, RedactStyle } from '@/types/editor';

interface RedactOptionsProps {
  redactStyle: RedactStyle;
  onRedactStyleChange: (style: RedactStyle) => void;
  redactIntensity: RedactIntensity;
  onRedactIntensityChange: (intensity: RedactIntensity) => void;
}

const STYLE_OPTIONS: {
  value: RedactStyle;
  label: string;
  icon: JSX.Element;
}[] = [
  {
    value: 'pixelate',
    label: 'Pixelate',
    icon: <Grid3X3 className="size-4" />,
  },
  { value: 'blur', label: 'Blur', icon: <Droplets className="size-4" /> },
  {
    value: 'blackout',
    label: 'Black Out',
    icon: <Square className="size-4" />,
  },
];

const INTENSITY_LEVELS: RedactIntensity[] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

export default function RedactOptions({
  redactStyle,
  onRedactStyleChange,
  redactIntensity,
  onRedactIntensityChange,
}: RedactOptionsProps) {
  const selectedStyleOption = STYLE_OPTIONS.find(
    opt => opt.value === redactStyle
  );

  return (
    <Select
      value={redactStyle}
      onValueChange={value => onRedactStyleChange(value as RedactStyle)}
    >
      <SelectTrigger size="sm" className="h-7! w-auto gap-1 px-2">
        <SelectValue>{selectedStyleOption?.icon}</SelectValue>
      </SelectTrigger>
      <SelectContent align="center" className="min-w-40">
        {STYLE_OPTIONS.map(option => (
          <SelectItem key={option.value} value={option.value}>
            <div className="flex items-center gap-2">
              {option.icon}
              <span>{option.label}</span>
            </div>
          </SelectItem>
        ))}
        {}
        {redactStyle !== 'blackout' && (
          <>
            <SelectSeparator />
            <div className="flex items-center justify-between px-2 py-1.5">
              <span className="text-muted-foreground text-xs">Intensity:</span>
              <Select
                value={String(redactIntensity)}
                onValueChange={value =>
                  onRedactIntensityChange(Number(value) as RedactIntensity)
                }
              >
                <SelectTrigger size="sm" className="h-6! w-14 px-2 text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent align="end">
                  {INTENSITY_LEVELS.map(level => (
                    <SelectItem key={level} value={String(level)}>
                      <span className="font-medium">{level}</span>
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </>
        )}
      </SelectContent>
    </Select>
  );
}
