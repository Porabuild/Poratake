import { app } from 'electron';
import { applyTitleBarAppearance } from '@/main/utils/title-bar';
import { registerSettingsIpc } from './ipc.ts';
import { registerStorageIpc } from './storage-ipc.ts';
import { applyLoginItemSetting, flushConfigFile, loadConfig } from './store.ts';
import { registerWallpaperIpc } from './wallpaper-ipc.ts';

export { createOrShowSettingsWindow } from './window';
export { migrateCloudConfig } from './migrations.ts';
export {
  getConfig,
  loadConfig,
  markOnboardingCompleted,
  markOnboardingSkipped,
  needsOnboarding,
  onConfigUpdated,
  saveConfig,
  updateConfig,
} from './store.ts';

export function init(): void {
  const config = loadConfig();
  applyTitleBarAppearance(config.appearance);
  app.on('will-quit', flushConfigFile);
  applyLoginItemSetting();
  registerSettingsIpc();
  registerWallpaperIpc();
  registerStorageIpc();
}
