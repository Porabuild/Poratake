import { screen } from 'electron';
import { daemon } from '@/main/daemon';
import { isMac } from '@/main/utils/platform';

const WINDOW_WIDTH = 140;
const WINDOW_HEIGHT = 52;
const TIMER_TOP_MARGIN = 20;

export interface TimerAreaRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface TimerPosition {
  x: number;
  y: number;
}

export function calculateTimerPosition(area: TimerAreaRect): TimerPosition {
  const x = Math.round(area.x + area.width / 2 - WINDOW_WIDTH / 2);
  let y = Math.round(area.y - WINDOW_HEIGHT - TIMER_TOP_MARGIN);

  if (y < TIMER_TOP_MARGIN) {
    y = TIMER_TOP_MARGIN;
  }

  return { x, y };
}

export async function showTimerControl(
  position: TimerPosition,
  duration: number
): Promise<boolean> {
  try {
    const screenPosition = isMac
      ? position
      : screen.dipToScreenPoint({ x: position.x, y: position.y });
    await daemon.call('timer-control', 'show', {
      x: screenPosition.x,
      y: screenPosition.y,
      duration,
    });
    return true;
  } catch (error) {
    console.error('Failed to show timer control:', error);
    return false;
  }
}

export async function hideTimerControl(): Promise<void> {
  try {
    await daemon.call('timer-control', 'hide');
  } catch (error) {
    console.error('Failed to hide timer control:', error);
  }
}

export function createTimerCompletionWaiter(): {
  result: Promise<boolean>;
  cancel: () => void;
} {
  let finish: (completed: boolean) => void = () => {};
  const result = new Promise<boolean>(resolve => {
    let isPending = true;
    const handler = (event: string) => {
      if (event === 'timer-control:completed') {
        finish(true);
      } else if (event === 'timer-control:cancel') {
        finish(false);
      }
    };
    finish = completed => {
      if (!isPending) return;

      isPending = false;
      daemon.offEvent(handler);
      resolve(completed);
    };
    daemon.onEvent(handler);
  });

  return {
    result,
    cancel: () => finish(false),
  };
}
