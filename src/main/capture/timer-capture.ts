import { getConfig, updateConfig } from '@/main/settings';
import { daemon } from '@/main/daemon';
import {
  startAreaSelection,
  cancelAreaSelection,
  hideAreaSelector,
} from '@/main/capture/area-selector';
import { captureArea } from '@/main/capture/screenshot';
import {
  hideDesktopIcons,
  showDesktopIcons,
  isSupported as isDesktopIconsSupported,
  checkAccessibilityPermission,
} from '@/main/capture/desktop-icons';

const TIMER_DURATION = 5;
const WINDOW_WIDTH = 140;
const WINDOW_HEIGHT = 52;
const TIMER_TOP_MARGIN = 20;

let eventCleanup: (() => void) | null = null;
let isTimerActive = false;

interface TimerControlPosition {
  x: number;
  y: number;
}

function calculateTimerPosition(area: {
  x: number;
  y: number;
  width: number;
  height: number;
}): TimerControlPosition {
  const x = Math.round(area.x + area.width / 2 - WINDOW_WIDTH / 2);
  let y = Math.round(area.y - WINDOW_HEIGHT - TIMER_TOP_MARGIN);

  if (y < TIMER_TOP_MARGIN) {
    y = TIMER_TOP_MARGIN;
  }

  return { x, y };
}

function setupEventListener(onCancel: () => void): void {
  if (eventCleanup) {
    eventCleanup();
  }

  const handler = (event: string) => {
    if (event === 'timer-control:cancel') {
      onCancel();
    }
  };

  daemon.onEvent(handler);
  eventCleanup = () => {
    daemon.offEvent(handler);
    eventCleanup = null;
  };
}

function cleanupEventListener(): void {
  eventCleanup?.();
  eventCleanup = null;
}

async function showTimerControl(position: TimerControlPosition): Promise<void> {
  try {
    await daemon.call('timer-control', 'show', {
      x: position.x,
      y: position.y,
      duration: TIMER_DURATION,
    });
  } catch (error) {
    console.error('Failed to show timer control:', error);
  }
}

async function hideTimerControl(): Promise<void> {
  try {
    await daemon.call('timer-control', 'hide');
  } catch (error) {
    console.error('Failed to hide timer control:', error);
  }
}

export default async function timerCapture(): Promise<void> {
  if (isTimerActive) {
    return;
  }

  const config = getConfig();
  let shouldHideIcons =
    config.screenshot.hideDesktopIcons && isDesktopIconsSupported();

  if (shouldHideIcons && !checkAccessibilityPermission(false)) {
    shouldHideIcons = false;
    updateConfig({
      screenshot: { ...config.screenshot, hideDesktopIcons: false },
    });
  }

  if (shouldHideIcons) {
    await hideDesktopIcons('capture');
  }

  return new Promise<void>(resolve => {
    let captured = false;
    let timerCancelled = false;

    const cleanup = async () => {
      isTimerActive = false;
      cleanupEventListener();
      await hideTimerControl();
      if (shouldHideIcons) {
        await showDesktopIcons('capture');
      }
    };

    const handleSelected = async (selection: {
      x?: number;
      y?: number;
      width?: number;
      height?: number;
    }) => {
      if (captured || timerCancelled) return;
      if (
        selection.x === undefined ||
        selection.y === undefined ||
        selection.width === undefined ||
        selection.height === undefined
      ) {
        return;
      }

      captured = true;
      isTimerActive = true;

      const position = calculateTimerPosition({
        x: selection.x,
        y: selection.y,
        width: selection.width,
        height: selection.height,
      });

      setupEventListener(async () => {
        timerCancelled = true;
        await cleanup();
        await hideAreaSelector();
        await cancelAreaSelection();
        resolve();
      });

      await showTimerControl(position);

      const timerPromise = new Promise<boolean>(timerResolve => {
        const handler = (event: string) => {
          if (event === 'timer-control:completed') {
            daemon.offEvent(handler);
            timerResolve(true);
          } else if (event === 'timer-control:cancel') {
            daemon.offEvent(handler);
            timerResolve(false);
          }
        };
        daemon.onEvent(handler);
      });

      const shouldCapture = await timerPromise;

      if (!shouldCapture || timerCancelled) {
        await cleanup();
        await hideAreaSelector();
        await cancelAreaSelection();
        resolve();
        return;
      }

      await hideTimerControl();
      cleanupEventListener();
      await hideAreaSelector();
      await new Promise(r => setTimeout(r, 50));
      await cancelAreaSelection();
      await new Promise(r => setTimeout(r, 50));

      await captureArea(
        {
          status: 'confirmed',
          x: selection.x,
          y: selection.y,
          width: selection.width,
          height: selection.height,
        },
        {
          onCaptured: async () => {
            if (shouldHideIcons) {
              await showDesktopIcons('capture');
            }
          },
        }
      );

      isTimerActive = false;
      resolve();
    };

    startAreaSelection({
      onSelected: handleSelected,
      onCancelled: async () => {
        timerCancelled = true;
        await cleanup();
        resolve();
      },
      showPrompt: false,
      style: 'simple',
    });
  });
}
