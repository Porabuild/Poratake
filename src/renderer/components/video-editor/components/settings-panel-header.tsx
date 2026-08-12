import { Switch } from '@/renderer/components/ui/switch';

interface SettingsPanelHeaderProps {
  title: string;
  description: string;
  enabled?: boolean;
  onEnabledChange?: (enabled: boolean) => void;
  action?: React.ReactNode;
}

export default function SettingsPanelHeader({
  title,
  description,
  enabled,
  onEnabledChange,
  action,
}: SettingsPanelHeaderProps) {
  const showToggle = typeof enabled === 'boolean' && onEnabledChange;
  const showRight = showToggle || action;

  return (
    <div className={showRight ? 'flex items-center justify-between' : ''}>
      <div>
        <h3 className="text-sm font-medium">{title}</h3>
        <p className="text-muted-foreground text-xs">{description}</p>
      </div>
      {showToggle && (
        <Switch size="sm" checked={enabled} onCheckedChange={onEnabledChange} />
      )}
      {!showToggle && action}
    </div>
  );
}
