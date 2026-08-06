import * as React from 'react';

import { cn } from '@/renderer/lib/utils';

interface ProgressProps extends React.HTMLAttributes<HTMLDivElement> {
  value?: number;
  indicatorClassName?: string;
  indeterminate?: boolean;
}

const Progress = React.forwardRef<HTMLDivElement, ProgressProps>(
  (
    {
      className,
      value = 0,
      indicatorClassName,
      indeterminate = false,
      ...props
    },
    ref
  ) => (
    <div
      ref={ref}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={indeterminate ? undefined : value}
      className={cn(
        'bg-primary/20 relative h-2 w-full overflow-hidden rounded-full',
        className
      )}
      {...props}
    >
      <div
        className={cn(
          'bg-primary h-full transition-all duration-300 ease-out',
          indeterminate && 'progress-indeterminate transition-none',
          indicatorClassName
        )}
        style={
          indeterminate
            ? undefined
            : { width: `${Math.max(0, Math.min(100, value))}%` }
        }
      />
    </div>
  )
);
Progress.displayName = 'Progress';

export { Progress };
