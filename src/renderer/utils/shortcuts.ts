const MAC_MODIFIER_SYMBOLS: Record<string, string> = {
  COMMANDORCONTROL: '⌘',
  CMDORCTRL: '⌘',
  COMMAND: '⌘',
  CMD: '⌘',
  META: '⌘',
  SUPER: '⌘',
  CONTROL: '⌃',
  CTRL: '⌃',
  ALT: '⌥',
  OPTION: '⌥',
  SHIFT: '⇧',
};

const PC_MODIFIER_SYMBOLS: Record<string, string> = {
  COMMANDORCONTROL: 'Ctrl',
  CMDORCTRL: 'Ctrl',
  COMMAND: 'Win',
  CMD: 'Win',
  META: 'Win',
  SUPER: 'Win',
  CONTROL: 'Ctrl',
  CTRL: 'Ctrl',
  ALT: 'Alt',
  OPTION: 'Alt',
  SHIFT: 'Shift',
};

function getModifierSymbols(): Record<string, string> {
  const isMac =
    typeof window === 'undefined' ||
    window.appPlatform === undefined ||
    window.appPlatform === 'darwin';
  return isMac ? MAC_MODIFIER_SYMBOLS : PC_MODIFIER_SYMBOLS;
}

export function getPrimaryModifierLabel(): string {
  return getModifierSymbols().COMMANDORCONTROL;
}

const EVENT_KEY_BY_TOKEN: Record<string, string> = {
  SPACE: ' ',
  UP: 'ARROWUP',
  DOWN: 'ARROWDOWN',
  LEFT: 'ARROWLEFT',
  RIGHT: 'ARROWRIGHT',
  RETURN: 'ENTER',
};

interface ParsedAccelerator {
  modifiers: string[];
  key: string | null;
}

function parseAccelerator(accelerator: string): ParsedAccelerator {
  const modifiers: string[] = [];
  let key: string | null = null;

  for (const part of accelerator.split('+')) {
    const token = part.trim();
    if (!token) continue;

    if (MAC_MODIFIER_SYMBOLS[token.toUpperCase()]) {
      modifiers.push(token.toUpperCase());
      continue;
    }

    key = token;
  }

  return { modifiers, key };
}

export function formatAccelerator(accelerator: string, separator = ''): string {
  if (!accelerator) return '';

  const symbols = getModifierSymbols();
  const { modifiers, key } = parseAccelerator(accelerator);
  const parts = modifiers.map(modifier => symbols[modifier]);

  if (key) {
    parts.push(key);
  }

  const isSymbolic = symbols === MAC_MODIFIER_SYMBOLS;
  if (!isSymbolic && !separator) {
    return parts.join('+');
  }

  return parts.join(separator);
}

interface AcceleratorMatchEvent {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

export function matchesAccelerator(
  event: AcceleratorMatchEvent,
  accelerator: string
): boolean {
  if (!accelerator) return false;

  const { modifiers, key } = parseAccelerator(accelerator);
  if (!key || modifiers.length === 0) return false;

  const expectsMetaOrControl = modifiers.some(
    modifier => modifier === 'COMMANDORCONTROL' || modifier === 'CMDORCTRL'
  );
  const expectsMeta = modifiers.some(
    modifier =>
      modifier === 'COMMAND' ||
      modifier === 'CMD' ||
      modifier === 'META' ||
      modifier === 'SUPER'
  );
  const expectsControl = modifiers.some(
    modifier => modifier === 'CONTROL' || modifier === 'CTRL'
  );
  const expectsShift = modifiers.includes('SHIFT');
  const expectsAlt = modifiers.some(
    modifier => modifier === 'ALT' || modifier === 'OPTION'
  );

  if (event.shiftKey !== expectsShift) return false;
  if (event.altKey !== expectsAlt) return false;

  if (expectsMetaOrControl && !event.metaKey && !event.ctrlKey) return false;

  if (!expectsMetaOrControl && event.metaKey !== expectsMeta) return false;

  if (!expectsMetaOrControl && event.ctrlKey !== expectsControl) return false;

  const upperKey = key.toUpperCase();

  return event.key.toUpperCase() === (EVENT_KEY_BY_TOKEN[upperKey] ?? upperKey);
}
