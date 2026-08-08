export type ContentType = 'screenshot' | 'video';

export interface CapturePreviewParams {
  filePath: string;
  contentType: ContentType;
  thumbnailBase64: string | null;
  historyId?: string;
}

export interface PreviewDisplayInfo {
  id: number;
  label: string;
  isSelected: boolean;
}
