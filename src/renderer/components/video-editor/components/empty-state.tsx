import type { ReactNode } from 'react';
import { cn } from '@/renderer/lib/utils';

interface EmptyStateProps {
  message: ReactNode;
  className?: string;
}

export default function EmptyState({ message, className }: EmptyStateProps) {
  return (
    <div
      className={cn('flex h-full items-center justify-center p-4', className)}
    >
      <p className="text-center text-sm text-muted-foreground">{message}</p>
    </div>
  );
}
