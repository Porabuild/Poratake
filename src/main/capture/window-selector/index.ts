import { daemon } from '@/main/daemon';
import type { WindowBounds } from '@/types/area';

export type { WindowBounds };

export interface WindowListItem {
  windowId: number;
  title: string;
  ownerName: string;
  ownerPid: number;
  bounds: WindowBounds;
}

export async function listWindows(): Promise<WindowListItem[]> {
  const result = await daemon.call<{ windows?: WindowListItem[] }>(
    'window-selector',
    'list'
  );
  return result.windows ?? [];
}
