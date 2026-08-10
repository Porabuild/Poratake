import * as React from 'react';
import { Button as HeroButton } from '@heroui/react';
import type { ButtonProps as HeroButtonProps } from '@heroui/react';

import { cn } from '@/renderer/lib/utils';

type LegacyVariant =
  | 'default'
  | 'destructive'
  | 'outline'
  | 'secondary'
  | 'tertiary'
  | 'ghost'
  | 'link';
type LegacySize = 'default' | 'sm' | 'lg' | 'icon' | 'icon-sm' | 'icon-lg';

interface ButtonProps extends Omit<
  HeroButtonProps,
  'variant' | 'size' | 'isDisabled'
> {
  variant?: LegacyVariant;
  size?: LegacySize;
  disabled?: boolean;
  title?: string;
  tabIndex?: number;
}

const VARIANTS: Record<LegacyVariant, HeroButtonProps['variant']> = {
  default: 'primary',
  destructive: 'danger',
  outline: 'outline',
  secondary: 'secondary',
  tertiary: 'tertiary',
  ghost: 'ghost',
  link: 'ghost',
};

const SIZES: Record<LegacySize, HeroButtonProps['size']> = {
  default: 'md',
  sm: 'sm',
  lg: 'lg',
  icon: 'md',
  'icon-sm': 'sm',
  'icon-lg': 'lg',
};

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  (
    { className, variant = 'default', size = 'default', disabled, ...props },
    ref
  ) => (
    <HeroButton
      ref={ref}
      data-slot="button"
      variant={VARIANTS[variant]}
      size={SIZES[size]}
      isIconOnly={size.startsWith('icon')}
      isDisabled={disabled}
      className={cn(
        'shrink-0 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*="size-"])]:size-4',
        variant === 'link' && 'text-accent underline-offset-4 hover:underline',
        className
      )}
      {...props}
    />
  )
);

Button.displayName = 'Button';

export { Button };
