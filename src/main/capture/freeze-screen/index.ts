import { daemon } from '@/main/daemon';
import { debugLog, debugLogMs } from '@/main/utils/debug-log';
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

  debugLog('freeze-screen', 'prewarm requested');
  daemon.call('freeze-screen', 'prewarm').catch(() => {});
}

export async function freezeScreen(
  watchSpaceKey: boolean = false
): Promise<boolean> {
  if (!isSupported()) {
    return false;
  }

  const startedAt = performance.now();
  try {
    const result = (await daemon.call('freeze-screen', 'freeze', {
      watchSpaceKey,
    })) as { frozen?: boolean } | undefined;
    isFrozen = result?.frozen ?? true;
    debugLogMs('freeze-screen', `freeze (frozen=${isFrozen})`, startedAt);
    return isFrozen;
  } catch (error) {
    debugLogMs('freeze-screen', 'freeze failed', startedAt);
    console.error('Failed to freeze screen:', error);
    return false;
  }
}

export async function releaseScreen(): Promise<boolean> {
  if (!isSupported() || !isFrozen) {
    return false;
  }

  const startedAt = performance.now();
  try {
    await daemon.call('freeze-screen', 'release');
    isFrozen = false;
    debugLogMs('freeze-screen', 'release', startedAt);
    return true;
  } catch (error) {
    debugLogMs('freeze-screen', 'release failed', startedAt);
    console.error('Failed to release screen:', error);
    return false;
  }
}
