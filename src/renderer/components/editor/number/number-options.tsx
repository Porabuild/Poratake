import { ListBox, Popover, Select } from '@heroui/react';
import { ChevronDown } from 'lucide-react';
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
      <Popover>
        <Popover.Trigger
          aria-label="Number options"
          className="group bg-default hover:bg-default-hover flex h-7 items-center gap-2 rounded-3xl px-2 outline-none"
          style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
        >
          <NumberBadgePreview style={numberStyle} size={20} />
          <ChevronDown className="text-muted-foreground size-3.5 transition-transform group-aria-expanded:rotate-180" />
        </Popover.Trigger>
        <Popover.Content placement="bottom" className="min-w-48">
          <Popover.Dialog className="p-0">
            <ListBox
              aria-label="Number style"
              selectionMode="single"
              disallowEmptySelection
              selectedKeys={[numberStyle]}
              onSelectionChange={keys => {
                if (keys === 'all') {
                  return;
                }

                const value = keys.values().next().value;
                if (value === undefined) {
                  return;
                }

                onNumberStyleChange(value as NumberStyle);
              }}
            >
              {NUMBER_STYLES.map(({ value, label }) => (
                <ListBox.Item key={value} id={value} textValue={label}>
                  <NumberBadgePreview style={value} size={20} />
                  <span className="flex-1">{label}</span>
                  <ListBox.ItemIndicator />
                </ListBox.Item>
              ))}
            </ListBox>
            <div className="bg-separator h-px" />
            <div className="flex items-center justify-between px-2 py-1.5">
              <span className="text-muted-foreground text-xs">Starting:</span>
              <Select
                aria-label="Starting number"
                variant="secondary"
                value={String(numberStartValue)}
                onChange={value => {
                  if (value === null) {
                    return;
                  }

                  onNumberStartValueChange(Number(value));
                }}
              >
                <Select.Trigger className="h-6 min-h-6 w-14 items-center rounded-3xl py-0 ps-2 text-xs">
                  <span className="flex-1">
                    {getDisplayValue(numberStartValue, numberStyle)}
                  </span>
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox className="[&>*+*]:mt-0">
                    {START_VALUES.map(value => (
                      <ListBox.Item
                        key={value}
                        id={String(value)}
                        textValue={getDisplayValue(value, numberStyle)}
                        className="min-h-6 gap-2 rounded-lg py-1 text-xs"
                      >
                        <span className="flex-1">
                          {getDisplayValue(value, numberStyle)}
                        </span>
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
            </div>
            <div className="bg-separator h-px" />
            <div className="flex items-center justify-between px-2 py-1.5">
              <span className="text-muted-foreground text-xs">Size:</span>
              <Select
                aria-label="Number size"
                variant="secondary"
                value={numberSize}
                onChange={value => {
                  if (value === null) {
                    return;
                  }

                  onNumberSizeChange(value as NumberSize);
                }}
              >
                <Select.Trigger className="h-6 min-h-6 w-24 items-center rounded-3xl py-0 ps-2 text-xs">
                  <span className="flex-1 capitalize">{numberSize}</span>
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox className="[&>*+*]:mt-0">
                    {NUMBER_SIZES.map(({ value, label }) => (
                      <ListBox.Item
                        key={value}
                        id={value}
                        textValue={label}
                        className="min-h-6 gap-2 rounded-lg py-1 text-xs"
                      >
                        <span className="flex-1">{label}</span>
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
            </div>
          </Popover.Dialog>
        </Popover.Content>
      </Popover>
      <div className="bg-border mx-1 h-[18px] w-px" />
    </>
  );
}
