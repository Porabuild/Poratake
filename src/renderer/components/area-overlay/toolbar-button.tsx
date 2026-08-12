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
        'size-8 min-w-8 rounded-3xl hover:bg-white/15 disabled:pointer-events-none disabled:opacity-35',
        className
      )}
      {...props}
      style={
        {
          '--button-fg': 'rgb(255 255 255 / 0.85)',
        } as React.CSSProperties & { '--button-fg': string }
      }
    />
  );
}
