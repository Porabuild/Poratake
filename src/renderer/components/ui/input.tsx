import * as React from 'react';
import { Input as HeroInput } from '@heroui/react';
import type { InputProps as HeroInputProps } from '@heroui/react';

import { cn } from '@/renderer/lib/utils';

export interface InputProps extends Omit<
  HeroInputProps,
  'isDisabled' | 'isReadOnly'
> {
  disabled?: boolean;
  readOnly?: boolean;
}

const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, disabled, readOnly, ...props }, ref) => (
    <HeroInput
      ref={ref}
      data-slot="input"
      disabled={disabled}
      readOnly={readOnly}
      className={cn('w-full', className)}
      {...props}
    />
  )
);

Input.displayName = 'Input';

export { Input };
