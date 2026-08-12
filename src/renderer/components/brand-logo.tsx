import { cn } from '@/renderer/lib/utils';

interface BrandLogoProps {
  className?: string;
  compact?: boolean;
}

export default function BrandLogo({
  className,
  compact = false,
}: BrandLogoProps) {
  return (
    <span
      aria-label="Pora.take"
      className={cn(
        'text-foreground inline-flex items-baseline',
        compact ? 'text-base' : 'text-2xl',
        className
      )}
    >
      <span className="font-bold" aria-hidden="true">
        Pora
      </span>
      <svg
        viewBox="0 0 24 100"
        aria-hidden="true"
        className="mr-[0.03em] ml-[0.1em] inline-block h-[1em] w-[0.24em] overflow-visible [fill:var(--accent)] align-baseline"
      >
        <circle cx="12" cy="96" r="9" />
      </svg>
      <span className="font-semibold" aria-hidden="true">
        take
      </span>
    </span>
  );
}
