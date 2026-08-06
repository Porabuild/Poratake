import { describe, expect, it, vi } from 'vitest';
import { shouldIgnoreGlobalKeyboardShortcuts } from '../../../src/renderer/utils/keyboard';

describe('shouldIgnoreGlobalKeyboardShortcuts', () => {
  it('returns true for text inputs', () => {
    expect(
      shouldIgnoreGlobalKeyboardShortcuts({
        tagName: 'input',
      } as unknown as EventTarget)
    ).toBe(true);

    expect(
      shouldIgnoreGlobalKeyboardShortcuts({
        tagName: 'TEXTAREA',
      } as unknown as EventTarget)
    ).toBe(true);
  });

  it('returns true for contenteditable targets and descendants', () => {
    expect(
      shouldIgnoreGlobalKeyboardShortcuts({
        isContentEditable: true,
      } as unknown as EventTarget)
    ).toBe(true);

    const closest = vi.fn(() => ({}));

    expect(
      shouldIgnoreGlobalKeyboardShortcuts({
        closest,
      } as unknown as EventTarget)
    ).toBe(true);
    expect(closest).toHaveBeenCalledWith(
      'input, textarea, select, [contenteditable]:not([contenteditable="false"])'
    );
  });

  it('returns false for non-editable targets', () => {
    expect(shouldIgnoreGlobalKeyboardShortcuts(null)).toBe(false);
    expect(
      shouldIgnoreGlobalKeyboardShortcuts({
        tagName: 'DIV',
      } as unknown as EventTarget)
    ).toBe(false);
  });
});
