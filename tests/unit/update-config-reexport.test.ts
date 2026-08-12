import { describe, it, expect, vi } from 'vitest';

vi.mock('electron', () => ({
  app: { isPackaged: false, getPath: () => '/tmp' },
}));

describe('update/config', () => {
  it('exports the fork update destination', async () => {
    const m = await import('@/main/update/config');
    expect(m.UPDATE_OWNER).toBe('Porabuild');
    expect(m.UPDATE_REPOSITORY).toBe('Poratake');
  });
});
