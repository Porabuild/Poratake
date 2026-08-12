import type { ReactNode } from 'react';
import { ListBox, Select as HeroSelect } from '@heroui/react';

import { cn } from '@/renderer/lib/utils';

interface SelectOption {
  value: string;
  label: string;
  content?: ReactNode;
  disabled?: boolean;
}

interface SelectProps {
  label: string;
  options: readonly SelectOption[];
  value: string;
  onChange: (value: string) => void;
  onOpenChange?: (isOpen: boolean) => void;
  size?: 'sm' | 'md';
  disabled?: boolean;
  placeholder?: string;
  className?: string;
  triggerClassName?: string;
}

const TRIGGER_SIZES: Record<'sm' | 'md', string> = {
  sm: 'select__trigger--sm h-7 min-h-7 py-0',
  md: '',
};

function Select({
  label,
  options,
  value,
  onChange,
  onOpenChange,
  size = 'md',
  disabled,
  placeholder,
  className,
  triggerClassName,
}: SelectProps) {
  const selectedValue = options.some(option => option.value === value)
    ? value
    : null;

  return (
    <HeroSelect
      data-slot="select"
      aria-label={label}
      variant="secondary"
      className={className}
      value={selectedValue}
      isDisabled={disabled}
      placeholder={placeholder}
      onChange={nextValue => {
        if (nextValue === null) {
          return;
        }

        onChange(String(nextValue));
      }}
      onOpenChange={onOpenChange}
    >
      {/* `items-center` keeps the value from stretching to the trigger height,
          which otherwise pins its text and icons to the top edge. */}
      <HeroSelect.Trigger
        className={cn('items-center', TRIGGER_SIZES[size], triggerClassName)}
      >
        <HeroSelect.Value />
        <HeroSelect.Indicator />
      </HeroSelect.Trigger>
      <HeroSelect.Popover
        className={size === 'sm' ? 'select__popover--sm' : undefined}
      >
        <ListBox>
          {options.map(option => (
            <ListBox.Item
              key={option.value}
              id={option.value}
              textValue={option.label}
              isDisabled={option.disabled}
            >
              {option.content ?? option.label}
              <ListBox.ItemIndicator />
            </ListBox.Item>
          ))}
        </ListBox>
      </HeroSelect.Popover>
    </HeroSelect>
  );
}

export { Select };
export type { SelectOption, SelectProps };
