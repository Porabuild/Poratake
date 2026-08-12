import {
  cancelAreaSelection,
  hideAreaSelector,
  setAreaSelectionMode,
  setAreaSelectorAspectRatio,
  startAreaSelection,
  updateAreaSelection,
  updateAreaSelectionCallbacks,
} from '@/main/capture/area-selector';
import type { AreaSelectionMode } from '@/main/capture/area-selector';
import {
  showAllInOneControl,
  updateAllInOnePosition,
  hideAllInOneControl,
  getCurrentAreaSelection,
  setAllInOneCallbacks,
} from './open-all-in-one.ts';
import { captureArea } from '@/main/capture/screenshot/capture-area.ts';
import captureText from '@/main/capture/ocr';
import {
  showPreRecordingControl,
  updateRecordingControlPosition,
  hidePreRecordingControl,
  prewarmRecordingControlWindow,
} from '@/main/capture/video/recording-control.ts';
import { prewarmRecorder } from '@/main/capture/video/recorder.ts';
import {
  hideRecordingOverlay,
  prewarmOverlay,
  showRecordedWindowOutline,
} from '@/main/capture/video/overlay.ts';
import type { AreaSelection } from '@/types/area.ts';
import type {
  AllInOneCaptureMode,
  AllInOneCaptureTarget,
  AreaOverlayToolbar,
  AreaOverlayToolbarAction,
} from '@/types/area-overlay.ts';
import { clipboard, globalShortcut, Notification, screen } from 'electron';
import { updateConfig } from '@/main/settings';
import { isFeatureSupported } from '@/main/system/capabilities';
import { isWindows } from '@/main/utils/platform';
import { setOverlayToolbar } from '@/main/capture/area-overlay';
import { isScreenFrozen } from '@/main/capture/freeze-screen';
import { isFreezeScreenEnabled } from '@/main/capture/freeze-screen/preference';

export { showAllInOneControl, updateAllInOnePosition, hideAllInOneControl };

const SCREENSHOT_SHORTCUTS = ['C', 'Enter'];
const MIN_SELECTION_SIZE = 20;

type AreaBounds = { x: number; y: number; width: number; height: number };
type ManualSize = { width: number; height: number };
type TargetableMode = Exclude<AllInOneCaptureMode, 'ocr'>;

const SELECTION_MODES: Record<AllInOneCaptureTarget, AreaSelectionMode> = {
  area: 'manual',
  window: 'window',
  screen: 'display',
};

let activeCaptureMode: AllInOneCaptureMode = 'screenshot';
let captureTargets: Record<TargetableMode, AllInOneCaptureTarget> = {
  screenshot: 'area',
  record: 'area',
};
let currentSelection: AreaSelection | null = null;

function activeCaptureTarget(): AllInOneCaptureTarget {
  return activeCaptureMode === 'ocr'
    ? 'area'
    : captureTargets[activeCaptureMode];
}

function toolbarState(): AreaOverlayToolbar {
  return {
    kind: 'all-in-one',
    recordingEnabled: isFeatureSupported('recording'),
    ocrEnabled: isFeatureSupported('ocr'),
    activeMode: activeCaptureMode,
    activeTarget: activeCaptureTarget(),
  };
}

function persistAreaSelection(bounds: AreaBounds): void {
  updateConfig({ allInOne: { lastArea: bounds } });
}

function registerAllInOneShortcuts(
  onScreenshot: () => void,
  onRecord: () => void
): void {
  unregisterAllInOneShortcuts();
  globalShortcut.register('C', onScreenshot);
  globalShortcut.register('Enter', onScreenshot);
  if (isFeatureSupported('recording')) {
    globalShortcut.register('R', onRecord);
  }
}

function unregisterAllInOneShortcuts(): void {
  for (const shortcut of [...SCREENSHOT_SHORTCUTS, 'R']) {
    globalShortcut.unregister(shortcut);
  }
}

async function closeAreaSelection(): Promise<void> {
  await Promise.all([cancelAreaSelection(), hideAllInOneControl()]);
}

