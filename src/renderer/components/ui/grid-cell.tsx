import * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '@/renderer/lib/utils';

const gridCellVariants = cva('flex items-center justify-center', {
  variants: {
    border: {
      right: 'border-r border-border',
      bottom: 'border-b border-border',
      left: 'border-l border-border',
      top: 'border-t border-border',
      both: 'border-r border-b border-border',
      none: '',
    },
  },
  defaultVariants: {
    border: 'none',
  },
});

interface GridCellProps
  extends
    React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof gridCellVariants> {}

function GridCell({ className, border, ...props }: GridCellProps) {
  return (
    <div className={cn(gridCellVariants({ border, className }))} {...props} />
  );
}

export { GridCell, gridCellVariants };
