import { daemon } from '@/main/daemon';
import type { Rect } from '@/types/geometry';

export type { Rect };

export interface WindowListItem {
  windowId: number;
  title: string;
  ownerName: string;
  ownerPid: number;
  bounds: Rect;
}

export async function listWindows(): Promise<WindowListItem[]> {
  const result = await daemon.call<{ windows?: WindowListItem[] }>(
    'window-selector',
    'list'
  );
  return result.windows ?? [];
}