function extractBounds(selection: AreaSelection): {
  x: number;
  y: number;
  width: number;
  height: number;
} | null {
  if (
    selection.x === undefined ||
    selection.y === undefined ||
    selection.width === undefined ||
    selection.height === undefined
  ) {
    return null;
  }

  return {
    x: selection.x,
    y: selection.y,
    width: selection.width,
    height: selection.height,
  };
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function isValidManualSize(size: ManualSize): boolean {
  return (
    Number.isFinite(size.width) &&
    Number.isFinite(size.height) &&
    size.width > 0 &&
    size.height > 0
  );
}

function getDisplayForArea(area: AreaBounds): Electron.Display | null {
  const displays = screen.getAllDisplays();
  const centerX = area.x + area.width / 2;
  const centerY = area.y + area.height / 2;

  const displayContainingCenter = displays.find(display => {
    const { x, y, width, height } = display.bounds;
    return (
      centerX >= x &&
      centerX < x + width &&
      centerY >= y &&
      centerY < y + height
    );
  });

  if (displayContainingCenter) {
    return displayContainingCenter;
  }

  const displayContainingOrigin = displays.find(display => {
    const { x, y, width, height } = display.bounds;
    return (
      area.x >= x && area.x < x + width && area.y >= y && area.y < y + height
    );
  });

  return displayContainingOrigin ?? displays[0] ?? null;
}

function getBoundsForManualSize(
  area: AreaBounds,
  size: ManualSize
): AreaBounds | null {
  if (!isValidManualSize(size)) {
    return null;
  }

  const display = getDisplayForArea(area);
  if (!display) {
    return null;
  }

  const displayBounds = display.bounds;
  const width = clamp(
    Math.round(size.width),
    MIN_SELECTION_SIZE,
    displayBounds.width
  );
  const height = clamp(
    Math.round(size.height),
    MIN_SELECTION_SIZE,
    displayBounds.height
  );
  const centerX = area.x + area.width / 2;
  const centerY = area.y + area.height / 2;
  const maxX = displayBounds.x + displayBounds.width - width;
  const maxY = displayBounds.y + displayBounds.height - height;

  return {
    x: clamp(Math.round(centerX - width / 2), displayBounds.x, maxX),
    y: clamp(Math.round(centerY - height / 2), displayBounds.y, maxY),
    width,
    height,
  };
}

async function handleScreenshotAction(): Promise<void> {
  const area = getCurrentAreaSelection();
  if (!area) {
    return;
  }

  const windowId = currentSelection?.windowId;
  unregisterAllInOneShortcuts();
  const frozen = isScreenFrozen();

  if (!frozen) {
    await closeAreaSelection();
  }

  try {
    const selection = {
      status: 'confirmed' as const,
      x: area.x,
      y: area.y,
      width: area.width,
      height: area.height,
    };

    if (frozen) {
      await captureArea(selection, {
        cached: true,
        windowId,
        onCaptured: closeAreaSelection,
      });
    } else {
      await captureArea(selection, { windowId });
    }
    persistAreaSelection(area);
  } catch (error) {
    console.error('Failed to capture screenshot:', error);
  } finally {
    if (frozen) {
      await closeAreaSelection();
    }
  }
}

function handleRecordAction(): void {
  if (!isFeatureSupported('recording')) {
    return;
  }

  const area = getCurrentAreaSelection();
  if (!area) {
    return;
  }

  const windowId = currentSelection?.windowId;
  unregisterAllInOneShortcuts();
  prewarmRecordingControlWindow();
  prewarmRecorder();
  prewarmOverlay();

  hideAllInOneControl();

  updateAreaSelectionCallbacks({
    onUpdate: selection => {
      if (
        selection.status === 'updated' &&
        selection.x !== undefined &&
        selection.y !== undefined &&
        selection.width !== undefined &&
        selection.height !== undefined
      ) {
        updateRecordingControlPosition({
          x: selection.x,
          y: selection.y,
          width: selection.width,
          height: selection.height,
        });
      }
    },
    onCancelled: () => {
      hidePreRecordingControl();
      if (windowId !== undefined) {
        void hideRecordingOverlay(true);
      }
    },
  });

  showPreRecordingControl(area, currentSelection?.windowName);
  persistAreaSelection(area);

  if (windowId === undefined) {
    return;
  }

  void hideAreaSelector();
  void showRecordedWindowOutline(windowId);
}

async function handleOcrAction(): Promise<void> {
  const area = getCurrentAreaSelection();
  if (!area || !isFeatureSupported('ocr')) {
    return;
  }

  unregisterAllInOneShortcuts();
  const frozen = isScreenFrozen();

  if (!frozen) {
    await closeAreaSelection();
  }

  try {
    if (frozen) {
      await captureText(area, {
        cached: true,
        onCaptured: closeAreaSelection,
      });
    } else {
      await captureText(area);
    }
  } finally {
    if (frozen) {
      await closeAreaSelection();
    }
  }
}

function handleCopyColorAction(color: string): void {
  clipboard.writeText(color);
  new Notification({
    title: 'Color copied',
    body: `${color.toUpperCase()} copied to the clipboard`,
  }).show();
  handleCloseAction();
}

function applyActiveCaptureTarget(): void {
  void setAreaSelectionMode(SELECTION_MODES[activeCaptureTarget()]);
  setOverlayToolbar(toolbarState());
}

function handleCaptureModeSelected(mode: AllInOneCaptureMode): void {
  if (mode === 'record' && !isFeatureSupported('recording')) return;
  if (mode === 'ocr' && !isFeatureSupported('ocr')) return;
  if (mode === activeCaptureMode) return;

  activeCaptureMode = mode;
  applyActiveCaptureTarget();
}

function handleCaptureTargetSelected(target: AllInOneCaptureTarget): void {
  if (activeCaptureMode === 'ocr') return;
  if (captureTargets[activeCaptureMode] === target) return;

  captureTargets = { ...captureTargets, [activeCaptureMode]: target };
  applyActiveCaptureTarget();
}

function runActiveCaptureMode(): void {
  switch (activeCaptureMode) {
    case 'record':
      handleRecordAction();
      break;
    case 'ocr':
      void handleOcrAction();
      break;
    case 'screenshot':
      void handleScreenshotAction();
      break;
  }
}

async function handleUpdateSizeAction(size: ManualSize): Promise<void> {
  const area = getCurrentAreaSelection();
  if (!area) {
    return;
  }

  const bounds = getBoundsForManualSize(area, size);
  if (!bounds) {
    return;
  }

  const updated = await updateAreaSelection(bounds);
  if (!updated) {
    return;
  }

  persistAreaSelection(bounds);
  await updateAllInOnePosition(bounds);
}

function handleSizeEditorOpened(): void {
  unregisterAllInOneShortcuts();
}

function handleSizeEditorClosed(): void {
  const area = getCurrentAreaSelection();
  if (!area) {
    return;
  }

  registerAllInOneShortcuts(handleScreenshotAction, handleRecordAction);
}

function handleCloseAction(): void {
  currentSelection = null;
  unregisterAllInOneShortcuts();
  cancelAreaSelection();
  hideAllInOneControl();
}

function handleToolbarAction(action: AreaOverlayToolbarAction): void {
  switch (action.action) {
    case 'close':
      handleCloseAction();
      break;
    case 'screenshot':
      void handleScreenshotAction();
      break;
    case 'record':
      handleRecordAction();
      break;
    case 'ocr':
      void handleOcrAction();
      break;
    case 'copy-color':
      handleCopyColorAction(action.color);
      break;
    case 'select-capture-mode':
      handleCaptureModeSelected(action.mode);
      break;
    case 'select-capture-target':
      handleCaptureTargetSelected(action.target);
      break;
    case 'select-aspect-ratio':
      void setAreaSelectorAspectRatio({
        name: action.name,
        width: action.width,
        height: action.height,
      });
      break;
    case 'update-size':
      void handleUpdateSizeAction({
        width: action.width,
        height: action.height,
      });
      break;
    case 'size-editor-opened':
      handleSizeEditorOpened();
      break;
    case 'size-editor-closed':
      handleSizeEditorClosed();
      break;
  }
}

export default async function startAllInOne(): Promise<void> {
  if (!isFeatureSupported('all-in-one')) {
    return;
  }

  setAllInOneCallbacks({
    onClose: handleCloseAction,
    onScreenshot: handleScreenshotAction,
    onRecord: handleRecordAction,
    onUpdateSize: handleUpdateSizeAction,
    onSizeEditorOpened: handleSizeEditorOpened,
    onSizeEditorClosed: handleSizeEditorClosed,
  });

  activeCaptureMode = 'screenshot';
  captureTargets = { screenshot: 'area', record: 'area' };
  currentSelection = null;

  const handleSelected = (selection: AreaSelection) => {
    const bounds = extractBounds(selection);
    if (bounds) {
      currentSelection = selection;
      showAllInOneControl(bounds);
      if (isWindows) {
        queueMicrotask(runActiveCaptureMode);
        return;
      }
      registerAllInOneShortcuts(handleScreenshotAction, handleRecordAction);
    }
  };

  const handleUpdate = (selection: AreaSelection) => {
    const bounds = extractBounds(selection);
    if (bounds) {
      currentSelection = selection;
      updateAllInOnePosition(bounds);
    }
  };

  const handleCancelled = () => {
    currentSelection = null;
    unregisterAllInOneShortcuts();
  };

  const selection = await startAreaSelection({
    freeze: isWindows && isFreezeScreenEnabled(),
    toolbar: toolbarState(),
    onSelected: handleSelected,
    onUpdate: handleUpdate,
    onCancelled: handleCancelled,
    onToolbarAction: handleToolbarAction,
  });

  if (!selection) {
    unregisterAllInOneShortcuts();
    hideAllInOneControl();
    return;
  }

  const finalBounds = extractBounds(selection);
  if (finalBounds) {
    persistAreaSelection(finalBounds);
  }
}

export function init(): void {
  setAllInOneCallbacks({
    onClose: handleCloseAction,
    onScreenshot: handleScreenshotAction,
    onRecord: handleRecordAction,
    onUpdateSize: handleUpdateSizeAction,
    onSizeEditorOpened: handleSizeEditorOpened,
    onSizeEditorClosed: handleSizeEditorClosed,
  });
}
