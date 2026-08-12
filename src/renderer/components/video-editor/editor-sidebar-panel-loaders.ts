export type SidebarTab =
  | 'cursor'
  | 'zoom'
  | 'drawing'
  | 'camera'
  | 'audio'
  | 'wallpaper'
  | 'keyboard'
  | 'subtitle'
  | 'first-frame'
  | 'export';

export const loadCursorSettingsPanel = () => import('./cursor-settings-panel');
export const loadZoomSettingsPanel = () => import('./zoom-settings-panel');
export const loadDrawingSettingsPanel = () =>
  import('./drawing-settings-panel');
export const loadCameraSettingsPanel = () => import('./camera-settings-panel');
export const loadAudioSettingsPanel = () => import('./audio-settings-panel');
export const loadWallpaperSettingsPanel = () =>
  import('./wallpaper-settings-panel');
export const loadKeyboardSettingsPanel = () =>
  import('./keyboard-settings-panel');
export const loadSubtitleSettingsPanel = () =>
  import('./subtitle-settings-panel');
export const loadFirstFrameSettingsPanel = () =>
  import('./first-frame-settings-panel');
export const loadExportSettingsPanel = () => import('./export-settings-panel');

const SIDEBAR_PANEL_LOADERS = {
  cursor: loadCursorSettingsPanel,
  zoom: loadZoomSettingsPanel,
  drawing: loadDrawingSettingsPanel,
  camera: loadCameraSettingsPanel,
  audio: loadAudioSettingsPanel,
  wallpaper: loadWallpaperSettingsPanel,
  keyboard: loadKeyboardSettingsPanel,
  subtitle: loadSubtitleSettingsPanel,
  'first-frame': loadFirstFrameSettingsPanel,
  export: loadExportSettingsPanel,
} satisfies Record<SidebarTab, () => Promise<unknown>>;

export function preloadEditorSidebarTab(tab: SidebarTab): void {
  void SIDEBAR_PANEL_LOADERS[tab]().catch(() => {});
}
