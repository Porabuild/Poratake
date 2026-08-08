import { useCallback, useEffect, useState } from 'react';
import { Button } from '@/renderer/components/ui/button';
import { Label } from '@/renderer/components/ui/label';
import { Switch } from '@/renderer/components/ui/switch';
import DeviceSelect from './device-select';
import { useMediaDevices } from '@/renderer/hooks/use-media-devices';
import type { MediaDeviceDescriptor } from '@/types/devices';
import type { SettingsConfig } from '@/types/settings';

interface CameraDeviceSettingProps {
  settings: SettingsConfig;
  onUpdate: (updates: Partial<SettingsConfig>) => void;
}

export default function CameraDeviceSetting({
  settings,
  onUpdate,
}: CameraDeviceSettingProps) {
  const { cameras, refresh } = useMediaDevices();
  const [testing, setTesting] = useState(false);

  const camera = settings.recording.camera;
  const selectedId = camera.selectedDeviceId;
  const selectedName = camera.selectedDeviceName;

  const startTest = useCallback(
    async (
      deviceId: string | null,
      deviceName: string | null,
      flipped: boolean
    ) => {
      try {
        const started = await window.ipcRenderer.invoke(
          'devices:camera-test:start',
          { deviceId, deviceName, flipped }
        );
        setTesting(Boolean(started));
      } catch (error) {
        console.error('Failed to start camera test:', error);
        setTesting(false);
      }
    },
    []
  );

  const stopTest = useCallback(() => {
    setTesting(false);
    window.ipcRenderer.invoke('devices:camera-test:stop').catch(() => {});
  }, []);

  useEffect(() => stopTest, [stopTest]);

  const handleSelect = useCallback(
    (device: MediaDeviceDescriptor | null) => {
      onUpdate({
        recording: {
          ...settings.recording,
          camera: {
            ...camera,
            selectedDeviceId: device?.id ?? null,
            selectedDeviceName: device?.label ?? null,
          },
        },
      });
      if (testing) {
        void startTest(
          device?.id ?? null,
          device?.label ?? null,
          camera.flipped ?? false
        );
      }
    },
    [onUpdate, settings.recording, camera, testing, startTest]
  );

  const handleToggleTest = useCallback(() => {
    if (testing) {
      stopTest();
      return;
    }
    void startTest(selectedId, selectedName, camera.flipped ?? false);
  }, [testing, stopTest, startTest, selectedId, selectedName, camera.flipped]);

  const handleFlippedChange = useCallback(
    (flipped: boolean) => {
      onUpdate({
        recording: {
          ...settings.recording,
          camera: { ...camera, flipped },
        },
      });
      if (testing) {
        void startTest(selectedId, selectedName, flipped);
      }
    },
    [
      onUpdate,
      settings.recording,
      camera,
      testing,
      startTest,
      selectedId,
      selectedName,
    ]
  );

  return (
    <div className="space-y-3 py-2">
      <div className="space-y-0.5">
        <Label className="text-sm">Camera</Label>
        <p className="text-muted-foreground text-xs">
          Choose which camera is used for recordings
        </p>
      </div>
      <DeviceSelect
        devices={cameras}
        selectedId={selectedId}
        selectedName={selectedName}
        onSelect={handleSelect}
        onOpen={refresh}
      />
      <div className="flex items-center justify-between gap-4">
        <div className="space-y-0.5">
          <Label htmlFor="mirror-camera">Mirror camera</Label>
          <p className="text-muted-foreground text-xs">
            Flip the camera horizontally in previews and recordings
          </p>
        </div>
        <Switch
          id="mirror-camera"
          checked={camera.flipped ?? false}
          onCheckedChange={handleFlippedChange}
        />
      </div>
      <div className="flex items-center gap-3">
        <Button
          variant={testing ? 'secondary' : 'outline'}
          size="sm"
          className="w-24 shrink-0"
          onClick={handleToggleTest}
        >
          {testing ? 'Stop Test' : 'Test Video'}
        </Button>
        {testing && (
          <p className="text-muted-foreground text-xs">
            The camera preview opens in a floating window — the same one shown
            while recording
          </p>
        )}
      </div>
    </div>
  );
}
