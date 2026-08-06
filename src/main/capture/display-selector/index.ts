import { daemon } from '@/main/daemon';

export interface DisplaySelection {
  status: 'selected' | 'cancelled';
  displayNumber?: number;
  screenId?: number;
}

let isSelecting = false;

export async function selectDisplay(): Promise<DisplaySelection> {
  if (isSelecting) {
    throw new Error('Display selector is already active');
  }

  isSelecting = true;

  try {
    const result = await daemon.call<DisplaySelection>(
      'display-selector',
      'select'
    );
    return result;
  } finally {
    isSelecting = false;
  }
}

export function killDisplaySelector(): void {
  if (isSelecting) {
    daemon.call('display-selector', 'cancel').catch(() => {});
    isSelecting = false;
  }
}
