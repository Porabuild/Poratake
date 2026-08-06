import fs from 'fs/promises';
import type { KeyboardData } from '@/types/keyboard';
import { getKeysPath } from './recording-project';

export function getKeyboardDataPath(videoPath: string): string {
  return getKeysPath(videoPath);
}

export async function loadKeyboardData(
  videoPath: string
): Promise<KeyboardData | null> {
  const keysPath = getKeyboardDataPath(videoPath);

  try {
    const content = await fs.readFile(keysPath, 'utf-8');
    return JSON.parse(content) as KeyboardData;
  } catch {
    return null;
  }
}

export async function deleteKeyboardData(videoPath: string): Promise<void> {
  const keysPath = getKeyboardDataPath(videoPath);

  try {
    await fs.unlink(keysPath);
    console.log(`Keyboard data deleted: ${keysPath}`);
  } catch {
    console.warn(`Failed to delete keyboard data: ${keysPath}`);
  }
}
