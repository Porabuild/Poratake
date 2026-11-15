import { describe, it, expect, vi, beforeEach } from 'vitest';
import fs from 'fs';
import os from 'os';
import path from 'path';

const mockGetConfig = vi.fn();
const mockGenerateFilename = vi.fn();
const mockEnsureDirectoryExists = vi.fn();
const mockIsValidDirectory = vi.fn();
const mockGetPath = vi.fn();
const mockShowNotification = vi.fn();

vi.mock('electron', () => ({
  app: { getPath: (...a: unknown[]) => mockGetPath(...a) },
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
}));

vi.mock('@/main/utils/filename-generator', () => ({
  generateFilename: (...a: unknown[]) => mockGenerateFilename(...a),
}));

vi.mock('@/main/utils/paths', () => ({
  ensureDirectoryExists: (...a: unknown[]) => mockEnsureDirectoryExists(...a),
  isValidDirectory: (...a: unknown[]) => mockIsValidDirectory(...a),
}));

vi.mock('@/main/utils/notification', () => ({
  showNotification: (...args: unknown[]) => mockShowNotification(...args),
}));

describe('screenshot utils', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockEnsureDirectoryExists.mockImplementation((p: string) => p);
    mockGenerateFilename.mockReturnValue('Screenshot.png');
    mockGetPath.mockReturnValue('/Users/me/Pictures');
  });

  describe('getScreenshotsDir', () => {
    it('uses custom path when valid', async () => {
      mockGetConfig.mockReturnValue({
        storage: { screenshotsPath: '/custom/dir' },
      });
      mockIsValidDirectory.mockReturnValue(true);
      const { getScreenshotsDir } =
        await import('@/main/capture/screenshot/utils');
      expect(getScreenshotsDir()).toBe('/custom/dir');
    });

    it('falls back to Pictures/Poratake when custom path invalid', async () => {
      mockGetConfig.mockReturnValue({
        storage: { screenshotsPath: '/bad/dir' },
      });
      mockIsValidDirectory.mockReturnValue(false);
      const { getScreenshotsDir } =
        await import('@/main/capture/screenshot/utils');
      expect(getScreenshotsDir()).toBe(
        path.join('/Users/me/Pictures', 'Poratake')
      );
    });

    it('uses default Pictures/Poratake when no custom path', async () => {
      mockGetConfig.mockReturnValue({ storage: {} });
      const { getScreenshotsDir } =
        await import('@/main/capture/screenshot/utils');
      expect(getScreenshotsDir()).toBe(
        path.join('/Users/me/Pictures', 'Poratake')
      );
    });
  });

  describe('generateScreenshotPath', () => {
    it('combines directory and generated filename', async () => {
      mockGetConfig.mockReturnValue({
        storage: { namingPattern: 'CustomPattern' },
      });
      mockIsValidDirectory.mockReturnValue(false);
      mockGenerateFilename.mockReturnValue('My Screenshot.png');
      const { generateScreenshotPath } =
        await import('@/main/capture/screenshot/utils');
      const result = generateScreenshotPath();
      expect(result).toBe(
        path.join('/Users/me/Pictures', 'Poratake', 'My Screenshot.png')
      );
      expect(mockGenerateFilename).toHaveBeenCalledWith({
        pattern: 'CustomPattern',
        type: 'Screenshot',
        extension: 'png',
      });
    });

    it('uses default pattern when not configured', async () => {
      mockGetConfig.mockReturnValue({ storage: {} });
      const { generateScreenshotPath } =
        await import('@/main/capture/screenshot/utils');
      generateScreenshotPath();
      const [args] = mockGenerateFilename.mock.calls[0];
      expect(args.pattern).toBeDefined();
    });
  });

  describe('generateScreenshotExportName', () => {
    it('uses provided extension', async () => {
      mockGetConfig.mockReturnValue({ storage: {} });
      mockGenerateFilename.mockReturnValue('Screenshot.jpg');
      const { generateScreenshotExportName } =
        await import('@/main/capture/screenshot/utils');
      const name = generateScreenshotExportName('jpg');
      expect(name).toBe('Screenshot.jpg');
      const [args] = mockGenerateFilename.mock.calls[0];
      expect(args.extension).toBe('jpg');
    });

    it('defaults extension to png', async () => {
      mockGetConfig.mockReturnValue({ storage: {} });
      const { generateScreenshotExportName } =
        await import('@/main/capture/screenshot/utils');
      generateScreenshotExportName();
      const [args] = mockGenerateFilename.mock.calls[0];
      expect(args.extension).toBe('png');
    });
  });

  describe('publishScreenshot', () => {
    it('keeps existing captures and publishes each new image under a unique name', async () => {
      const { publishScreenshot } =
        await import('@/main/capture/screenshot/utils');
      const directory = fs.mkdtempSync(
        path.join(os.tmpdir(), 'poratake-publish-')
      );
      const destination = path.join(directory, 'Capture.png');
      const first = path.join(directory, 'first.tmp.png');
      const second = path.join(directory, 'second.tmp.png');

      try {
        fs.writeFileSync(destination, 'original');
        fs.writeFileSync(first, 'first');
        fs.writeFileSync(second, 'second');

        const [firstPublished, secondPublished] = await Promise.all([
          publishScreenshot(first, destination),
          publishScreenshot(second, destination),
        ]);

        expect(new Set([firstPublished, secondPublished])).toEqual(
          new Set([
            path.join(directory, 'Capture (1).png'),
            path.join(directory, 'Capture (2).png'),
          ])
        );
        expect(fs.readFileSync(destination, 'utf8')).toBe('original');
        expect(fs.readFileSync(firstPublished, 'utf8')).toBe('first');
        expect(fs.readFileSync(secondPublished, 'utf8')).toBe('second');
        expect(fs.existsSync(first)).toBe(false);
        expect(fs.existsSync(second)).toBe(false);
      } finally {
        for (const name of fs.readdirSync(directory)) {
          fs.unlinkSync(path.join(directory, name));
        }
        fs.rmdirSync(directory);
      }
    });

    it('preserves existing captures when hard links are unavailable', async () => {
      const { publishScreenshot } =
        await import('@/main/capture/screenshot/utils');
      const directory = fs.mkdtempSync(
        path.join(os.tmpdir(), 'poratake-publish-')
      );
      const destination = path.join(directory, 'Capture.png');
      const temporary = path.join(directory, 'capture.tmp.png');
      const link = vi
        .spyOn(fs.promises, 'link')
        .mockImplementation(async () => {
          throw Object.assign(new Error('hard links unavailable'), {
            code: 'EPERM',
          });
        });

      try {
        fs.writeFileSync(destination, 'original');
        fs.writeFileSync(temporary, 'capture');
        const published = await publishScreenshot(temporary, destination);

        expect(link).toHaveBeenCalled();
        expect(published).toBe(path.join(directory, 'Capture (1).png'));
        expect(fs.readFileSync(destination, 'utf8')).toBe('original');
        expect(fs.readFileSync(published, 'utf8')).toBe('capture');
        expect(fs.existsSync(temporary)).toBe(false);
      } finally {
        link.mockRestore();
        for (const name of fs.readdirSync(directory)) {
          fs.unlinkSync(path.join(directory, name));
        }
        fs.rmdirSync(directory);
      }
    });

    it('keeps the event loop available while fallback copying is pending', async () => {
      const { publishScreenshot } =
        await import('@/main/capture/screenshot/utils');
      const directory = fs.mkdtempSync(
        path.join(os.tmpdir(), 'poratake-publish-')
      );
      const destination = path.join(directory, 'Capture.png');
      const temporary = path.join(directory, 'capture.tmp.png');
      let releaseCopy: () => void = () => {};
      let markStarted: () => void = () => {};
      const started = new Promise<void>(resolve => {
        markStarted = resolve;
      });
      const gate = new Promise<void>(resolve => {
        releaseCopy = resolve;
      });
      const copyFile = fs.promises.copyFile.bind(fs.promises);
      const link = vi
        .spyOn(fs.promises, 'link')
        .mockRejectedValue(
          Object.assign(new Error('hard links unavailable'), { code: 'EPERM' })
        );
      const copy = vi
        .spyOn(fs.promises, 'copyFile')
        .mockImplementation(async (source, target, flags) => {
          markStarted();
          await gate;
          await copyFile(source, target, flags);
        });
      fs.writeFileSync(temporary, 'capture');
      const publication = publishScreenshot(temporary, destination);

      try {
        await started;
        let settled = false;
        void publication.then(() => {
          settled = true;
        });
        await new Promise(resolve => setImmediate(resolve));
        expect(settled).toBe(false);
        releaseCopy();
        expect(await publication).toBe(destination);
        expect(fs.readFileSync(destination, 'utf8')).toBe('capture');
      } finally {
        releaseCopy();
        await publication.catch(() => {});
        link.mockRestore();
        copy.mockRestore();
        for (const name of fs.readdirSync(directory)) {
          fs.unlinkSync(path.join(directory, name));
        }
        fs.rmdirSync(directory);
      }
    });

    it('retains the temporary capture when publication fails', async () => {
      const { publishScreenshot } =
        await import('@/main/capture/screenshot/utils');
      const directory = fs.mkdtempSync(
        path.join(os.tmpdir(), 'poratake-publish-')
      );
      const temporary = path.join(directory, 'capture.tmp.png');

      try {
        fs.writeFileSync(temporary, 'capture');
        await expect(
          publishScreenshot(
            temporary,
            path.join(directory, 'missing', 'Capture.png')
          )
        ).rejects.toThrow(/ENOENT/);
        expect(fs.readFileSync(temporary, 'utf8')).toBe('capture');
      } finally {
        fs.unlinkSync(temporary);
        fs.rmdirSync(directory);
      }
    });
  });

  describe('captureScreenshotToUniqueFile', () => {
    it('preserves existing files across concurrent captures with the same name', async () => {
      const { captureScreenshotToUniqueFile } =
        await import('@/main/capture/screenshot/utils');
      const directory = fs.mkdtempSync(
        path.join(os.tmpdir(), 'poratake-capture-')
      );
      const existing = path.join(directory, 'Capture.png');
      mockGetConfig.mockReturnValue({
        storage: { screenshotsPath: directory },
      });
      mockIsValidDirectory.mockReturnValue(true);
      mockGenerateFilename.mockReturnValue('Capture.png');

      try {
        fs.writeFileSync(existing, 'original');
        const paths = await Promise.all(
          ['first', 'second'].map(bytes =>
            captureScreenshotToUniqueFile(async (temporary, started) => {
              const file = temporary();
              started();
              await fs.promises.writeFile(file, bytes);
              return true;
            })
          )
        );

        expect(new Set(paths)).toEqual(
          new Set([
            path.join(directory, 'Capture (1).png'),
            path.join(directory, 'Capture (2).png'),
          ])
        );
        expect(fs.readFileSync(existing, 'utf8')).toBe('original');
        expect(
          new Set(paths.map(file => fs.readFileSync(file!, 'utf8')))
        ).toEqual(new Set(['first', 'second']));
        expect(fs.readdirSync(directory)).toHaveLength(3);
      } finally {
        for (const name of fs.readdirSync(directory))
          fs.unlinkSync(path.join(directory, name));
        fs.rmdirSync(directory);
      }
    });

    it('does not reserve a temporary file when selection is cancelled', async () => {
      const { captureScreenshotToUniqueFile } =
        await import('@/main/capture/screenshot/utils');
      const directory = fs.mkdtempSync(
        path.join(os.tmpdir(), 'poratake-capture-')
      );
      mockGetConfig.mockReturnValue({
        storage: { screenshotsPath: directory },
      });
      mockIsValidDirectory.mockReturnValue(true);
      const open = vi.spyOn(fs, 'openSync');

      try {
        await expect(
          captureScreenshotToUniqueFile(async () => false)
        ).resolves.toBeNull();
        expect(open).not.toHaveBeenCalled();
        expect(fs.readdirSync(directory)).toEqual([]);
        expect(mockShowNotification).not.toHaveBeenCalled();
      } finally {
        open.mockRestore();
        fs.rmdirSync(directory);
      }
    });

    it('removes a prepared file if capture is cancelled before it starts', async () => {
      const { captureScreenshotToUniqueFile } =
        await import('@/main/capture/screenshot/utils');
      const directory = fs.mkdtempSync(
        path.join(os.tmpdir(), 'poratake-capture-')
      );
      mockGetConfig.mockReturnValue({
        storage: { screenshotsPath: directory },
      });
      mockIsValidDirectory.mockReturnValue(true);

      try {
        await expect(
          captureScreenshotToUniqueFile(async temporary => {
            temporary();
            return false;
          })
        ).resolves.toBeNull();
        expect(fs.readdirSync(directory)).toEqual([]);
        expect(mockShowNotification).not.toHaveBeenCalled();
      } finally {
        fs.rmdirSync(directory);
      }
    });

    it('retains and reports capture data when a started capture fails', async () => {
      const { captureScreenshotToUniqueFile } =
        await import('@/main/capture/screenshot/utils');
      const directory = fs.mkdtempSync(
        path.join(os.tmpdir(), 'poratake-capture-')
      );
      mockGetConfig.mockReturnValue({
        storage: { screenshotsPath: directory },
      });
      mockIsValidDirectory.mockReturnValue(true);
      let temporaryPath = '';

      try {
        await expect(
          captureScreenshotToUniqueFile(async (temporary, started, failed) => {
            temporaryPath = temporary();
            started();
            await fs.promises.writeFile(temporaryPath, 'capture data');
            failed(true);
            return false;
          })
        ).resolves.toBeNull();
        expect(fs.readFileSync(temporaryPath, 'utf8')).toBe('capture data');
        expect(mockShowNotification).toHaveBeenCalledWith(
          'Screenshot Not Saved',
          `Capture data may be available at ${temporaryPath}`
        );
      } finally {
        if (temporaryPath && fs.existsSync(temporaryPath))
          fs.unlinkSync(temporaryPath);
        fs.rmdirSync(directory);
      }
    });

    it('removes an empty staging file after a confirmed capture failure', async () => {
      const { captureScreenshotToUniqueFile } =
        await import('@/main/capture/screenshot/utils');
      const directory = fs.mkdtempSync(
        path.join(os.tmpdir(), 'poratake-capture-')
      );
      mockGetConfig.mockReturnValue({
        storage: { screenshotsPath: directory },
      });
      mockIsValidDirectory.mockReturnValue(true);

      try {
        await expect(
          captureScreenshotToUniqueFile(async (temporary, started, failed) => {
            temporary();
            started();
            failed(true);
            return false;
          })
        ).resolves.toBeNull();
        expect(fs.readdirSync(directory)).toEqual([]);
        expect(mockShowNotification).not.toHaveBeenCalled();
      } finally {
        for (const name of fs.readdirSync(directory))
          fs.unlinkSync(path.join(directory, name));
        fs.rmdirSync(directory);
      }
    });

    it('retains an empty staging file when a timed-out worker may still write', async () => {
      const { captureScreenshotToUniqueFile } =
        await import('@/main/capture/screenshot/utils');
      const directory = fs.mkdtempSync(
        path.join(os.tmpdir(), 'poratake-capture-')
      );
      mockGetConfig.mockReturnValue({
        storage: { screenshotsPath: directory },
      });
      mockIsValidDirectory.mockReturnValue(true);
      let temporaryPath = '';

      try {
        await expect(
          captureScreenshotToUniqueFile(async (temporary, started, failed) => {
            temporaryPath = temporary();
            started();
            failed(false);
            return false;
          })
        ).resolves.toBeNull();
        expect(fs.existsSync(temporaryPath)).toBe(true);
        expect(mockShowNotification).toHaveBeenCalledWith(
          'Screenshot Not Saved',
          `Capture data may be available at ${temporaryPath}`
        );
        await fs.promises.writeFile(temporaryPath, 'late image');
        expect(fs.readFileSync(temporaryPath, 'utf8')).toBe('late image');
      } finally {
        if (temporaryPath && fs.existsSync(temporaryPath))
          fs.unlinkSync(temporaryPath);
        fs.rmdirSync(directory);
      }
    });

    it('retains capture data and an existing image when publication fails', async () => {
      const { captureScreenshotToUniqueFile } =
        await import('@/main/capture/screenshot/utils');
      const directory = fs.mkdtempSync(
        path.join(os.tmpdir(), 'poratake-capture-')
      );
      const existing = path.join(directory, 'Capture.png');
      mockGetConfig.mockReturnValue({
        storage: { screenshotsPath: directory },
      });
      mockIsValidDirectory.mockReturnValue(true);
      mockGenerateFilename.mockReturnValue('Capture.png');
      let temporaryPath = '';
      const failure = Object.assign(new Error('write denied'), {
        code: 'EACCES',
      });
      const link = vi.spyOn(fs.promises, 'link').mockRejectedValue(failure);
      const copy = vi.spyOn(fs.promises, 'copyFile').mockRejectedValue(failure);

      try {
        fs.writeFileSync(existing, 'original');
        await expect(
          captureScreenshotToUniqueFile(async (temporary, started) => {
            temporaryPath = temporary();
            started();
            await fs.promises.writeFile(temporaryPath, 'capture data');
            return true;
          })
        ).rejects.toThrow('write denied');
        expect(fs.readFileSync(existing, 'utf8')).toBe('original');
        expect(fs.readFileSync(temporaryPath, 'utf8')).toBe('capture data');
        expect(mockShowNotification).toHaveBeenCalledWith(
          'Screenshot Not Saved',
          `Capture data may be available at ${temporaryPath}`
        );
      } finally {
        link.mockRestore();
        copy.mockRestore();
        for (const name of fs.readdirSync(directory))
          fs.unlinkSync(path.join(directory, name));
        fs.rmdirSync(directory);
      }
    });
  });
});
