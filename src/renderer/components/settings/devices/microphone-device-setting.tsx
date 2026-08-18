import { useCallback, useEffect, useState } from 'react';
import { Button } from '@/renderer/components/ui/button';
import { Label } from '@/renderer/components/ui/label';
import DeviceSelect from './device-select';
import LevelMeter from './level-meter';
import {
  useDeviceTest,
  useMediaDevices,
} from '@/renderer/hooks/use-media-devices';
import type { MediaDeviceDescriptor } from '@/types/devices';
import type { SettingsConfig } from '@/types/settings';

interface MicrophoneDeviceSettingProps {
  settings: SettingsConfig;
  onUpdate: (updates: Partial<SettingsConfig>) => void;
}

export default function MicrophoneDeviceSetting({
  settings,
  onUpdate,
}: MicrophoneDeviceSettingProps) {
  const { microphones, defaultMicrophoneId, refresh } = useMediaDevices();
  const { testing, startTest, stopTest } = useDeviceTest('mic');
  const [level, setLevel] = useState(0);

  const selectedId = settings.recording.selectedMicId;
  const selectedName = settings.recording.selectedMicName;

  useEffect(() => {
    if (!testing) return;

    const handleLevel = (_event: unknown, value: number) => setLevel(value);
    window.ipcRenderer.on('devices:mic-test:level', handleLevel);
    return () => {
      window.ipcRenderer.off('devices:mic-test:level', handleLevel);
    };
  }, [testing]);

  const handleSelect = useCallback(
    (device: MediaDeviceDescriptor | null) => {
      onUpdate({
        recording: {
          ...settings.recording,
          selectedMicId: device?.id ?? null,
          selectedMicName: device?.label ?? null,
        },
      });
      if (testing) {
        void startTest({
          deviceId: device?.id ?? null,
          deviceName: device?.label ?? null,
        });
      }
    },
    [onUpdate, settings.recording, testing, startTest]
  );

  const handleToggleTest = useCallback(() => {
    if (testing) {
      stopTest();
      setLevel(0);
      return;
    }
    void startTest({ deviceId: selectedId, deviceName: selectedName });
  }, [testing, stopTest, startTest, selectedId, selectedName]);

  return (
    <div className="space-y-3 py-2">
      <div className="space-y-0.5">
        <Label className="text-sm">Microphone</Label>
        <p className="text-xs text-muted-foreground">
          Choose which microphone is used for recordings
        </p>
      </div>
      <DeviceSelect
        label="Microphone"
        devices={microphones}
        selectedId={selectedId}
        selectedName={selectedName}
        defaultDeviceId={defaultMicrophoneId}
        onSelect={handleSelect}
        onOpen={refresh}
      />
      <div className="flex items-center gap-3">
        <Button
          variant="secondary"
          size="sm"
          className="w-24 shrink-0"
          onClick={handleToggleTest}
        >
          {testing ? 'Stop Test' : 'Mic Test'}
        </Button>
        <LevelMeter level={level} active={testing} />
      </div>
      {testing && (
        <p className="text-xs text-muted-foreground">
          Speak into your microphone — the meter should react to your voice
        </p>
      )}
    </div>
  );
}
