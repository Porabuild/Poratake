import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

const SOURCE_EXTENSIONS = new Set([
  '.css',
  '.html',
  '.js',
  '.jsx',
  '.rs',
  '.swift',
  '.ts',
  '.tsx',
]);
const FORBIDDEN_POINTER_PATTERNS = [
  /\bcursor-pointer\b/i,
  /\bcursor-hand\b/i,
  /\bcursor\s*:\s*(?:pointer|hand)\b/i,
  /\bstyle\.cursor\s*=\s*['"](?:pointer|hand)['"]/i,
];

function getSourceFiles(directory: string): string[] {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const filePath = path.join(directory, entry.name);
    if (entry.isDirectory()) return getSourceFiles(filePath);
    return SOURCE_EXTENSIONS.has(path.extname(entry.name)) ? [filePath] : [];
  });
}

describe('desktop cursor policy', () => {
  it('keeps clickable renderer UI on the desktop arrow cursor', () => {
    const rendererRoot = path.join(process.cwd(), 'src', 'renderer');
    const rendererFiles = [
      ...getSourceFiles(rendererRoot),
      path.join(process.cwd(), 'index.html'),
      path.join(process.cwd(), 'history.html'),
    ];
    const violations = rendererFiles.flatMap(filePath => {
      const source = fs.readFileSync(filePath, 'utf8');
      return FORBIDDEN_POINTER_PATTERNS.some(pattern => pattern.test(source))
        ? [path.relative(process.cwd(), filePath)]
        : [];
    });

    expect(violations).toEqual([]);
  });

  it('overrides browser and HeroUI interactive cursors globally', () => {
    const baseStyles = fs.readFileSync(
      path.join(process.cwd(), 'src', 'renderer', 'styles', 'base.css'),
      'utf8'
    );

    expect(baseStyles).toContain('--cursor-interactive: default');
    expect(baseStyles).toMatch(/a,\s*button,[\s\S]*?cursor:\s*default;/);
  });

  it('keeps native controls off the pointing-hand cursor', () => {
    const macRoot = path.join(process.cwd(), 'src', 'main', 'daemon');
    const windowsRoot = path.join(
      process.cwd(),
      'src',
      'main',
      'daemon-win',
      'src'
    );
    const macViolations = getSourceFiles(macRoot).filter(filePath => {
      if (
        filePath.endsWith(
          path.join('ScreenRecorder', 'Trackers', 'CursorTypeDetector.swift')
        )
      ) {
        return false;
      }
      return /\b(?:NSCursor\s*\.\s*)?pointingHand\b/.test(
        fs.readFileSync(filePath, 'utf8')
      );
    });
    const windowsViolations = getSourceFiles(windowsRoot).filter(filePath => {
      if (filePath.endsWith(path.join('modules', 'recording_input.rs'))) {
        return false;
      }
      return /\bIDC_HAND\b/.test(fs.readFileSync(filePath, 'utf8'));
    });

    expect([...macViolations, ...windowsViolations]).toEqual([]);
  });
});
