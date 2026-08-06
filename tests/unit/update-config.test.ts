import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock env module - needs getter for dynamic values
let mockIsProduction = false;
vi.mock('@/main/utils/env', () => ({
  get isProduction() {
    return mockIsProduction;
  },
}));

// Mock paths module
vi.mock('@/main/utils/paths', () => ({
  getConfigDir: vi.fn(() => '/mock/config'),
  getLicenseFilePath: vi.fn(() => '/mock/config/license.json'),
  getTrialFilePath: vi.fn(() => '/mock/config/trial.json'),
}));

describe('Update Config', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockIsProduction = false;
  });

  afterEach(() => {
    vi.resetModules();
  });

  describe('API_URL', () => {
    it('should export production URL when in production', async () => {
      mockIsProduction = true;
      const { API_URL } = await import('@/main/update/config');

      expect(API_URL).toBe('https://capty.app');
    });

    it('should export development URL when not in production', async () => {
      mockIsProduction = false;
      const { API_URL } = await import('@/main/update/config');

      expect(API_URL).toBe('https://capty.test');
    });
  });
});
