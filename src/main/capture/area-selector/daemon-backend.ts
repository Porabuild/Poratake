import { screen } from 'electron';
import type { AreaSelection } from '@/types/area';
import type { AspectRatio } from '@/types/aspect-ratio';
import { selectDisplay } from '@/main/capture/display-selector';
import { selectWindow } from '@/main/capture/window-selector';
import { daemon } from '@/main/daemon';
import { isWindows } from '@/main/utils/platform';
import type {
  AreaSelectionStyle,
  PresetArea,
  StartAreaSelectionOptions,
} from './types';

let pendingAreaSelection: AreaSelection | null = null;
let callbackCleanup: (() => void) | null = null;
let completionCleanup: (() => void) | null = null;
let completePendingSelection:
  ((selection: AreaSelection | null) => void) | null = null;

function handleDaemonEvent(
  event: string,
  data: unknown,
  options?: StartAreaSelectionOptions
): void {
  if (!event.startsWith('area-selector:')) return;

  const eventType = event.replace('area-selector:', '');
  const selection = data as AreaSelection;

  switch (eventType) {
    case 'selected':
      pendingAreaSelection = { ...selection, status: 'selected' };
      options?.onSelected?.(pendingAreaSelection);
      options?.onUpdate?.(pendingAreaSelection);
      break;
    case 'updated':
      pendingAreaSelection = { ...selection, status: 'updated' };
      options?.onUpdate?.(pendingAreaSelection);
      break;
    case 'confirmed':
      pendingAreaSelection = { ...selection, status: 'confirmed' };
      break;
    case 'cancelled':
      pendingAreaSelection = null;
      options?.onCancelled?.();
      break;
  }
}

function setupEventListener(options?: StartAreaSelectionOptions): void {
  callbackCleanup?.();

  const handler = (event: string, data: unknown) => {
    handleDaemonEvent(event, data, options);
  };

  daemon.onEvent(handler);
  callbackCleanup = () => {
    daemon.offEvent(handler);
    callbackCleanup = null;
  };
}

function cleanupEventListener(): void {
  callbackCleanup?.();
  completionCleanup?.();
  callbackCleanup = null;
  completionCleanup = null;
}

export function updateAreaSelectionCallbacks(
  options: StartAreaSelectionOptions
): void {
  setupEventListener(options);
}

async function startDaemonAreaSelector(
  params: {
    fullscreen?: boolean;
    displayId?: number;
    presetX?: number;
    presetY?: number;
    presetWidth?: number;
    presetHeight?: number;
    showPrompt?: boolean;
    style?: AreaSelectionStyle;
  },
  options?: StartAreaSelectionOptions
): Promise<AreaSelection | null> {
  completePendingSelection?.(null);
  setupEventListener(options);

  let settle: (selection: AreaSelection | null) => void = () => {};
  const result = new Promise<AreaSelection | null>(resolve => {
    let isPending = true;
    const handler = (event: string, data: unknown) => {
      if (event === 'area-selector:confirmed') {
        settle({ ...(data as AreaSelection), status: 'confirmed' });
      } else if (event === 'area-selector:cancelled') {
        settle(null);
      }
    };

    completionCleanup = () => {
      daemon.offEvent(handler);
      completionCleanup = null;
    };
    settle = selection => {
      if (!isPending) return;

      isPending = false;
      completePendingSelection = null;
      cleanupEventListener();
      resolve(selection);
    };
    completePendingSelection = settle;
    daemon.onEvent(handler);
  });

  try {
    await daemon.call('area-selector', 'start', params);
  } catch (error) {
    console.error('Failed to start area selector:', error);
    settle(null);
  }

  return result;
}

