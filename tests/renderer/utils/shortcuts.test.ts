import { describe, expect, it } from 'vitest';
import {
  formatAccelerator,
  matchesAccelerator,
} from '../../../src/renderer/utils/shortcuts';

interface FakeKeyboardEvent {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

function keyEvent(
  key: string,
  modifiers: Partial<Omit<FakeKeyboardEvent, 'key'>> = {}
): FakeKeyboardEvent {
  return {
    key,
    metaKey: false,
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    ...modifiers,
  };
}

describe('formatAccelerator', () => {
  it('renders modifiers as symbols without a separator by default', () => {
    expect(formatAccelerator('Command+Shift+U')).toBe('⌘⇧U');
  });

  it('supports a custom separator', () => {
    expect(formatAccelerator('Command+Shift+3', ' ')).toBe('⌘ ⇧ 3');
  });

  it('maps every supported modifier alias', () => {
    expect(formatAccelerator('CommandOrControl+A')).toBe('⌘A');
    expect(formatAccelerator('Control+Alt+B')).toBe('⌃⌥B');
    expect(formatAccelerator('Cmd+Option+C')).toBe('⌘⌥C');
  });

  it('keeps the original casing of named keys', () => {
    expect(formatAccelerator('Command+Space', ' ')).toBe('⌘ Space');
  });

  it('returns an empty string for an empty accelerator', () => {
    expect(formatAccelerator('')).toBe('');
  });
});

describe('matchesAccelerator', () => {
  it('matches the exact modifier combination', () => {
    expect(
      matchesAccelerator(
        keyEvent('U', { metaKey: true, shiftKey: true }),
        'Command+Shift+U'
      )
    ).toBe(true);
  });

  it('is case insensitive on the key', () => {
    expect(
      matchesAccelerator(
        keyEvent('u', { metaKey: true, shiftKey: true }),
        'Command+Shift+U'
      )
    ).toBe(true);
  });

  it('rejects a missing modifier', () => {
    expect(
      matchesAccelerator(keyEvent('U', { metaKey: true }), 'Command+Shift+U')
    ).toBe(false);
  });

  it('rejects an extra modifier', () => {
    expect(
      matchesAccelerator(
        keyEvent('U', { metaKey: true, shiftKey: true, altKey: true }),
        'Command+Shift+U'
      )
    ).toBe(false);
  });

  it('rejects a different key with the same modifiers', () => {
    expect(
      matchesAccelerator(
        keyEvent('S', { metaKey: true, shiftKey: true }),
        'Command+Shift+U'
      )
    ).toBe(false);
  });

  it('does not confuse Command with Control', () => {
    expect(
      matchesAccelerator(
        keyEvent('U', { ctrlKey: true, shiftKey: true }),
        'Command+Shift+U'
      )
    ).toBe(false);
  });

  it('accepts either Command or Control for CommandOrControl', () => {
    expect(
      matchesAccelerator(keyEvent('U', { metaKey: true }), 'CommandOrControl+U')
    ).toBe(true);
    expect(
      matchesAccelerator(keyEvent('U', { ctrlKey: true }), 'CommandOrControl+U')
    ).toBe(true);
  });

  it('matches named keys against their event key', () => {
    expect(
      matchesAccelerator(keyEvent(' ', { metaKey: true }), 'Command+Space')
    ).toBe(true);
    expect(
      matchesAccelerator(keyEvent('ArrowUp', { metaKey: true }), 'Command+Up')
    ).toBe(true);
  });

  it('never matches an empty or modifier-less accelerator', () => {
    expect(matchesAccelerator(keyEvent('U', { metaKey: true }), '')).toBe(
      false
    );
    expect(matchesAccelerator(keyEvent('U'), 'U')).toBe(false);
  });
});
