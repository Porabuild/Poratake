import { setOverlayToolbar } from '@/main/capture/area-overlay';

let currentAreaSelection: {
  x: number;
  y: number;
  width: number;
  height: number;
} | null = null;

export async function showAllInOneControl(area?: {
  x: number;
  y: number;
  width: number;
  height: number;
}): Promise<void> {
  currentAreaSelection = area || null;
}

export async function updateAllInOnePosition(area: {
  x: number;
  y: number;
  width: number;
  height: number;
}): Promise<void> {
  currentAreaSelection = area;
}

export async function hideAllInOneControl(): Promise<void> {
  setOverlayToolbar(null);
  currentAreaSelection = null;
}

export function getCurrentAreaSelection(): {
  x: number;
  y: number;
  width: number;
  height: number;
} | null {
  return currentAreaSelection;
}
