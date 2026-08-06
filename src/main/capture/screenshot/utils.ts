import { app } from 'electron';
import path from 'path';
import { getConfig } from '@/main/settings';
import { generateFilename } from '@/main/utils/filename-generator';
import { ensureDirectoryExists, isValidDirectory } from '@/main/utils/paths';
import { DEFAULT_STORAGE_CONFIG } from '@/types/settings';

export function getScreenshotsDir(): string {
  const config = getConfig();
  const customPath = config.storage?.screenshotsPath;

  if (customPath && isValidDirectory(customPath)) {
    return ensureDirectoryExists(customPath);
  }

  const picturesPath = app.getPath('pictures');
  const defaultDir = path.join(picturesPath, 'Capty');
  return ensureDirectoryExists(defaultDir);
}

export function generateScreenshotPath(): string {
  const config = getConfig();
  const pattern =
    config.storage?.namingPattern || DEFAULT_STORAGE_CONFIG.namingPattern;

  const filename = generateFilename({
    pattern,
    type: 'Screenshot',
    extension: 'png',
  });

  return path.join(getScreenshotsDir(), filename);
}

export function generateScreenshotExportName(extension = 'png'): string {
  const config = getConfig();
  const pattern =
    config.storage?.namingPattern || DEFAULT_STORAGE_CONFIG.namingPattern;

  return generateFilename({
    pattern,
    type: 'Screenshot',
    extension,
  });
}
