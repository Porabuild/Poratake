import { screen } from 'electron';
import { getConfig, updateConfig } from '@/main/settings';
import { daemon } from '@/main/daemon';
import {
  startAreaSelection,
  cancelAreaSelection,
} from '@/main/capture/area-selector';
import { selectAreaWithOverlay } from '@/main/capture/area-overlay';
import { isFreezeScreenEnabled } from '@/main/capture/freeze-screen/preference';
import { captureArea } from '@/main/capture/screenshot';
import {
  hideDesktopIcons,
  showDesktopIcons,
  isSupported as isDesktopIconsSupported,
  checkAccessibilityPermission,
} from '@/main/capture/desktop-icons';
import { isFeatureSupported } from '@/main/system/capabilities';
import { isMac } from '@/main/utils/platform';

const TIMER_DURATION = 5;
const WINDOW_WIDTH = 140;
const WINDOW_HEIGHT = 52;
const TIMER_TOP_MARGIN = 20;

let isTimerActive = false;

interface TimerControlPosition {
  x: number;
  y: number;
}

interface AreaRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

function calculateTimerPosition(area: AreaRect): TimerControlPosition {
  const x = Math.round(area.x + area.width / 2 - WINDOW_WIDTH / 2);
  let y = Math.round(area.y - WINDOW_HEIGHT - TIMER_TOP_MARGIN);

  if (y < TIMER_TOP_MARGIN) {
    y = TIMER_TOP_MARGIN;
  }

  return { x, y };
}

async function showTimerControl(
  position: TimerControlPosition
): Promise<boolean> {
  try {
    await daemon.call('timer-control', 'show', {
      x: position.x,
      y: position.y,
      duration: TIMER_DURATION,
    });
    return true;
  } catch (error) {
    console.error('Failed to show timer control:', error);
    return false;
  }
}

async function hideTimerControl(): Promise<void> {
  try {
    await daemon.call('timer-control', 'hide');
  } catch (error) {
    console.error('Failed to hide timer control:', error);
  }
}

function createTimerCompletionWaiter(): {
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

function resolveHideIcons(): boolean {
  const config = getConfig();
  let shouldHideIcons =
    config.screenshot.hideDesktopIcons && isDesktopIconsSupported();

  if (shouldHideIcons && !checkAccessibilityPermission(false)) {
    shouldHideIcons = false;
    updateConfig({
      screenshot: { ...config.screenshot, hideDesktopIcons: false },
    });
  }

  return shouldHideIcons;
}

async function timerCaptureWindows(shouldHideIcons: boolean): Promise<void> {
  let timerRequested = false;
  try {
    if (shouldHideIcons) {
      await hideDesktopIcons('capture');
    }

    const selection = await selectAreaWithOverlay({
      freeze: isFreezeScreenEnabled(),
    });
    if (!selection) {
      return;
    }

    await selection.release();
    const globalArea = selection.rect;

    const dipPosition = calculateTimerPosition(globalArea);
    const position = screen.dipToScreenPoint({
      x: dipPosition.x,
      y: dipPosition.y,
    });

    const timerCompletion = createTimerCompletionWaiter();
    timerRequested = true;
    if (!(await showTimerControl(position))) {
      timerCompletion.cancel();
      return;
    }

    if (!(await timerCompletion.result)) {
      return;
    }

    await captureArea({ status: 'confirmed', ...globalArea });
  } catch (error) {
    console.error('Timer capture failed:', error);
  } finally {
    if (timerRequested) {
      await hideTimerControl();
    }
    if (shouldHideIcons) {
      await showDesktopIcons('capture');
    }
    isTimerActive = false;
  }
}

async function timerCaptureMac(shouldHideIcons: boolean): Promise<void> {
  let iconsRestored = !shouldHideIcons;
  const restoreIcons = async () => {
    if (iconsRestored) return;

    iconsRestored = true;
    await showDesktopIcons('capture');
  };

  try {
    await new Promise<void>(resolve => {
      let isSettled = false;
      let isCancelled = false;
      let hasSelection = false;
      let timerCompletion: ReturnType<
        typeof createTimerCompletionWaiter
      > | null = null;

      const settle = () => {
        if (isSettled) return;

        isSettled = true;
        timerCompletion?.cancel();
        resolve();
      };

      const handleCancelled = () => {
        isCancelled = true;
        timerCompletion?.cancel();
        if (!hasSelection) {
          settle();
        }
      };

      const handleSelected = async (selection: {
        x?: number;
        y?: number;
        width?: number;
        height?: number;
      }) => {
        if (hasSelection || isSettled) return;
        if (
          selection.x === undefined ||
          selection.y === undefined ||
          selection.width === undefined ||
          selection.height === undefined
        ) {
          return;
        }

        hasSelection = true;
        const position = calculateTimerPosition({
          x: selection.x,
          y: selection.y,
          width: selection.width,
          height: selection.height,
        });

        timerCompletion = createTimerCompletionWaiter();
        if (!(await showTimerControl(position)) || isCancelled || isSettled) {
          await cancelAreaSelection(true);
          settle();
          return;
        }

        const shouldCapture = await timerCompletion.result;
        timerCompletion = null;
        if (!shouldCapture || isCancelled || isSettled) {
          await cancelAreaSelection(true);
          settle();
          return;
        }

        await hideTimerControl();
        await cancelAreaSelection(true);

        try {
          await captureArea(
            {
              status: 'confirmed',
              x: selection.x,
              y: selection.y,
              width: selection.width,
              height: selection.height,
            },
            { onCaptured: restoreIcons }
          );
        } catch (error) {
          console.error('Timer capture failed:', error);
        }
        settle();
      };

      void startAreaSelection({
        onSelected: handleSelected,
        onCancelled: handleCancelled,
        showPrompt: false,
      }).then(
        selection => {
          if (!selection && !hasSelection) {
            settle();
          }
        },
        error => {
          console.error('Timer area selection failed:', error);
          settle();
        }
      );
    });
  } finally {
    await hideTimerControl();
    await restoreIcons();
    isTimerActive = false;
  }
}

export default async function timerCapture(): Promise<void> {
  if (!isFeatureSupported('timer-capture')) {
    return;
  }

  if (isTimerActive) {
    return;
  }

  const shouldHideIcons = resolveHideIcons();
  isTimerActive = true;

  if (isMac) {
    if (shouldHideIcons) {
      await hideDesktopIcons('capture');
    }
    return timerCaptureMac(shouldHideIcons);
  }

  return timerCaptureWindows(shouldHideIcons);
}
