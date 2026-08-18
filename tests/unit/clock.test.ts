import { describe, it, expect } from 'vitest';
import { formatClock } from '@/main/utils/clock';

describe('formatClock', () => {
  it('formats a wall-clock timestamp with millisecond precision', () => {
    expect(formatClock()).toMatch(/^\d{2}:\d{2}:\d{2}\.\d{3}$/);
  });
});