export async function startAreaSelection(
  options?: StartAreaSelectionOptions
): Promise<AreaSelection | null> {
  const mode = options?.mode ?? 'manual';

  if (mode === 'display') {
    const displays = screen.getAllDisplays();

    if (displays.length > 1) {
      const displaySelection = await selectDisplay();

      if (displaySelection.status === 'cancelled') {
        return null;
      }

      return startDaemonAreaSelector(
        {
          fullscreen: true,
          displayId: displaySelection.screenId,
          showPrompt: options?.showPrompt,
          style: options?.style,
        },
        options
      );
    }

    const primaryDisplay = screen.getPrimaryDisplay();
    return startDaemonAreaSelector(
      {
        fullscreen: true,
        ...(isWindows ? {} : { displayId: primaryDisplay.id }),
        showPrompt: options?.showPrompt,
        style: options?.style,
      },
      options
    );
  }

  if (mode === 'window') {
    const windowSelection = await selectWindow();

    if (windowSelection.status === 'cancelled') {
      return null;
    }

    if (windowSelection.status === 'error' || !windowSelection.bounds) {
      console.error('Window selection failed:', windowSelection);
      return null;
    }

    const windowBounds = isWindows
      ? screen.screenToDipRect(null, windowSelection.bounds)
      : windowSelection.bounds;
    const displays = screen.getAllDisplays();
    const windowCenterX = windowBounds.x + windowBounds.width / 2;
    const windowCenterY = windowBounds.y + windowBounds.height / 2;

    const targetDisplay =
      displays.find(display => {
        const { x, y, width, height } = display.bounds;
        return (
          windowCenterX >= x &&
          windowCenterX < x + width &&
          windowCenterY >= y &&
          windowCenterY < y + height
        );
      }) ?? screen.getPrimaryDisplay();

    return startDaemonAreaSelector(
      {
        ...(isWindows ? {} : { displayId: targetDisplay.id }),
        presetX: windowBounds.x,
        presetY: windowBounds.y,
        presetWidth: windowBounds.width,
        presetHeight: windowBounds.height,
        showPrompt: options?.showPrompt,
        style: options?.style,
      },
      options
    );
  }

  if (options?.preset) {
    const { x, y, width, height } = options.preset;
    const displays = screen.getAllDisplays();
    const centerX = x + width / 2;
    const centerY = y + height / 2;

    const targetDisplay =
      displays.find(display => {
        const bounds = display.bounds;
        return (
          centerX >= bounds.x &&
          centerX < bounds.x + bounds.width &&
          centerY >= bounds.y &&
          centerY < bounds.y + bounds.height
        );
      }) ?? screen.getPrimaryDisplay();

    return startDaemonAreaSelector(
      {
        ...(isWindows ? {} : { displayId: targetDisplay.id }),
        presetX: x,
        presetY: y,
        presetWidth: width,
        presetHeight: height,
        showPrompt: options?.showPrompt,
        style: options?.style,
      },
      options
    );
  }

  return startDaemonAreaSelector(
    {
      showPrompt: options?.showPrompt,
      style: options?.style,
    },
    options
  );
}

export async function confirmAreaSelection(): Promise<AreaSelection | null> {
  if (!pendingAreaSelection) {
    return null;
  }

  try {
    await daemon.call('area-selector', 'confirm');
    const selection = pendingAreaSelection;
    pendingAreaSelection = null;
    completePendingSelection?.(selection);
    cleanupEventListener();
    return selection;
  } catch (error) {
    console.error('Failed to confirm area selection:', error);
    return null;
  }
}

export async function cancelAreaSelection(
  silent: boolean = false
): Promise<void> {
  if (silent) {
    completePendingSelection?.(null);
    cleanupEventListener();
  }

  try {
    await daemon.call('area-selector', 'cancel');
  } catch (error) {
    console.error('Failed to cancel area selection:', error);
  }
  pendingAreaSelection = null;

  if (!silent) {
    completePendingSelection?.(null);
    cleanupEventListener();
  }
}

export function hasPendingSelection(): boolean {
  return pendingAreaSelection !== null;
}

export async function hideAreaSelector(): Promise<void> {
  try {
    await daemon.call('area-selector', 'hide');
  } catch (error) {
    console.error('Failed to hide area selector:', error);
  }
}

export async function showAreaSelector(): Promise<void> {
  try {
    await daemon.call('area-selector', 'show');
  } catch (error) {
    console.error('Failed to show area selector:', error);
  }
}

export async function updateAreaSelection(
  bounds: PresetArea
): Promise<boolean> {
  try {
    await daemon.call('area-selector', 'update', {
      x: bounds.x,
      y: bounds.y,
      width: bounds.width,
      height: bounds.height,
    });
    return true;
  } catch (error) {
    console.error('Failed to update area selection:', error);
    return false;
  }
}

export async function setAreaSelectorAspectRatio(
  ratio: AspectRatio
): Promise<void> {
  try {
    await daemon.call('area-selector', 'setAspectRatio', {
      width: ratio.width,
      height: ratio.height,
    });
  } catch (error) {
    console.error('Failed to set area selector aspect ratio:', error);
  }
}
