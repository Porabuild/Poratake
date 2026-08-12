import { Dropdown } from '@heroui/react';
import { AppWindow, ChevronDown, Monitor, SquareDashed } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { AllInOneCaptureTarget } from '@/types/area-overlay';

const CAPTURE_TARGETS: readonly {
  id: AllInOneCaptureTarget;
  label: string;
  icon: LucideIcon;
}[] = [
  { id: 'area', label: 'Area', icon: SquareDashed },
  { id: 'window', label: 'Window', icon: AppWindow },
  { id: 'screen', label: 'Full screen', icon: Monitor },
];

const TRIGGER_CLASSES =
  'inline-flex h-8 w-12 min-w-12 flex-row items-center justify-center gap-1 rounded-3xl px-1.5 whitespace-nowrap text-white/85 outline-none [--button-fg:rgb(255_255_255/0.85)] hover:bg-white/15 hover:text-white hover:[--button-fg:white] disabled:pointer-events-none disabled:opacity-35';

export default function CaptureTargetMenu({
  target,
  onSelect,
}: {
  target: AllInOneCaptureTarget;
  onSelect: (target: AllInOneCaptureTarget) => void;
}) {
  const active =
    CAPTURE_TARGETS.find(option => option.id === target) ?? CAPTURE_TARGETS[0];
  const ActiveIcon = active.icon;

  return (
    <Dropdown>
      <Dropdown.Trigger aria-label="Capture target" className={TRIGGER_CLASSES}>
        <ActiveIcon className="size-4" />
        <ChevronDown className="size-3" />
      </Dropdown.Trigger>
      <Dropdown.Popover placement="bottom" className="min-w-40">
        <Dropdown.Menu
          aria-label="Capture targets"
          selectionMode="single"
          selectedKeys={[target]}
          onAction={key => onSelect(key as AllInOneCaptureTarget)}
        >
          {CAPTURE_TARGETS.map(({ id, label, icon: Icon }) => (
            <Dropdown.Item key={id} id={id} textValue={label}>
              <Icon className="size-4" />
              <span className="min-w-0 flex-1 truncate">{label}</span>
              <Dropdown.ItemIndicator className="text-foreground" />
            </Dropdown.Item>
          ))}
        </Dropdown.Menu>
      </Dropdown.Popover>
    </Dropdown>
  );
}
