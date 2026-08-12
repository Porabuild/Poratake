export type FeatureId =
  | 'screenshot-screen'
  | 'screenshot-area'
  | 'screenshot-window'
  | 'ocr'
  | 'qrcode'
  | 'timer-capture'
  | 'scroll-capture'
  | 'all-in-one'
  | 'recording'
  | 'video-editor'
  | 'desktop-icons'
  | 'freeze-screen'
  | 'display-selector'
  | 'print'
  | 'desktop-wallpaper'
  | 'transcription'
  | 'capture-sound';

const CROSS_PLATFORM_FEATURES: readonly FeatureId[] = [
  'screenshot-screen',
  'screenshot-area',
];

const WINDOWS_FEATURES: readonly FeatureId[] = [
  ...CROSS_PLATFORM_FEATURES,
  'screenshot-window',
  'ocr',
  'qrcode',
  'timer-capture',
  'desktop-icons',
  'display-selector',
  'desktop-wallpaper',
  'freeze-screen',
  'scroll-capture',
  'all-in-one',
  'print',
  'recording',
  'video-editor',
  'transcription',
];

export function isFeatureSupportedOn(
  platform: string | undefined,
  feature: FeatureId
): boolean {
  switch (platform) {
    case undefined:
    case 'darwin':
      return true;
    case 'win32':
      return WINDOWS_FEATURES.includes(feature);
    default:
      return CROSS_PLATFORM_FEATURES.includes(feature);
  }
}
