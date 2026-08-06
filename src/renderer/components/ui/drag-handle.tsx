import * as React from 'react';
import { GripVertical, GripHorizontal } from 'lucide-react';
import { cn } from '@/renderer/lib/utils';

interface DragHandleProps extends React.HTMLAttributes<HTMLDivElement> {
  orientation?: 'vertical' | 'horizontal';
  iconSize?: number;
}

function DragHandle({
  className,
  orientation = 'vertical',
  iconSize = 16,
  style,
  ...props
}: DragHandleProps) {
  const Icon = orientation === 'vertical' ? GripVertical : GripHorizontal;

  return (
    <div
      className={cn(
        'text-muted-foreground flex cursor-grab items-center justify-center active:cursor-grabbing',
        className
      )}
      style={{ WebkitAppRegion: 'drag', ...style } as React.CSSProperties}
      {...props}
    >
      <Icon size={iconSize} strokeWidth={1.5} />
    </div>
  );
}

export { DragHandle };
