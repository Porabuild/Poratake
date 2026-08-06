export type CloudUploadState = 'idle' | 'uploading' | 'success' | 'error';

export interface CloudUploadResult {
  success: boolean;
  url?: string;
  error?: string;
}
