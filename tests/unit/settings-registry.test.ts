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
});
