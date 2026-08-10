export interface ReleaseFile {
  platform: 'macos' | 'windows' | 'linux';
  arch: 'arm64' | 'x64' | 'universal';
  file_name: string;
  file_size: number;
  download_url: string;
}

export interface VersionInfo {
  version: string;
  released_at: string;
  release_notes: string;
  files: ReleaseFile[];
}

export interface LatestVersionResponse {
  version: string | null;
  released_at?: string;
  release_notes?: string;
  files?: ReleaseFile[];
}

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

export interface UpdateCheckResult {
  updateAvailable: boolean;
  latestVersion: string | null;
  releaseNotes: string | null;
  downloadUrl: string | null;
  fileName: string | null;
  fileSize: number | null;
}
