import { useCallback, useEffect, useState } from 'react';
import { Button } from '@/renderer/components/ui/button';
import { Label } from '@/renderer/components/ui/label';
import DeviceSelect from './device-select';
import LevelMeter from './level-meter';
import { useMediaDevices } from '@/renderer/hooks/use-media-devices';
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
  const { microphones, refresh } = useMediaDevices();
  const [testing, setTesting] = useState(false);
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

  const startTest = useCallback(
    async (deviceId: string | null, deviceName: string | null) => {
      try {
        const started = await window.ipcRenderer.invoke(
          'devices:mic-test:start',
          { deviceId, deviceName }
        );
        setTesting(Boolean(started));
      } catch (error) {
        console.error('Failed to start mic test:', error);
        setTesting(false);
      }
    },
    []
  );

  const stopTest = useCallback(() => {
    setTesting(false);
    setLevel(0);
    window.ipcRenderer.invoke('devices:mic-test:stop').catch(() => {});
  }, []);

  useEffect(() => stopTest, [stopTest]);

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
        void startTest(device?.id ?? null, device?.label ?? null);
      }
    },
    [onUpdate, settings.recording, testing, startTest]
  );

  const handleToggleTest = useCallback(() => {
    if (testing) {
      stopTest();
      return;
    }
    void startTest(selectedId, selectedName);
  }, [testing, stopTest, startTest, selectedId, selectedName]);

  return (
    <div className="space-y-3 py-2">
      <div className="space-y-0.5">
        <Label className="text-sm">Microphone</Label>
        <p className="text-muted-foreground text-xs">
          Choose which microphone is used for recordings
        </p>
      </div>
      <DeviceSelect
        devices={microphones}
        selectedId={selectedId}
        selectedName={selectedName}
        onSelect={handleSelect}
        onOpen={refresh}
      />
      <div className="flex items-center gap-3">
        <Button
          variant={testing ? 'secondary' : 'outline'}
          size="sm"
          className="w-24 shrink-0"
          onClick={handleToggleTest}
        >
          {testing ? 'Stop Test' : 'Mic Test'}
        </Button>
        <LevelMeter level={level} active={testing} />
      </div>
      {testing && (
        <p className="text-muted-foreground text-xs">
          Speak into your microphone — the meter should react to your voice
        </p>
      )}
    </div>
  );
}
