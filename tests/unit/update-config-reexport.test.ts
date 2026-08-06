import { describe, it, expect, vi } from 'vitest';

vi.mock('electron', () => ({
  app: { isPackaged: false, getPath: () => '/tmp' },
}));

describe('update/config', () => {
  it('re-exports API_URL', async () => {
    const m = await import('@/main/update/config');
    expect(m.API_URL).toBeDefined();
    expect(typeof m.API_URL).toBe('string');
  });
});
