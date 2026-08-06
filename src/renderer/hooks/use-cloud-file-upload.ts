import { useCallback, useRef, useState } from 'react';
import type { CloudUploadResult, CloudUploadState } from '@/types/cloud';

interface UseCloudFileUploadReturn {
  uploadState: CloudUploadState;
  isUploading: boolean;
  upload: () => Promise<void>;
}

export function useCloudFileUpload(filePath: string): UseCloudFileUploadReturn {
  const [uploadState, setUploadState] = useState<CloudUploadState>('idle');
  const isUploadPending = useRef(false);

  const upload = useCallback(async () => {
    if (isUploadPending.current) return;

    isUploadPending.current = true;
    setUploadState('uploading');

    try {
      const isConfigured = (await window.ipcRenderer.invoke(
        'cloud:isConfigured'
      )) as boolean;

      if (!isConfigured) {
        window.ipcRenderer.send('open-settings', 'cloud');
        setUploadState('idle');
        return;
      }

      const result = (await window.ipcRenderer.invoke(
        'cloud:uploadFile',
        filePath
      )) as CloudUploadResult;

      setUploadState(result.success ? 'success' : 'idle');
    } catch (error) {
      console.error('Failed to upload to cloud:', error);
      setUploadState('idle');
    } finally {
      isUploadPending.current = false;
    }
  }, [filePath]);

  return { uploadState, isUploading: uploadState === 'uploading', upload };
}
