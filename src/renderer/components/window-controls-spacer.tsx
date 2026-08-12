import { isWindowsPlatform } from '@/renderer/utils/platform';

export default function WindowControlsSpacer() {
  if (!isWindowsPlatform()) {
    return null;
  }

  return (
    <div aria-hidden className="window-controls-spacer ml-auto flex-none" />
  );
}
