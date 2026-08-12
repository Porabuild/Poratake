import { ChevronDown, Grid3X3, Droplets, Square } from 'lucide-react';
import { ListBox, Popover, Select } from '@heroui/react';
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
    <Popover>
      <Popover.Trigger
        aria-label="Redaction options"
        className="group bg-default hover:bg-default-hover flex h-7 items-center gap-2 rounded-3xl px-2 outline-none"
        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
      >
        {selectedStyleOption?.icon}
        <ChevronDown className="text-muted-foreground size-3.5 transition-transform group-aria-expanded:rotate-180" />
      </Popover.Trigger>
      <Popover.Content placement="bottom" className="min-w-40">
        <Popover.Dialog className="p-0">
          <ListBox
            aria-label="Redaction style"
            selectionMode="single"
            disallowEmptySelection
            selectedKeys={[redactStyle]}
            onSelectionChange={keys => {
              if (keys === 'all') {
                return;
              }

              const value = keys.values().next().value;
              if (value === undefined) {
                return;
              }

              onRedactStyleChange(value as RedactStyle);
            }}
          >
            {STYLE_OPTIONS.map(option => (
              <ListBox.Item
                key={option.value}
                id={option.value}
                textValue={option.label}
              >
                {option.icon}
                <span className="flex-1">{option.label}</span>
                <ListBox.ItemIndicator />
              </ListBox.Item>
            ))}
          </ListBox>
          {redactStyle !== 'blackout' && (
            <>
              <div className="bg-separator h-px" />
              <div className="flex items-center justify-between px-2 py-1.5">
                <span className="text-muted-foreground text-xs">
                  Intensity:
                </span>
                <Select
                  aria-label="Redaction intensity"
                  variant="secondary"
                  value={String(redactIntensity)}
                  onChange={value => {
                    if (value === null) {
                      return;
                    }

                    onRedactIntensityChange(Number(value) as RedactIntensity);
                  }}
                >
                  <Select.Trigger className="h-6 min-h-6 w-14 items-center rounded-3xl py-0 ps-2 text-xs">
                    <span className="flex-1 font-medium">
                      {redactIntensity}
                    </span>
                    <Select.Indicator />
                  </Select.Trigger>
                  <Select.Popover>
                    <ListBox className="[&>*+*]:mt-0">
                      {INTENSITY_LEVELS.map(level => (
                        <ListBox.Item
                          key={level}
                          id={String(level)}
                          textValue={String(level)}
                          className="min-h-6 gap-2 rounded-lg py-1 text-xs"
                        >
                          <span className="flex-1 font-medium">{level}</span>
                          <ListBox.ItemIndicator />
                        </ListBox.Item>
                      ))}
                    </ListBox>
                  </Select.Popover>
                </Select>
              </div>
            </>
          )}
        </Popover.Dialog>
      </Popover.Content>
    </Popover>
  );
}
