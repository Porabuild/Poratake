import { app } from 'electron';
import { randomUUID } from 'crypto';
import fs from 'fs';
import path from 'path';
import { getConfig } from '@/main/settings';
import { generateFilename } from '@/main/utils/filename-generator';
import { ensureDirectoryExists, isValidDirectory } from '@/main/utils/paths';
import { showNotification } from '@/main/utils/notification';
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

export async function captureScreenshotToUniqueFile(
  capture: (
    temporaryPath: () => string,
    started: () => void,
    failed: (completed: boolean) => void
  ) => Promise<boolean>
): Promise<string | null> {
  const outputPath = generateScreenshotPath();
  let temporaryPath: string | null = null;
  const prepare = () => {
    if (temporaryPath) return temporaryPath;
    while (true) {
      const candidate = path.join(
        path.dirname(outputPath),
        `.poratake-${randomUUID()}.tmp.png`
      );
      try {
        const file = fs.openSync(candidate, 'wx');
        fs.closeSync(file);
        temporaryPath = candidate;
        return candidate;
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error;
      }
    }
  };

  let started = false;
  let completedFailure = false;
  let published = false;
  try {
    if (
      !(await capture(
        prepare,
        () => (started = true),
        completed => (completedFailure = completed)
      ))
    )
      return null;
    if (!temporaryPath)
      throw new Error('Screenshot destination was not prepared');
    const result = await publishScreenshot(temporaryPath, outputPath);
    published = true;
    return result;
  } finally {
    if (!published && temporaryPath) {
      try {
        if (
          !started ||
          (completedFailure &&
            (await fs.promises.stat(temporaryPath)).size === 0)
        ) {
          await fs.promises.unlink(temporaryPath);
        } else if (fs.existsSync(temporaryPath)) {
          console.error('Screenshot retained at:', temporaryPath);
          showNotification(
            'Screenshot Not Saved',
            `Capture data may be available at ${temporaryPath}`
          );
        }
      } catch (error) {
        console.error('Failed to handle temporary screenshot:', error);
      }
    }
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
