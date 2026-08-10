import * as React from 'react';
import { ProgressBar } from '@heroui/react';

import { cn } from '@/renderer/lib/utils';

interface ProgressProps extends Omit<
  React.ComponentProps<typeof ProgressBar>,
  'value'
> {
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
    <ProgressBar
      ref={ref}
      value={value}
      isIndeterminate={indeterminate}
      className={className}
      {...props}
    >
      <ProgressBar.Track className="h-2">
        <ProgressBar.Fill className={cn(indicatorClassName)} />
      </ProgressBar.Track>
    </ProgressBar>
  )
);

Progress.displayName = 'Progress';

export { Progress };
