import { describe, it, expect, vi } from 'vitest';
import { parseWhisperOutput } from '../../src/main/transcription/whisper-transcribe';

vi.mock('electron', () => ({
  app: {
    isPackaged: false,
    getVersion: vi.fn(() => '0.0.0'),
    getAppPath: vi.fn(() => '/mock/app'),
    getPath: vi.fn(() => '/mock/path'),
  },
}));

describe('whisper transcribe parsing', () => {
  it('uses dtw midpoints for word timing', () => {
    const output = {
      transcription: [
        {
          timestamps: { from: '00:00:00.000', to: '00:00:02.000' },
          offsets: { from: 0, to: 2000 },
          text: ' Hello world',
          tokens: [
            {
              text: ' Hello',
              offsets: { from: 0, to: 1000 },
              t_dtw: 20,
            },
            {
              text: ' world',
              offsets: { from: 1000, to: 2000 },
              t_dtw: 150,
            },
          ],
        },
      ],
    };

    const segments = parseWhisperOutput(output);
    expect(segments).toHaveLength(1);

    const words = segments[0].words ?? [];
    expect(words).toHaveLength(2);
    expect(words[0].text).toBe('Hello');
    expect(words[1].text).toBe('world');
    expect(words[0].start).toBeCloseTo(0, 4);
    expect(words[0].end).toBeCloseTo(0.85, 2);
    expect(words[1].start).toBeCloseTo(0.85, 2);
    expect(words[1].end).toBeCloseTo(2, 4);
  });

  it('falls back to token offsets when dtw is missing', () => {
    const output = {
      transcription: [
        {
          timestamps: { from: '00:00:01.000', to: '00:00:03.000' },
          offsets: { from: 1000, to: 3000 },
          text: ' Hello world',
          tokens: [
            {
              text: ' Hello',
              offsets: { from: 1000, to: 1500 },
            },
            {
              text: ' world',
              offsets: { from: 1500, to: 3000 },
            },
          ],
        },
      ],
    };

    const segments = parseWhisperOutput(output);
    const words = segments[0].words ?? [];

    expect(words).toHaveLength(2);
    expect(words[0].start).toBeCloseTo(1, 4);
    expect(words[0].end).toBeCloseTo(1.5, 4);
    expect(words[1].start).toBeCloseTo(1.5, 4);
    expect(words[1].end).toBeCloseTo(3, 4);
  });
});
