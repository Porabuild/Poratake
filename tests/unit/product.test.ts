import { describe, expect, it } from 'vitest';

import {
  ISSUES_URL,
  PORABUILD_URL,
  PRODUCT_HOMEPAGE,
  SOURCE_URL,
  UPDATE_OWNER,
  UPDATE_REPOSITORY,
  UPSTREAM_URL,
} from '@/types/product';

describe('product destinations', () => {
  it('points source, homepage, issues, and updates at Porabuild/Poratake', () => {
    expect(UPDATE_OWNER).toBe('Porabuild');
    expect(UPDATE_REPOSITORY).toBe('Poratake');
    expect(SOURCE_URL).toBe('https://github.com/Porabuild/Poratake');
    expect(ISSUES_URL).toBe('https://github.com/Porabuild/Poratake/issues');
    expect(PORABUILD_URL).toBe('https://porabuild.com');
    expect(PRODUCT_HOMEPAGE).toBe('https://porabuild.com/poratake');
    expect(UPSTREAM_URL).toBe('https://github.com/capty-app/capty');
  });
});
