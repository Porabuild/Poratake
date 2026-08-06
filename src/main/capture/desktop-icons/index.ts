import { systemPreferences } from 'electron';
import type { DesktopIconsHideSource } from '@/types/desktop-icons';
import { daemon } from '@/main/daemon';

export function checkAccessibilityPermission(prompt = false): boolean {
  return systemPreferences.isTrustedAccessibilityClient(prompt);
}

const hideReasons = new Set<DesktopIconsHideSource>();
let isHidden = false;

export function areDesktopIconsHidden(): boolean {
  return isHidden;
}

export function isSupported(): boolean {
  return process.platform === 'darwin';
}

export async function hideDesktopIcons(
  source: DesktopIconsHideSource = 'menu'
): Promise<boolean> {
  if (!isSupported()) {
    return false;
  }

  hideReasons.add(source);

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
