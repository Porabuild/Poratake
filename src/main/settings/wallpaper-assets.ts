import crypto from 'crypto';
import fs from 'fs';
import path from 'path';
import { fileURLToPath, pathToFileURL } from 'url';
import { getConfigDir } from '@/main/utils/paths.ts';
import type {
  CustomBackground,
  SettingsConfig,
  WallpaperPreset,
} from '@/types/settings.ts';

const MIME_EXTENSIONS: Record<string, string> = {
  'image/png': '.png',
  'image/jpeg': '.jpg',
  'image/svg+xml': '.svg',
  'image/webp': '.webp',
  'image/gif': '.gif',
  'image/bmp': '.bmp',
};

const EXTENSION_MIME_TYPES = Object.fromEntries(
  Object.entries(MIME_EXTENSIONS).map(([mime, extension]) => [extension, mime])
);
const FILE_EXTENSIONS = new Set(Object.values(MIME_EXTENSIONS));
const PROJECT_ASSET_PREFIX = '.wallpaper-asset-';

function getWallpaperDirectory(): string {
  return path.join(getConfigDir(), 'wallpapers');
}

function isPathInside(filePath: string, directory: string): boolean {
  const relative = path.relative(directory, filePath);
  return !!relative && !relative.startsWith('..') && !path.isAbsolute(relative);
}

function getManagedWallpaperPath(imageUrl: string): string | null {
  const filePath = getWallpaperFilePath(imageUrl);
  if (!filePath) {
    return null;
  }
  return isPathInside(filePath, getWallpaperDirectory()) ? filePath : null;
}

function getWallpaperFilePath(imageUrl: string): string | null {
  if (typeof imageUrl !== 'string' || !imageUrl.startsWith('file:')) {
    return null;
  }

  try {
    return fileURLToPath(imageUrl);
  } catch {
    return null;
  }
}

function detectImageExtension(buffer: Buffer): string | null {
  if (
    buffer.length >= 8 &&
    buffer.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))
  ) {
    return '.png';
  }
  if (
    buffer.length >= 3 &&
    buffer[0] === 255 &&
    buffer[1] === 216 &&
    buffer[2] === 255
  ) {
    return '.jpg';
  }
  if (
    buffer.length >= 12 &&
    buffer.subarray(0, 4).toString('ascii') === 'RIFF' &&
    buffer.subarray(8, 12).toString('ascii') === 'WEBP'
  ) {
    return '.webp';
  }
  if (
    buffer.length >= 6 &&
    ['GIF87a', 'GIF89a'].includes(buffer.subarray(0, 6).toString('ascii'))
  ) {
    return '.gif';
  }
  if (buffer.length >= 2 && buffer.subarray(0, 2).toString('ascii') === 'BM') {
    return '.bmp';
  }

  const text = buffer.subarray(0, 1024).toString('utf8').trimStart();
  if (
    text.startsWith('<svg') ||
    (text.startsWith('<?xml') && text.includes('<svg'))
  ) {
    return '.svg';
  }
  return null;
}

function resolveImageExtension(filePath: string, buffer: Buffer): string {
  const sourceExtension = path.extname(filePath).toLowerCase();
  const normalizedExtension =
    sourceExtension === '.jpeg' || sourceExtension === '.jfif'
      ? '.jpg'
      : sourceExtension;
  if (FILE_EXTENSIONS.has(normalizedExtension)) {
    return normalizedExtension;
  }

  const detectedExtension = detectImageExtension(buffer);
  if (!detectedExtension) {
    throw new Error('Unsupported wallpaper image format');
  }
  return detectedExtension;
}

function persistBuffer(buffer: Buffer, extension: string): string {
  const directory = getWallpaperDirectory();
  if (!fs.existsSync(directory)) {
    fs.mkdirSync(directory, { recursive: true });
  }

  const digest = crypto.createHash('sha256').update(buffer).digest('hex');
  const assetPath = path.join(directory, `${digest}${extension}`);
  if (!fs.existsSync(assetPath)) {
    fs.writeFileSync(assetPath, buffer);
  }
  return pathToFileURL(assetPath).href;
}

export function persistWallpaperFile(filePath: string): string {
  const buffer = fs.readFileSync(filePath);
  return persistBuffer(buffer, resolveImageExtension(filePath, buffer));
}

export function persistWallpaperImage(imageUrl: string): string {
  if (imageUrl.startsWith('file:')) {
    if (getManagedWallpaperPath(imageUrl)) {
      return imageUrl;
    }
    return persistWallpaperFile(fileURLToPath(imageUrl));
  }
  if (!imageUrl.startsWith('data:')) {
    return imageUrl;
  }

  const match = /^data:([^;,]+);base64,([\s\S]+)$/.exec(imageUrl);
  const extension = match ? MIME_EXTENSIONS[match[1].toLowerCase()] : undefined;
  if (!match || !extension) {
    throw new Error('Unsupported wallpaper image data');
  }
  return persistBuffer(Buffer.from(match[2], 'base64'), extension);
}

