import { daemon } from '@/main/daemon';

let isVisible = false;

export async function showRecordingOverlay(
  x: number,
  y: number,
  width: number,
  height: number
): Promise<void> {
  try {
    await daemon.call('recording-overlay', 'show', { x, y, width, height });
    isVisible = true;
  } catch (error) {
    console.error('Failed to show recording overlay:', error);
  }
}

export async function hideRecordingOverlay(): Promise<void> {
  if (!isVisible) {
    return;
  }

  try {
    await daemon.call('recording-overlay', 'hide');
    isVisible = false;
  } catch (error) {
    console.error('Failed to hide recording overlay:', error);
  }
}

export async function prewarmOverlay(): Promise<void> {
  // No longer needed - daemon is always running
}
