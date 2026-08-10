import { describe, it, expect } from 'vitest';

describe('Update Config', () => {
  it('uses the Poratake fork release channel', async () => {
    const { UPDATE_OWNER, UPDATE_REPOSITORY } =
      await import('@/main/update/config');

    expect(UPDATE_OWNER).toBe('SDSLeon');
    expect(UPDATE_REPOSITORY).toBe('capty');
  });
});
