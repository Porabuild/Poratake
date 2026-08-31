import {
  PLATFORM_CAPABILITIES,
  type CapabilityTarget,
  type FeatureId,
} from './capabilities.generated';

export type { FeatureId } from './capabilities.generated';

function capabilityTarget(
  platform: string | undefined
): CapabilityTarget | undefined {
  switch (platform) {
    case 'darwin':
      return 'macos';
    case 'win32':
      return 'windows';
    default:
      return undefined;
  }
}

export function isFeatureSupportedOn(
  platform: string | undefined,
  feature: FeatureId
): boolean {
  const target = capabilityTarget(platform);
  if (!target) return false;
  const supported: readonly FeatureId[] = PLATFORM_CAPABILITIES[target];
  return supported.includes(feature);
}
