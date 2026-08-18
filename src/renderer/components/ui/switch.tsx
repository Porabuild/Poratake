import { Switch as HeroSwitch } from '@heroui/react';
import type { SwitchProps as HeroSwitchProps } from '@heroui/react';

import { cn } from '@/renderer/lib/utils';

interface SwitchProps extends Omit<
  HeroSwitchProps,
  'isSelected' | 'defaultSelected' | 'onChange' | 'children' | 'isDisabled'
> {
  checked?: boolean;
  defaultChecked?: boolean;
  onCheckedChange?: (checked: boolean) => void;
  disabled?: boolean;
}

function Switch({
  className,
  checked,
  defaultChecked,
  onCheckedChange,
  disabled,
  ...props
}: SwitchProps) {
  return (
    <HeroSwitch
      data-slot="switch"
      isSelected={checked}
      defaultSelected={defaultChecked}
      onChange={onCheckedChange}
      isDisabled={disabled}
      className={cn('shrink-0', className)}
      {...props}
    >
      <HeroSwitch.Content>
        <HeroSwitch.Control
          onClick={event => {
            event.preventDefault();
            event.stopPropagation();
            if (disabled) return;
            event.currentTarget
              .closest('label')
              ?.querySelector<HTMLInputElement>('input[role="switch"]')
              ?.click();
          }}
        >
          <HeroSwitch.Thumb />
        </HeroSwitch.Control>
      </HeroSwitch.Content>
    </HeroSwitch>
  );
}

export { Switch };
