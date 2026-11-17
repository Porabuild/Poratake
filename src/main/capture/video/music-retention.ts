import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { getMusicFolderPath } from './recording-project';
import type { MusicTrack } from '@/types/music';

const RETENTION_FOLDER = '.poratake-music-retention';

export function isMusicNameReserved(folder: string, fileName: string): boolean {
  const records = path.join(path.dirname(folder), RETENTION_FOLDER);
  try {
    if (!fs.lstatSync(records).isDirectory())
      throw new Error('Invalid retention folder');
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return false;
    throw error;
  }
  const hash = createHash('sha256').update(fileName, 'utf8').digest('hex');
  try {
    fs.lstatSync(path.join(records, `${hash}.json`));
    return true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return false;
    throw error;
  }
}

export function recordMusicCandidate(
  projectPath: string,
  fileName: string
): void {
  const folder = getMusicFolderPath(projectPath);
  if (
    !folder ||
    !fileName ||
    fileName === '.' ||
    fileName === '..' ||
    path.basename(fileName) !== fileName
  ) {
    throw new Error('Invalid music path');
  }
  if (!fs.lstatSync(folder).isDirectory())
    throw new Error('Invalid music folder');
  if (!fs.lstatSync(path.join(folder, fileName)).isFile()) {
    throw new Error('Not a music file');
  }
  const records = path.join(path.dirname(folder), RETENTION_FOLDER);
  try {
    fs.mkdirSync(records);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error;
  }
  if (!fs.lstatSync(records).isDirectory())
    throw new Error('Invalid retention folder');
  const hash = createHash('sha256').update(fileName, 'utf8').digest('hex');
  const marker = path.join(records, `${hash}.json`);
  let descriptor: number;
  try {
    descriptor = fs.openSync(marker, 'wx', 0o600);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'EEXIST') return;
    throw error;
  }
  try {
    try {
      fs.writeFileSync(descriptor, JSON.stringify({ fileName }));
      fs.fsyncSync(descriptor);
    } finally {
      fs.closeSync(descriptor);
    }
  } catch (error) {
    try {
      fs.unlinkSync(marker);
    } catch {}
    throw error;
  }
}

export function recordRemovedMusic(
  projectPath: string,
  before: Pick<MusicTrack, 'source' | 'fileName'>[],
  after: Pick<MusicTrack, 'source' | 'fileName'>[]
): void {
  const retained = new Set(
    after.filter(track => track.source === 'music').map(track => track.fileName)
  );
  const removed = new Set(
    before
      .filter(track => track.source === 'music')
      .map(track => track.fileName)
  );
  for (const fileName of removed) {
    if (retained.has(fileName)) continue;
    try {
      recordMusicCandidate(projectPath, fileName);
    } catch (error) {
      console.error('Could not record retained music:', error);
    }
  }
}
