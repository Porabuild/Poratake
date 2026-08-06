import { useCallback } from 'react';
import type { FirstFrameSettings, FirstFrameFit } from '@/types/first-frame';
import type { SliceController } from './use-editor-history';

interface UseFirstFrameReturn {
  firstFrame: FirstFrameSettings;
  setFirstFrame: (settings: FirstFrameSettings) => void;
  setEnabled: (enabled: boolean) => void;
  setImageData: (imageData: string | null) => void;
  setFit: (fit: FirstFrameFit) => void;
}

export function useFirstFrame(
  slice: SliceController<FirstFrameSettings>
): UseFirstFrameReturn {
  const { value: firstFrame, set } = slice;

  const setFirstFrame = useCallback(
    (settings: FirstFrameSettings) => set(settings),
    [set]
  );

  const setEnabled = useCallback(
    (enabled: boolean) => set(prev => ({ ...prev, enabled })),
    [set]
  );

  const setImageData = useCallback(
    (imageData: string | null) =>
      set(prev => ({
        ...prev,
        imageData,
        enabled: imageData !== null,
      })),
    [set]
  );

  const setFit = useCallback(
    (fit: FirstFrameFit) => set(prev => ({ ...prev, fit })),
    [set]
  );

  return {
    firstFrame,
    setFirstFrame,
    setEnabled,
    setImageData,
    setFit,
  };
}
