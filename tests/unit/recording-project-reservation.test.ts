import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import path from 'path';
import { describe, expect, it } from 'vitest';
import { createProjectFolder } from '@/main/capture/video/recording-project';

describe('recording project reservation', () => {
  it('does not reuse an existing recording directory', () => {
    const directory = mkdtempSync(path.join(tmpdir(), 'poratake-recording-'));
    try {
      const project = path.join(directory, 'Capture.poratake');
      const recording = createProjectFolder(project);
      writeFileSync(recording, 'existing recording');

      expect(() => createProjectFolder(project)).toThrow(/EEXIST/);
      expect(readFileSync(recording, 'utf8')).toBe('existing recording');
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });
});
