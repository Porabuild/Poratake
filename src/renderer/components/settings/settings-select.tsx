import { ListBox, Select } from '@heroui/react';

import { cn } from '@/renderer/lib/utils';

interface SettingsSelectOption {
  value: string;
  label: string;
}

interface SettingsSelectProps {
  label: string;
  options: readonly SettingsSelectOption[];
  value: string;
  onChange: (value: string) => void;
  onOpenChange?: (isOpen: boolean) => void;
  className?: string;
}

export default function SettingsSelect({
  label,
  options,
  value,
  onChange,
  onOpenChange,
  className,
}: SettingsSelectProps) {
  const selectedValue = options.some(option => option.value === value)
    ? value
    : null;

  return (
    <Select
      aria-label={label}
      className={cn('w-40 shrink-0', className)}
      variant="secondary"
      value={selectedValue}
      onChange={nextValue =>
        onChange(nextValue === null ? '' : String(nextValue))
      }
      onOpenChange={onOpenChange}
    >
      <Select.Trigger>
        <Select.Value />
        <Select.Indicator />
      </Select.Trigger>
      <Select.Popover>
        <ListBox>
          {options.map(option => (
            <ListBox.Item
              key={option.value}
              id={option.value}
              textValue={option.label}
            >
              {option.label}
              <ListBox.ItemIndicator />
            </ListBox.Item>
          ))}
        </ListBox>
      </Select.Popover>
    </Select>
  );
}
