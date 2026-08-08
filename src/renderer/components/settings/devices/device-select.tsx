import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/renderer/components/ui/select';
import type { MediaDeviceDescriptor } from '@/types/devices';

const SYSTEM_DEFAULT_VALUE = 'system-default';

interface DeviceSelectProps {
  devices: MediaDeviceDescriptor[];
  selectedId: string | null;
  selectedName: string | null;
  onSelect: (device: MediaDeviceDescriptor | null) => void;
  onOpen: () => void;
}

export default function DeviceSelect({
  devices,
  selectedId,
  selectedName,
  onSelect,
  onOpen,
}: DeviceSelectProps) {
  const isUnavailable =
    selectedId !== null && !devices.some(device => device.id === selectedId);

  const handleChange = (value: string) => {
    if (value === SYSTEM_DEFAULT_VALUE) {
      onSelect(null);
      return;
    }
    const device = devices.find(item => item.id === value);
    if (device) onSelect(device);
  };

  return (
    <Select
      value={selectedId ?? SYSTEM_DEFAULT_VALUE}
      onValueChange={handleChange}
      onOpenChange={open => {
        if (open) onOpen();
      }}
    >
      <SelectTrigger className="w-full">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={SYSTEM_DEFAULT_VALUE}>System Default</SelectItem>
        {devices.map(device => (
          <SelectItem key={device.id} value={device.id}>
            {device.label}
          </SelectItem>
        ))}
        {isUnavailable && (
          <SelectItem value={selectedId}>
            {selectedName ?? 'Unknown device'} (unavailable)
          </SelectItem>
        )}
      </SelectContent>
    </Select>
  );
}
