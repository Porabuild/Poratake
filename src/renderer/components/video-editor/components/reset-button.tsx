import { Button } from '@/renderer/components/ui/button';

interface ResetButtonProps {
  onClick: () => void;
  label?: string;
  className?: string;
}

export default function ResetButton({
  onClick,
  label = 'Reset to defaults',
  className,
}: ResetButtonProps) {
  return (
    <Button
      variant="ghost"
      size="sm"
      onClick={onClick}
      className={
        className ??
        'text-muted-foreground hover:text-foreground w-full text-xs'
      }
    >
      {label}
    </Button>
  );
}
