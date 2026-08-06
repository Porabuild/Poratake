export type FirstFrameFit = 'stretch' | 'cover';

export interface FirstFrameSettings {
  enabled: boolean;
  imageData: string | null;
  fit: FirstFrameFit;
}

export const DEFAULT_FIRST_FRAME_SETTINGS: FirstFrameSettings = {
  enabled: false,
  imageData: null,
  fit: 'cover',
};
