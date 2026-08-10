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
        'size-8 min-w-8 rounded-lg text-white/85 hover:bg-white/15 hover:text-white disabled:pointer-events-none disabled:opacity-35',
        className
      )}
      {...props}
    />
  );
}
