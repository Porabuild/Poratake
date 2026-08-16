import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

describe('settings registry search', () => {
  beforeAll(() => {
    vi.stubGlobal('window', { appPlatform: 'darwin' });
  });

  afterAll(() => {
    vi.unstubAllGlobals();
  });

  it('matches shortcut labels, sections, and keywords using every term', async () => {
    const { getItemsByCategory, matchesSettingsQuery } =
      await import('@/renderer/components/settings/settings-registry');
    const shortcuts = getItemsByCategory('shortcuts');
    const captureArea = shortcuts.find(
      item => item.id === 'shortcuts.screenshot.area'
    );

    expect(captureArea).toBeDefined();
    expect(matchesSettingsQuery(captureArea!, 'area screenshot')).toBe(true);
    expect(matchesSettingsQuery(captureArea!, 'area video')).toBe(false);
  });

  it('exposes the enabled-by-default All-in-One memory setting', async () => {
    const { DEFAULT_SETTINGS } = await import('@/types/settings');
    const { getItemsByCategory } =
      await import('@/renderer/components/settings/settings-registry');
    const item = getItemsByCategory('general').find(
      candidate => candidate.id === 'allInOne.rememberChoices'
    );

    expect(item?.type).toBe('switch');
    if (!item || item.type !== 'switch') return;

    expect(item.getValue(DEFAULT_SETTINGS)).toBe(true);
    expect(item.setValue(DEFAULT_SETTINGS, false)).toEqual({
      allInOne: {
        ...DEFAULT_SETTINGS.allInOne,
        rememberChoices: false,
      },
    });
  });
});
