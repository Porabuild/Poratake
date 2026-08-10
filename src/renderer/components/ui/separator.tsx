import * as React from 'react';
import { Separator as HeroSeparator } from '@heroui/react';
import { separatorVariants } from '@heroui/styles';

import { cn } from '@/renderer/lib/utils';

interface SeparatorProps extends Omit<
  React.ComponentProps<typeof HeroSeparator>,
  'orientation'
> {
  orientation?: 'horizontal' | 'vertical';
  decorative?: boolean;
}

function Separator({
  className,
  orientation = 'horizontal',
  decorative = true,
  variant,
  ...props
}: SeparatorProps) {
  const separatorClassName = cn('shrink-0', className);
  if (decorative) {
    return (
      <div
        data-slot="separator"
        data-orientation={orientation}
        className={separatorVariants({
          orientation,
          variant,
          className: separatorClassName,
        })}
        role="presentation"
        aria-hidden
      />
    );
  }

  return (
    <HeroSeparator
      data-slot="separator"
      orientation={orientation}
      variant={variant}
      className={separatorClassName}
      {...props}
    />
  );
}

export { Separator };
