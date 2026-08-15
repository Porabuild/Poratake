import { daemon } from '@/main/daemon';
import { isFeatureSupported } from '@/main/system/capabilities';

let isFrozen = false;

export function isScreenFrozen(): boolean {
  return isFrozen;
}

export function isSupported(): boolean {
  return isFeatureSupported('freeze-screen');
}

export function prewarmFreezeScreen(): void {
  if (!isSupported()) {
    return;
  }

  daemon.call('freeze-screen', 'prewarm').catch(() => {});
}

export async function freezeScreen(
  watchSpaceKey: boolean = false
): Promise<boolean> {
  if (!isSupported()) {
    return false;
  }

  try {
    const result = (await daemon.call('freeze-screen', 'freeze', {
      watchSpaceKey,
    })) as { frozen?: boolean } | undefined;
    isFrozen = result?.frozen ?? true;
    return isFrozen;
  } catch (error) {
    console.error('Failed to freeze screen:', error);
    return false;
  }
}

export async function releaseScreen(): Promise<boolean> {
  if (!isSupported() || !isFrozen) {
    return false;
  }

  try {
    await daemon.call('freeze-screen', 'release');
    isFrozen = false;
    return true;
  } catch (error) {
    console.error('Failed to release screen:', error);
    return false;
  }
}
