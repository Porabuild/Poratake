import { app } from 'electron';
import fs from 'fs';
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
  const defaultDir = path.join(picturesPath, 'Poratake');
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

export async function publishScreenshot(
  temporaryPath: string,
  outputPath: string
): Promise<string> {
  const { dir, name, ext } = path.parse(outputPath);
  let suffix = 0;
  let copy = false;

  while (true) {
    const destination = suffix
      ? path.join(dir, `${name} (${suffix})${ext}`)
      : outputPath;
    try {
      if (copy) {
        await fs.promises.copyFile(
          temporaryPath,
          destination,
          fs.constants.COPYFILE_EXCL
        );
      } else {
        await fs.promises.link(temporaryPath, destination);
      }
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code;
      if (code === 'EEXIST') {
        suffix += 1;
        continue;
      }
      if (!copy) {
        copy = true;
        continue;
      }
      throw error;
    }

    try {
      await fs.promises.unlink(temporaryPath);
    } catch (error) {
      console.error('Failed to remove temporary screenshot:', error);
    }
    return destination;
  }
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
