import { Button } from '@/renderer/components/ui/button';
import { cn } from '@/renderer/lib/utils';

export default function ToolbarButton({
  className,
  ...props
}: React.ComponentProps<typeof Button>) {
  return (
    <Button
      size="icon-sm"
      variant="ghost"
      className={cn(
        'size-8 min-w-8 rounded-3xl hover:bg-foreground/15 disabled:pointer-events-none disabled:opacity-35',
        className
      )}
      {...props}
      style={
        {
          '--button-fg':
            'color-mix(in oklab, var(--foreground) 85%, transparent)',
        } as React.CSSProperties & { '--button-fg': string }
      }
    />
  );
}
