import { getConfig, updateConfig } from '@/main/settings';
import { checkAccessibilityPermission, isSupported } from './index';

export function shouldHideDesktopIconsForCapture(): boolean {
  const config = getConfig();
  if (!config.screenshot.hideDesktopIcons || !isSupported()) {
    return false;
  }

  if (checkAccessibilityPermission(false)) {
    return true;
  }

  updateConfig({
    screenshot: { ...config.screenshot, hideDesktopIcons: false },
  });
  return false;
}
