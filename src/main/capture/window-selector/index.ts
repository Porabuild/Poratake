import { daemon } from '@/main/daemon';

export interface WindowBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface WindowSelection {
  status: 'selected' | 'cancelled' | 'error';
  windowId?: number;
  windowTitle?: string;
  ownerName?: string;
  ownerPid?: number;
  bounds?: WindowBounds;
}

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

let isSelecting = false;

export async function selectWindow(): Promise<WindowSelection> {
  if (isSelecting) {
    throw new Error('Window selector is already active');
  }

  isSelecting = true;

  try {
    const result = await daemon.call<WindowSelection>(
      'window-selector',
      'select'
    );
    return result;
  } finally {
    isSelecting = false;
  }
}

export function killWindowSelector(): void {
  if (isSelecting) {
    daemon.call('window-selector', 'cancel').catch(() => {});
    isSelecting = false;
  }
}
