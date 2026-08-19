import {
  cancelAreaSelection,
  hideAreaSelector,
  setAreaSelectionMode,
  startAreaSelection,
  setAreaSelectorFreeze,
  updateAreaSelectionCallbacks,
} from '@/main/capture/area-selector';
import type { AreaSelectionMode } from '@/main/capture/area-selector';
import {
  showAllInOneControl,
  updateAllInOnePosition,
  hideAllInOneControl,
  getCurrentAreaSelection,
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
import { clipboard, Notification } from 'electron';
import { getConfig, updateConfig } from '@/main/settings';
import { isFeatureSupported } from '@/main/system/capabilities';
import { debugLog, debugLogMs } from '@/main/utils/debug-log';
import { setOverlayToolbar } from '@/main/capture/area-overlay';
import { isScreenFrozen } from '@/main/capture/freeze-screen';
import { isFreezeScreenEnabled } from '@/main/capture/freeze-screen/preference';
import {
  DEFAULT_ALL_IN_ONE_CONFIG,
  type AllInOneConfig,
} from '@/types/settings';

export { showAllInOneControl, updateAllInOnePosition, hideAllInOneControl };

type AreaBounds = { x: number; y: number; width: number; height: number };
type TargetableMode = Exclude<AllInOneCaptureMode, 'ocr'>;

const SELECTION_MODES: Record<AllInOneCaptureTarget, AreaSelectionMode> = {
  area: 'manual',
  window: 'window',
  screen: 'display',
};
const CAPTURE_MODES = new Set<AllInOneCaptureMode>([
  'screenshot',
  'record',
  'ocr',
]);
const CAPTURE_TARGETS = new Set<AllInOneCaptureTarget>([
  'area',
  'window',
  'screen',
]);

let activeCaptureMode: AllInOneCaptureMode = 'screenshot';
let captureTargets: Record<TargetableMode, AllInOneCaptureTarget> = {
  screenshot: 'area',
  record: 'area',
};
let currentSelection: AreaSelection | null = null;

function isCaptureMode(value: unknown): value is AllInOneCaptureMode {
  return CAPTURE_MODES.has(value as AllInOneCaptureMode);
}

function isCaptureTarget(value: unknown): value is AllInOneCaptureTarget {
  return CAPTURE_TARGETS.has(value as AllInOneCaptureTarget);
}

function restorePreferences(): void {
  const preferences = getConfig().allInOne;
  if (!preferences.rememberChoices) {
    activeCaptureMode = DEFAULT_ALL_IN_ONE_CONFIG.lastMode;
    captureTargets = { ...DEFAULT_ALL_IN_ONE_CONFIG.lastTargets };
    return;
  }

  activeCaptureMode = isCaptureMode(preferences.lastMode)
    ? preferences.lastMode
    : DEFAULT_ALL_IN_ONE_CONFIG.lastMode;
  if (
    (activeCaptureMode === 'record' && !isFeatureSupported('recording')) ||
    (activeCaptureMode === 'ocr' && !isFeatureSupported('ocr'))
  ) {
    activeCaptureMode = DEFAULT_ALL_IN_ONE_CONFIG.lastMode;
  }
  captureTargets = {
    screenshot: isCaptureTarget(preferences.lastTargets?.screenshot)
      ? preferences.lastTargets.screenshot
      : DEFAULT_ALL_IN_ONE_CONFIG.lastTargets.screenshot,
    record: isCaptureTarget(preferences.lastTargets?.record)
      ? preferences.lastTargets.record
      : DEFAULT_ALL_IN_ONE_CONFIG.lastTargets.record,
  };
}

function persistPreferences(updates: Partial<AllInOneConfig>): void {
  const config = getConfig();
  if (!config.allInOne.rememberChoices) return;

  updateConfig({
    allInOne: {
      ...config.allInOne,
      ...updates,
    },
  });
}

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
  if (activeCaptureTarget() !== 'area') return;
  persistPreferences({ lastArea: bounds });
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

async function handleScreenshotAction(): Promise<void> {
  const area = getCurrentAreaSelection();
  if (!area) {
    return;
  }

  const windowId = currentSelection?.windowId;
  const windowBounds = currentSelection?.windowBounds;
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
        ...(windowBounds ? { windowBounds } : {}),
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

  showPreRecordingControl(area, currentSelection?.windowName, {
    preferenceScope: 'all-in-one',
  });
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
  const target = activeCaptureTarget();
  const shouldFreeze =
    activeCaptureMode !== 'record' && isFreezeScreenEnabled();
  void Promise.all([
    setAreaSelectorFreeze(shouldFreeze),
    setAreaSelectionMode(SELECTION_MODES[target]),
  ]);
  setOverlayToolbar(toolbarState());
}

function handleCaptureModeSelected(mode: AllInOneCaptureMode): void {
  if (mode === 'record' && !isFeatureSupported('recording')) return;
  if (mode === 'ocr' && !isFeatureSupported('ocr')) return;
  if (mode === activeCaptureMode) return;

  activeCaptureMode = mode;
  persistPreferences({ lastMode: mode });
  applyActiveCaptureTarget();
}

function handleCaptureTargetSelected(target: AllInOneCaptureTarget): void {
  if (activeCaptureMode === 'ocr') return;
  if (captureTargets[activeCaptureMode] === target) return;

  captureTargets = { ...captureTargets, [activeCaptureMode]: target };
  persistPreferences({ lastTargets: captureTargets });
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

function handleCloseAction(): void {
  currentSelection = null;
  cancelAreaSelection();
  hideAllInOneControl();
}

function handleToolbarAction(action: AreaOverlayToolbarAction): void {
  switch (action.action) {
    case 'close':
      handleCloseAction();
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
  }
}

export default async function startAllInOne(): Promise<void> {
  if (!isFeatureSupported('all-in-one')) {
    return;
  }

  const startedAt = performance.now();
  debugLog('aio', `trigger received mode=${activeCaptureMode}`);

  restorePreferences();
  currentSelection = null;

  const handleSelected = (selection: AreaSelection) => {
    const bounds = extractBounds(selection);
    if (bounds) {
      currentSelection = selection;
      showAllInOneControl(bounds);
      queueMicrotask(runActiveCaptureMode);
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
  };

  const freeze =
    activeCaptureMode === 'record' ? false : isFreezeScreenEnabled();
  const target = activeCaptureTarget();

  debugLog(
    'aio',
    `selection starting freeze=${freeze} target=${target} mode=${SELECTION_MODES[target]}`
  );
  const selection = await startAreaSelection({
    mode: SELECTION_MODES[target],
    requireDisplayPick: true,
    freeze,
    toolbar: toolbarState(),
    onSelected: handleSelected,
    onUpdate: handleUpdate,
    onCancelled: handleCancelled,
    onToolbarAction: handleToolbarAction,
  });

  debugLogMs('aio', 'selection finished', startedAt);

  if (!selection) {
    debugLog('aio', 'selection cancelled');
    hideAllInOneControl();
    return;
  }

  const finalBounds = extractBounds(selection);
  if (finalBounds) {
    persistAreaSelection(finalBounds);
  }
}
