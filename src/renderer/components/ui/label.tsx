import * as React from 'react';
import { Label as HeroLabel } from '@heroui/react';

import { cn } from '@/renderer/lib/utils';

function Label({
  className,
  ...props
}: React.ComponentProps<typeof HeroLabel>) {
  return (
    <HeroLabel
      data-slot="label"
      className={cn('text-sm font-medium', className)}
      {...props}
    />
  );
}

export { Label };
