import { cn } from '@/renderer/lib/utils';

export default function ToolbarButton({
  className,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type="button"
      className={cn(
        'flex h-8 items-center justify-center gap-1.5 rounded-full px-3 text-sm text-white/90 transition-colors hover:bg-white/15 hover:text-white disabled:pointer-events-none disabled:opacity-40',
        className
      )}
      {...props}
    />
  );
}
