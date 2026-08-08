import { useCallback, useEffect, useRef, useState } from 'react';
import type { MediaDeviceLists } from '@/types/devices';

const EMPTY_LISTS: MediaDeviceLists = { microphones: [], cameras: [] };

export function useMediaDevices() {
  const [devices, setDevices] = useState<MediaDeviceLists>(EMPTY_LISTS);
  const isMountedRef = useRef(true);

  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  const refresh = useCallback(async () => {
    try {
      const lists: MediaDeviceLists =
        await window.ipcRenderer.invoke('devices:list');
      if (isMountedRef.current) setDevices(lists);
    } catch (error) {
      console.error('Failed to list media devices:', error);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { ...devices, refresh };
}
