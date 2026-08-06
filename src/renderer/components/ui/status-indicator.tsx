import * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '@/renderer/lib/utils';

const statusIndicatorVariants = cva('rounded-full', {
  variants: {
    status: {
      ready: 'bg-blue-500 shadow-[0_0_10px_rgba(59,130,246,0.6)]',
      active: 'bg-red-500 shadow-[0_0_10px_rgba(239,68,68,0.6)]',
      paused: 'bg-amber-400 shadow-[0_0_10px_rgba(251,191,36,0.6)]',
      success: 'bg-emerald-500 shadow-[0_0_10px_rgba(16,185,129,0.6)]',
      error: 'bg-red-600 shadow-[0_0_10px_rgba(220,38,38,0.6)]',
      idle: 'bg-muted-foreground',
    },
    size: {
      sm: 'h-2 w-2',
      default: 'h-2.5 w-2.5',
      lg: 'h-3 w-3',
    },
    pulse: {
      true: 'animate-pulse',
      false: '',
    },
  },
  defaultVariants: {
    status: 'idle',
    size: 'default',
    pulse: false,
  },
});

interface StatusIndicatorProps
  extends
    React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof statusIndicatorVariants> {}

function StatusIndicator({
  className,
  status,
  size,
  pulse,
  ...props
}: StatusIndicatorProps) {
  return (
    <div
      className={cn(
        statusIndicatorVariants({ status, size, pulse, className })
      )}
      {...props}
    />
  );
}

export { StatusIndicator, statusIndicatorVariants };
