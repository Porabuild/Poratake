import { getConfig, updateConfig } from '@/main/settings';
import {
  hideDesktopIcons,
  showDesktopIcons,
  isSupported as isDesktopIconsSupported,
  checkAccessibilityPermission,
} from '@/main/capture/desktop-icons';
import {
  startAreaSelection,
  cancelAreaSelection,
} from '@/main/capture/area-selector';
import { selectAreaWithOverlay } from '@/main/capture/area-overlay';
import { isFreezeScreenEnabled } from '@/main/capture/freeze-screen/preference';
import { captureArea } from '@/main/capture/screenshot';
import {
  calculateTimerPosition,
  showTimerControl,
  hideTimerControl,
  createTimerCompletionWaiter,
  type TimerAreaRect,
} from '@/main/capture/timer-control';
import { isFeatureSupported } from '@/main/system/capabilities';
import { isMac } from '@/main/utils/platform';

const TIMER_DURATION = 5;

let isTimerActive = false;

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
    const globalArea: TimerAreaRect = selection.rect;

    const position = calculateTimerPosition(globalArea);

    const timerCompletion = createTimerCompletionWaiter();
    timerRequested = true;
    if (!(await showTimerControl(position, TIMER_DURATION))) {
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
        if (
          !(await showTimerControl(position, TIMER_DURATION)) ||
          isCancelled ||
          isSettled
        ) {
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
