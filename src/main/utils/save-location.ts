import path from 'path';
import { getConfig, updateConfig } from '@/main/settings';
import { isExistingDirectory } from '@/main/utils/paths';
import type { SaveLocationKind } from '@/types/settings';
import { DEFAULT_SAVE_LOCATIONS_CONFIG } from '@/types/settings';

export function getLastSaveDirectory(kind: SaveLocationKind): string | null {
  const directory = getConfig().saveLocations?.[kind];

  if (!directory || !isExistingDirectory(directory)) {
    return null;
  }

  return directory;
}

export function resolveSaveDialogPath(
  kind: SaveLocationKind,
  fileName: string,
  fallbackDirectory?: string
): string {
  const directory = getLastSaveDirectory(kind) ?? fallbackDirectory;

  return directory ? path.join(directory, fileName) : fileName;
}

export function rememberSaveDirectory(
  kind: SaveLocationKind,
  filePath: string
): void {
  const directory = path.dirname(filePath);
  const saveLocations = getConfig().saveLocations;

  if (saveLocations?.[kind] === directory) {
    return;
  }

  updateConfig({
    saveLocations: {
      ...DEFAULT_SAVE_LOCATIONS_CONFIG,
      ...saveLocations,
      [kind]: directory,
    },
  });
}
