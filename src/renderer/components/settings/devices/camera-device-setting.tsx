import { useCallback, useRef } from 'react';
import { Button } from '@/renderer/components/ui/button';
import { Label } from '@/renderer/components/ui/label';
import { Switch } from '@/renderer/components/ui/switch';
import DeviceSelect from './device-select';
import {
  useDeviceTest,
  useMediaDevices,
} from '@/renderer/hooks/use-media-devices';
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
  const { cameras, defaultCameraId, refresh } = useMediaDevices();
  const { testing, startTest, stopTest } = useDeviceTest('camera');

  const camera = settings.recording.camera;
  const cameraRef = useRef(camera);
  cameraRef.current = camera;
  const selectedId = camera.selectedDeviceId;
  const selectedName = camera.selectedDeviceName;

  const handleSelect = useCallback(
    (device: MediaDeviceDescriptor | null) => {
      const nextCamera = {
        ...cameraRef.current,
        selectedDeviceId: device?.id ?? null,
        selectedDeviceName: device?.label ?? null,
      };
      cameraRef.current = nextCamera;
      onUpdate({
        recording: {
          ...settings.recording,
          camera: nextCamera,
        },
      });
      if (testing) {
        void startTest({
          deviceId: device?.id ?? null,
          deviceName: device?.label ?? null,
          flipped: nextCamera.flipped ?? false,
        });
      }
    },
    [onUpdate, settings.recording, testing, startTest]
  );

  const handleToggleTest = useCallback(() => {
    if (testing) {
      stopTest();
      return;
    }
    void startTest({
      deviceId: selectedId,
      deviceName: selectedName,
      flipped: camera.flipped ?? false,
    });
  }, [testing, stopTest, startTest, selectedId, selectedName, camera.flipped]);

  const handleFlippedChange = useCallback(
    (flipped: boolean) => {
      const nextCamera = { ...cameraRef.current, flipped };
      cameraRef.current = nextCamera;
      onUpdate({
        recording: {
          ...settings.recording,
          camera: nextCamera,
        },
      });
      if (testing) {
        void startTest({
          deviceId: nextCamera.selectedDeviceId,
          deviceName: nextCamera.selectedDeviceName,
          flipped,
        });
      }
    },
    [onUpdate, settings.recording, testing, startTest]
  );

  return (
    <div className="space-y-3 py-2">
      <div className="space-y-0.5">
        <Label className="text-sm">Camera</Label>
        <p className="text-xs text-muted-foreground">
          Choose which camera is used for recordings
        </p>
      </div>
      <DeviceSelect
        label="Camera"
        devices={cameras}
        selectedId={selectedId}
        selectedName={selectedName}
        defaultDeviceId={defaultCameraId}
        onSelect={handleSelect}
        onOpen={refresh}
      />
      <div className="flex items-center justify-between gap-4">
        <div className="space-y-0.5">
          <Label htmlFor="mirror-camera">Mirror camera</Label>
          <p className="text-xs text-muted-foreground">
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
          variant="secondary"
          size="sm"
          className="w-24 shrink-0"
          onClick={handleToggleTest}
        >
          {testing ? 'Stop Test' : 'Test Video'}
        </Button>
        {testing && (
          <p className="text-xs text-muted-foreground">
            The camera preview opens in a floating window — the same one shown
            while recording
          </p>
        )}
      </div>
    </div>
  );
}
