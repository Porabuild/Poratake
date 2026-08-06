import { useEffect, useRef } from 'react';
import { matchesAccelerator } from '@/renderer/utils/shortcuts';
import { shouldIgnoreGlobalKeyboardShortcuts } from '@/renderer/utils/keyboard';

interface UseAcceleratorShortcutProps {
  accelerator: string | undefined;
  onTrigger: () => void;
}

export function useAcceleratorShortcut({
  accelerator,
  onTrigger,
}: UseAcceleratorShortcutProps): void {
  const onTriggerRef = useRef(onTrigger);

  useEffect(() => {
    onTriggerRef.current = onTrigger;
  });

  useEffect(() => {
    if (!accelerator) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.repeat) return;
      if (shouldIgnoreGlobalKeyboardShortcuts(event.target)) return;
      if (!matchesAccelerator(event, accelerator)) return;

      event.preventDefault();
      onTriggerRef.current();
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [accelerator]);
}
