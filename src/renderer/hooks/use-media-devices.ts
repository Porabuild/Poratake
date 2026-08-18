import { useCallback, useEffect, useRef, useState } from 'react';
import type { DeviceTestTarget, MediaDeviceLists } from '@/types/devices';

const EMPTY_LISTS: MediaDeviceLists = {
  microphones: [],
  cameras: [],
  defaultMicrophoneId: null,
  defaultCameraId: null,
};

function listMediaDevices(): Promise<MediaDeviceLists> {
  return window.ipcRenderer.invoke('devices:list');
}

export function useMediaDevices() {
  const [devices, setDevices] = useState<MediaDeviceLists>(EMPTY_LISTS);
  const isMountedRef = useRef(true);
  const refreshSequenceRef = useRef(0);

  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  const refresh = useCallback(async () => {
    const sequence = ++refreshSequenceRef.current;
    try {
      const lists = await listMediaDevices();
      if (isMountedRef.current && sequence === refreshSequenceRef.current) {
        setDevices(lists);
      }
    } catch (error) {
      console.error('Failed to list media devices:', error);
    }
  }, []);

  useEffect(() => {
    const sequence = ++refreshSequenceRef.current;
    void listMediaDevices()
      .then(lists => {
        if (isMountedRef.current && sequence === refreshSequenceRef.current) {
          setDevices(lists);
        }
      })
      .catch(error => {
        console.error('Failed to list media devices:', error);
      });
  }, []);

  return { ...devices, refresh };
}

type DeviceTestKind = 'mic' | 'camera';

export function useDeviceTest(kind: DeviceTestKind) {
  const [testing, setTesting] = useState(false);
  const requestSequenceRef = useRef(0);

  const startTest = useCallback(
    async (target: DeviceTestTarget) => {
      const sequence = ++requestSequenceRef.current;
      try {
        const started = await window.ipcRenderer.invoke(
          `devices:${kind}-test:start`,
          target
        );
        if (sequence === requestSequenceRef.current) {
          setTesting(Boolean(started));
        }
      } catch (error) {
        console.error(`Failed to start ${kind} test:`, error);
        if (sequence === requestSequenceRef.current) {
          setTesting(false);
        }
      }
    },
    [kind]
  );

  const stopTest = useCallback(() => {
    requestSequenceRef.current += 1;
    setTesting(false);
    window.ipcRenderer.invoke(`devices:${kind}-test:stop`).catch(() => {});
  }, [kind]);

  useEffect(
    () => () => {
      requestSequenceRef.current += 1;
      window.ipcRenderer.invoke(`devices:${kind}-test:stop`).catch(() => {});
    },
    [kind]
  );

  return { testing, startTest, stopTest };
}
