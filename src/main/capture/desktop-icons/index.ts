import { systemPreferences } from 'electron';
import type { DesktopIconsHideSource } from '@/types/desktop-icons';
import { daemon } from '@/main/daemon';
import { isMac } from '@/main/utils/platform';
import { isFeatureSupported } from '@/main/system/capabilities';

export function checkAccessibilityPermission(prompt = false): boolean {
  if (!isMac) {
    return true;
  }
  return systemPreferences.isTrustedAccessibilityClient(prompt);
}

const hideReasons = new Map<DesktopIconsHideSource, number>();
let isHidden = false;

export function areDesktopIconsHidden(): boolean {
  return isHidden;
}

export function isSupported(): boolean {
  return isFeatureSupported('desktop-icons');
}

export async function hideDesktopIcons(
  source: DesktopIconsHideSource = 'menu'
): Promise<boolean> {
  if (!isSupported()) {
    return false;
  }

  const activeCount = hideReasons.get(source) ?? 0;
  hideReasons.set(source, source === 'capture' ? activeCount + 1 : 1);

  if (isHidden) {
    return true;
  }

  try {
    await daemon.call('desktop-helper', 'hide');
    isHidden = true;
    return true;
  } catch (error) {
    console.error('Failed to hide desktop icons:', error);
    return false;
  }
}

export async function showDesktopIcons(
  source: DesktopIconsHideSource = 'system'
): Promise<boolean> {
  if (!isSupported()) {
    return false;
  }

  if (source === 'system') {
    hideReasons.clear();
  } else if (source === 'capture') {
    const activeCount = hideReasons.get(source) ?? 0;
    if (activeCount > 1) {
      hideReasons.set(source, activeCount - 1);
    } else {
      hideReasons.delete(source);
    }
  } else {
    hideReasons.delete(source);
  }

  if (hideReasons.size > 0) {
    return true;
  }

  if (!isHidden) {
    return true;
  }

  try {
    await daemon.call('desktop-helper', 'show');
    isHidden = false;
    return true;
  } catch (error) {
    console.error('Failed to show desktop icons:', error);
    return false;
  }
}
