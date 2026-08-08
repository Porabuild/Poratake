import { nativeImage } from 'electron';
import path from 'path';
import fs from 'fs';
import crypto from 'crypto';
import { getConfigDir } from './paths';
import { generateVideoThumbnail } from './ffmpeg';

const THUMBNAILS_DIR = path.join(getConfigDir(), 'thumbnails');
const THUMBNAIL_WIDTH = 300;
const THUMBNAIL_QUALITY = 80;

function ensureThumbnailsDir(): void {
  if (!fs.existsSync(THUMBNAILS_DIR)) {
    fs.mkdirSync(THUMBNAILS_DIR, { recursive: true });
  }
}

function getThumbnailPath(originalPath: string, ext: string = 'jpg'): string {
  const hash = crypto.createHash('md5').update(originalPath).digest('hex');
  return path.join(THUMBNAILS_DIR, `${hash}.${ext}`);
}

function generateImageThumbnail(
  inputPath: string,
  outputPath: string
): boolean {
  try {
    const image = nativeImage.createFromPath(inputPath);

    if (image.isEmpty()) {
      return false;
    }

    const { width } = image.getSize();
    const thumbnail =
      width > THUMBNAIL_WIDTH
        ? image.resize({ width: THUMBNAIL_WIDTH, quality: 'good' })
        : image;

    fs.writeFileSync(outputPath, thumbnail.toJPEG(THUMBNAIL_QUALITY));
    return true;
  } catch (error) {
    console.error('Failed to generate image thumbnail:', error);
    return false;
  }
}

export interface ThumbnailResult {
  base64: string | null;
  cached: boolean;
}

export async function getThumbnail(
  originalPath: string,
  type: 'screenshot' | 'video'
): Promise<ThumbnailResult> {
  if (!fs.existsSync(originalPath)) {
    return { base64: null, cached: false };
  }

  ensureThumbnailsDir();
  const thumbnailPath = getThumbnailPath(originalPath, 'jpg');

  if (fs.existsSync(thumbnailPath)) {
    try {
      const buffer = fs.readFileSync(thumbnailPath);
      return { base64: buffer.toString('base64'), cached: true };
    } catch {
      console.warn(`Failed to read cached thumbnail: ${thumbnailPath}`);
    }
  }

  let success = false;

  if (type === 'video') {
    const result = await generateVideoThumbnail({
      inputPath: originalPath,
      outputPath: thumbnailPath,
      time: 0.5,
    });
    success = result.success;
    if (!success) {
      console.error(`Failed to generate video thumbnail: ${result.message}`);
    }
  } else {
    success = generateImageThumbnail(originalPath, thumbnailPath);
  }

  if (success && fs.existsSync(thumbnailPath)) {
    const buffer = fs.readFileSync(thumbnailPath);
    return { base64: buffer.toString('base64'), cached: false };
  }

  return { base64: null, cached: false };
}

export function rekeyThumbnail(oldPath: string, newPath: string): void {
  const oldThumbnailPath = getThumbnailPath(oldPath, 'jpg');
  const newThumbnailPath = getThumbnailPath(newPath, 'jpg');
  try {
    if (fs.existsSync(oldThumbnailPath)) {
      fs.renameSync(oldThumbnailPath, newThumbnailPath);
    }
  } catch {
    console.warn('Failed to rekey thumbnail');
  }
}

export function deleteThumbnail(originalPath: string): void {
  const thumbnailPath = getThumbnailPath(originalPath, 'jpg');
  try {
    if (fs.existsSync(thumbnailPath)) {
      fs.unlinkSync(thumbnailPath);
    }
  } catch {
    console.warn(`Failed to delete thumbnail: ${thumbnailPath}`);
  }
}

export function clearAllThumbnails(): void {
  try {
    if (fs.existsSync(THUMBNAILS_DIR)) {
      fs.rmSync(THUMBNAILS_DIR, { recursive: true, force: true });
    }
  } catch {
    console.warn('Failed to clear thumbnails directory');
  }
}
