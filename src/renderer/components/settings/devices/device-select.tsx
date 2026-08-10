import SettingsSelect from '@/renderer/components/settings/settings-select';
import type { MediaDeviceDescriptor } from '@/types/devices';

const SYSTEM_DEFAULT_VALUE = 'system-default';

interface DeviceSelectProps {
  label: string;
  devices: MediaDeviceDescriptor[];
  selectedId: string | null;
  selectedName: string | null;
  defaultDeviceId: string | null;
  onSelect: (device: MediaDeviceDescriptor | null) => void;
  onOpen: () => void;
}

export default function DeviceSelect({
  label,
  devices,
  selectedId,
  selectedName,
  defaultDeviceId,
  onSelect,
  onOpen,
}: DeviceSelectProps) {
  const isUnavailable =
    selectedId !== null && !devices.some(device => device.id === selectedId);
  const defaultDevice =
    devices.find(device => device.id === defaultDeviceId) ?? null;
  const systemDefaultLabel = defaultDevice
    ? `System Default (${defaultDevice.label})`
    : 'System Default';
  const options = [
    { value: SYSTEM_DEFAULT_VALUE, label: systemDefaultLabel },
    ...devices.map(device => ({ value: device.id, label: device.label })),
    ...(isUnavailable
      ? [
          {
            value: selectedId,
            label: `${selectedName ?? 'Unknown device'} (unavailable)`,
          },
        ]
      : []),
  ];

  const handleChange = (value: string) => {
    if (value === SYSTEM_DEFAULT_VALUE) {
      onSelect(null);
      return;
    }
    const device = devices.find(item => item.id === value);
    if (device) onSelect(device);
  };

  return (
    <SettingsSelect
      label={label}
      options={options}
      value={selectedId ?? SYSTEM_DEFAULT_VALUE}
      onChange={handleChange}
      onOpenChange={open => {
        if (open) onOpen();
      }}
      className="w-full"
    />
  );
}
