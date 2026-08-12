import { cn } from '@/renderer/lib/utils';

export default function ToolbarSurface({
  className,
  ...props
}: React.ComponentProps<'div'>) {
  return (
    <div
      className={cn(
        'border-muted-foreground/35 bg-muted/95 flex items-center gap-0.5 rounded-4xl border-2 p-1 shadow-2xl backdrop-blur-xl',
        className
      )}
      {...props}
    />
  );
}
