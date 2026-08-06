const GLOBAL_SHORTCUT_BYPASS_SELECTOR =
  'input, textarea, select, [contenteditable]:not([contenteditable="false"])';

interface ShortcutBypassTarget {
  tagName?: string;
  isContentEditable?: boolean;
  closest?: (selector: string) => unknown;
}

export function shouldIgnoreGlobalKeyboardShortcuts(
  target: EventTarget | null
): boolean {
  if (!target || typeof target !== 'object') {
    return false;
  }

  const element = target as ShortcutBypassTarget;
  const tagName = element.tagName?.toUpperCase();

  if (tagName === 'INPUT' || tagName === 'TEXTAREA' || tagName === 'SELECT') {
    return true;
  }

  if (element.isContentEditable) {
    return true;
  }

  if (typeof element.closest !== 'function') {
    return false;
  }

  return element.closest(GLOBAL_SHORTCUT_BYPASS_SELECTOR) !== null;
}
