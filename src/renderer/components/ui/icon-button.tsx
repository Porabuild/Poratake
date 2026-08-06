import * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '@/renderer/lib/utils';

const iconButtonVariants = cva(
  'flex items-center justify-center transition-all duration-200 disabled:cursor-not-allowed disabled:opacity-40',
  {
    variants: {
      variant: {
        default:
          'text-muted-foreground hover:bg-white/10 hover:text-foreground',
        danger: 'text-red-400 hover:bg-red-500/20 hover:text-red-300',
        success:
          'text-emerald-400 hover:bg-emerald-500/20 hover:text-emerald-300',
        warning: 'text-amber-400 hover:bg-amber-500/20 hover:text-amber-300',
      },
      size: {
        default: 'h-full w-full',
        sm: 'h-8 w-8',
        md: 'h-10 w-10',
        lg: 'h-12 w-12',
      },
      active: {
        true: '',
        false: '',
      },
    },
    compoundVariants: [
      {
        variant: 'default',
        active: true,
        className: 'bg-accent text-foreground',
      },
    ],
    defaultVariants: {
      variant: 'default',
      size: 'default',
      active: false,
    },
  }
);

interface IconButtonProps
  extends
    React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof iconButtonVariants> {
  noDrag?: boolean;
}

function IconButton({
  className,
  variant,
  size,
  active,
  noDrag = true,
  style,
  ...props
}: IconButtonProps) {
  return (
    <button
      className={cn(iconButtonVariants({ variant, size, active, className }))}
      style={
        noDrag
          ? ({ WebkitAppRegion: 'no-drag', ...style } as React.CSSProperties)
          : style
      }
      {...props}
    />
  );
}

export { IconButton, iconButtonVariants };
