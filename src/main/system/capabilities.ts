import type { FeatureId } from '@/types/capabilities';
import { isFeatureSupportedOn } from '@/types/capabilities';

export function isFeatureSupported(feature: FeatureId): boolean {
  return isFeatureSupportedOn(process.platform, feature);
}
