export type UpdateStatus =
  | 'idle'
  | 'checking'
  | 'available'
  | 'downloading'
  | 'ready'
  | 'error'
  | 'unsupported'
  | 'up_to_date';

export interface UpdateState {
  status: UpdateStatus;
  currentVersion: string;
  latestVersion: string | null;
  releaseNotes: string | null;
  downloadProgress: number;
  downloadedFilePath: string | null;
  error: string | null;
}