export function embedWallpaperImage(imageUrl: string): string {
  const filePath = getWallpaperFilePath(imageUrl);
  if (!filePath) {
    return imageUrl;
  }

  const buffer = fs.readFileSync(filePath);
  const extension = resolveImageExtension(filePath, buffer);
  const mimeType = EXTENSION_MIME_TYPES[extension];
  return `data:${mimeType};base64,${buffer.toString('base64')}`;
}

export function localizeWallpaperImage(
  imageUrl: string,
  directory: string
): string {
  const sourcePath = getWallpaperFilePath(imageUrl);
  if (!sourcePath) {
    return imageUrl;
  }

  if (
    path.dirname(sourcePath) === directory &&
    path.basename(sourcePath).startsWith(PROJECT_ASSET_PREFIX) &&
    fs.existsSync(sourcePath)
  ) {
    return path.basename(sourcePath);
  }

  const buffer = fs.readFileSync(sourcePath);
  const extension = resolveImageExtension(sourcePath, buffer);
  const digest = crypto.createHash('sha256').update(buffer).digest('hex');
  const targetName = `${PROJECT_ASSET_PREFIX}${digest}${extension}`;
  const targetPath = path.join(directory, targetName);
  if (!fs.existsSync(targetPath)) {
    fs.writeFileSync(targetPath, buffer);
  }
  return targetName;
}

export function resolveLocalizedWallpaperImage(
  imageUrl: string,
  directory: string
): string {
  if (
    !imageUrl.startsWith(PROJECT_ASSET_PREFIX) ||
    path.basename(imageUrl) !== imageUrl
  ) {
    return imageUrl;
  }
  return pathToFileURL(path.join(directory, imageUrl)).href;
}

export function pruneWallpaperAssets(
  wallpaper: SettingsConfig['wallpaper']
): void {
  const directory = getWallpaperDirectory();
  if (!fs.existsSync(directory)) {
    return;
  }

  const retainedPaths = new Set<string>();
  for (const background of wallpaper.customBackgrounds) {
    if (
      background.type !== 'image' ||
      !background.data ||
      typeof background.data.imageUrl !== 'string'
    ) {
      continue;
    }
    const filePath = getManagedWallpaperPath(background.data.imageUrl);
    if (filePath) {
      retainedPaths.add(filePath);
    }
  }
  for (const preset of wallpaper.presets) {
    if (typeof preset.backgroundImage !== 'string') {
      continue;
    }
    const filePath = getManagedWallpaperPath(preset.backgroundImage);
    if (filePath) {
      retainedPaths.add(filePath);
    }
  }

  let entries: string[];
  try {
    entries = fs.readdirSync(directory);
  } catch (error) {
    console.warn('Failed to inspect wallpaper assets:', error);
    return;
  }

  for (const entry of entries) {
    const assetPath = path.join(directory, entry);
    if (!retainedPaths.has(assetPath)) {
      try {
        fs.unlinkSync(assetPath);
      } catch (error) {
        console.warn('Failed to remove unused wallpaper asset:', error);
      }
    }
  }
}

export function persistCustomBackground(
  background: CustomBackground
): CustomBackground {
  if (background.type !== 'image') {
    return background;
  }
  const imageUrl = persistWallpaperImage(background.data.imageUrl);
  if (imageUrl === background.data.imageUrl) {
    return background;
  }
  return {
    ...background,
    data: {
      imageUrl,
    },
  };
}

export function persistWallpaperPreset(
  preset: WallpaperPreset
): WallpaperPreset {
  if (!preset.backgroundImage) {
    return preset;
  }
  const backgroundImage = persistWallpaperImage(preset.backgroundImage);
  if (backgroundImage === preset.backgroundImage) {
    return preset;
  }
  return {
    ...preset,
    backgroundImage,
  };
}

export function migrateWallpaperAssets(
  wallpaper: SettingsConfig['wallpaper']
): { wallpaper: SettingsConfig['wallpaper']; migrated: boolean } {
  let migrated = false;
  const customBackgrounds = wallpaper.customBackgrounds.map(background => {
    try {
      const persisted = persistCustomBackground(background);
      migrated ||= persisted !== background;
      return persisted;
    } catch (error) {
      console.error('Failed to migrate wallpaper background:', error);
      return background;
    }
  });
  const presets = wallpaper.presets.map(preset => {
    try {
      const persisted = persistWallpaperPreset(preset);
      migrated ||= persisted !== preset;
      return persisted;
    } catch (error) {
      console.error('Failed to migrate wallpaper preset:', error);
      return preset;
    }
  });

  return {
    wallpaper: { ...wallpaper, customBackgrounds, presets },
    migrated,
  };
}
