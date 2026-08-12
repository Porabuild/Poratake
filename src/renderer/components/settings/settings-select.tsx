import { Select } from '@/renderer/components/ui/select';
import type { SelectOption } from '@/renderer/components/ui/select';

import { cn } from '@/renderer/lib/utils';

interface SettingsSelectProps {
  label: string;
  options: readonly SelectOption[];
  value: string;
  onChange: (value: string) => void;
  onOpenChange?: (isOpen: boolean) => void;
  size?: 'sm' | 'md';
  className?: string;
}

export default function SettingsSelect({
  label,
  options,
  value,
  onChange,
  onOpenChange,
  size,
  className,
}: SettingsSelectProps) {
  return (
    <Select
      label={label}
      options={options}
      value={value}
      onChange={onChange}
      onOpenChange={onOpenChange}
      size={size}
      className={cn('w-40 shrink-0', className)}
    />
  );
}
