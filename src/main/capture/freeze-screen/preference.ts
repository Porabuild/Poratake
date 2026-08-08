import { getConfig } from '@/main/settings';
import { isFeatureSupported } from '@/main/system/capabilities';

export function isFreezeScreenEnabled(): boolean {
  return (
    getConfig().screenshot.freezeScreen && isFeatureSupported('freeze-screen')
  );
}
