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
      size="xs"
      onClick={onClick}
      className={
        className ??
        'w-full text-xs text-muted-foreground hover:text-foreground'
      }
    >
      {label}
    </Button>
  );
}
