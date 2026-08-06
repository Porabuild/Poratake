import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from '@/renderer/components/ui/select';
import type { NumberSize, NumberStyle } from '@/types/editor';
import { getDisplayValue } from './number-utils';

interface NumberOptionsProps {
  numberStyle: NumberStyle;
  onNumberStyleChange: (style: NumberStyle) => void;
  numberSize: NumberSize;
  onNumberSizeChange: (size: NumberSize) => void;
  numberStartValue: number;
  onNumberStartValueChange: (value: number) => void;
}

const NUMBER_STYLES: { value: NumberStyle; label: string }[] = [
  { value: 'numeric', label: '1, 2, 3, 4 ...' },
  { value: 'alpha-upper', label: 'A, B, C, D ...' },
  { value: 'roman', label: 'I, II, III, IV ...' },
  { value: 'alpha-lower', label: 'a, b, c, d ...' },
];

const NUMBER_SIZES: { value: NumberSize; label: string }[] = [
  { value: 'small', label: 'Small' },
  { value: 'medium', label: 'Medium' },
  { value: 'large', label: 'Large' },
];

const START_VALUES = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

function NumberBadgePreview({
  style,
  size = 20,
}: {
  style: NumberStyle;
  size?: number;
}) {
  const getPreviewText = () => {
    switch (style) {
      case 'numeric':
        return '1';
      case 'alpha-upper':
        return 'A';
      case 'roman':
        return 'I';
      case 'alpha-lower':
        return 'a';
      default:
        return '1';
    }
  };

  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none">
      <circle cx="12" cy="12" r="10" fill="currentColor" />
      <text
        x="12"
        y="12"
        textAnchor="middle"
        dominantBaseline="central"
        fontSize="11"
        fontWeight="bold"
        fontFamily="system-ui, -apple-system, sans-serif"
        className="fill-background"
      >
        {getPreviewText()}
      </text>
    </svg>
  );
}

export default function NumberOptions({
  numberStyle,
  onNumberStyleChange,
  numberSize,
  onNumberSizeChange,
  numberStartValue,
  onNumberStartValueChange,
}: NumberOptionsProps) {
  return (
    <>
      <Select
        value={numberStyle}
        onValueChange={value => onNumberStyleChange(value as NumberStyle)}
      >
        <SelectTrigger size="sm" className="h-7! w-auto gap-1 px-2">
          <SelectValue>
            <NumberBadgePreview style={numberStyle} size={20} />
          </SelectValue>
        </SelectTrigger>
        <SelectContent align="center">
          {NUMBER_STYLES.map(({ value, label }) => (
            <SelectItem key={value} value={value}>
              <div className="flex items-center gap-2">
                <NumberBadgePreview style={value} size={20} />
                <span>{label}</span>
              </div>
            </SelectItem>
          ))}
          <SelectSeparator />
          <div className="flex items-center justify-between px-2 py-1.5">
            <span className="text-muted-foreground text-xs">Starting:</span>
            <Select
              value={String(numberStartValue)}
              onValueChange={value => onNumberStartValueChange(Number(value))}
            >
              <SelectTrigger size="sm" className="h-6! w-14 px-2 text-xs">
                <SelectValue>
                  {getDisplayValue(numberStartValue, numberStyle)}
                </SelectValue>
              </SelectTrigger>
              <SelectContent align="end">
                {START_VALUES.map(value => (
                  <SelectItem key={value} value={String(value)}>
                    {getDisplayValue(value, numberStyle)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <SelectSeparator />
          <div className="flex items-center justify-between px-2 py-1.5">
            <span className="text-muted-foreground text-xs">Size:</span>
            <Select
              value={numberSize}
              onValueChange={value => onNumberSizeChange(value as NumberSize)}
            >
              <SelectTrigger size="sm" className="h-6! w-20 px-2 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent align="end">
                {NUMBER_SIZES.map(({ value, label }) => (
                  <SelectItem key={value} value={value}>
                    {label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </SelectContent>
      </Select>
      <div className="bg-border mx-1 h-[18px] w-px" />
    </>
  );
}
