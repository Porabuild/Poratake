import fs from 'fs/promises';
import type { CursorData } from '@/types/cursor';
import { getCursorPath } from './recording-project.ts';

export function getCursorDataPath(videoPath: string): string {
  return getCursorPath(videoPath);
}

export async function loadCursorData(
  videoPath: string
): Promise<CursorData | null> {
  const cursorPath = getCursorDataPath(videoPath);

  try {
    const content = await fs.readFile(cursorPath, 'utf-8');
    return JSON.parse(content) as CursorData;
  } catch {
    return null;
  }
}

export async function saveCursorData(
  videoPath: string,
  cursorData: CursorData
): Promise<void> {
  const cursorPath = getCursorDataPath(videoPath);
  await fs.writeFile(cursorPath, JSON.stringify(cursorData, null, 2), 'utf-8');
}

export async function deleteCursorData(videoPath: string): Promise<void> {
  const cursorPath = getCursorDataPath(videoPath);

  try {
    await fs.unlink(cursorPath);
    console.log(`Cursor data deleted: ${cursorPath}`);
  } catch {
    console.warn(`Failed to delete cursor data: ${cursorPath}`);
  }
}
