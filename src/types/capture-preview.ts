export type ContentType = 'screenshot' | 'video';

export interface CapturePreviewParams {
  filePath: string;
  contentType: ContentType;
  imageUrl: string | null;
  thumbnailUrl?: string;
  historyId?: string;
}

export interface PreviewDisplayInfo {
  id: number;
  label: string;
  isSelected: boolean;
}
