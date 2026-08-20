import { globalShortcut, ipcMain, app } from 'electron';
import screenshot, {
  openImageInEditor,
  openClipboardInEditor,
} from '@/main/capture/screenshot';
import type { CaptureMode } from '@/main/capture/screenshot';
import captureText from '@/main/capture/ocr';
import scanQRCode from '@/main/capture/qrcode';
import timerCapture from '@/main/capture/timer-capture';
import recordArea, { recordScreen, recordWindow } from '@/main/capture/video';
import { toggleHistoryPopover } from '@/main/history';
import { getTray, rebuildTrayMenu } from '@/main/menu';
import { getConfig } from '@/main/settings';
import startAllInOne from '@/main/capture/all-in-one';
import { flushPendingContinuations } from '@/main/utils/event-loop';
import { isFeatureSupported } from '@/main/system/capabilities';
import type { FeatureId } from '@/types/capabilities';
import type { SettingsConfig } from '@/types/settings';

type ShortcutAction =
  | CaptureMode
  | 'captureText'
  | 'scanQRCode'
  | 'timerCapture'
  | 'recordArea'
  | 'recordScreen'
  | 'recordWindow'
  | 'history'
  | 'allInOne'
  | 'openInEditor'
  | 'clipboardInEditor';

type ShortcutsConfig = SettingsConfig['shortcuts'];

interface ShortcutDefinition {
  accelerator: (shortcuts: ShortcutsConfig) => string | undefined;
  run: () => void;
  feature?: FeatureId;
}

const SHORTCUTS: Record<ShortcutAction, ShortcutDefinition> = {
  area: {
    accelerator: shortcuts => shortcuts.screenshot.area,
    run: () => void screenshot('area'),
  },
  window: {
    accelerator: shortcuts => shortcuts.screenshot.window,
    run: () => void screenshot('window'),
    feature: 'screenshot-window',
  },
  screen: {
    accelerator: shortcuts => shortcuts.screenshot.screen,
    run: () => void screenshot('screen'),
  },
  captureText: {
    accelerator: shortcuts => shortcuts.captureText,
    run: () => void captureText(),
    feature: 'ocr',
  },
  scanQRCode: {
    accelerator: shortcuts => shortcuts.scanQRCode,
    run: () => void scanQRCode(),
    feature: 'qrcode',
  },
  timerCapture: {
    accelerator: shortcuts => shortcuts.timerCapture,
    run: () => void timerCapture(),
    feature: 'timer-capture',
  },
  recordArea: {
    accelerator: shortcuts => shortcuts.recording.area,
    run: () => void recordArea(),
    feature: 'recording',
  },
  recordScreen: {
    accelerator: shortcuts => shortcuts.recording.screen,
    run: () => void recordScreen(),
    feature: 'recording',
  },
  recordWindow: {
    accelerator: shortcuts => shortcuts.recording.window,
    run: () => void recordWindow(),
    feature: 'recording',
  },
  history: {
    accelerator: shortcuts => shortcuts.history,
    run: () => void toggleHistoryPopover(getTray()?.getBounds()),
  },
  allInOne: {
    accelerator: shortcuts => shortcuts.allInOne,
    run: () => void startAllInOne(),
    feature: 'all-in-one',
  },
  openInEditor: {
    accelerator: shortcuts => shortcuts.openInEditor,
    run: () => void openImageInEditor(),
  },
  clipboardInEditor: {
    accelerator: shortcuts => shortcuts.clipboardInEditor,
    run: () => void openClipboardInEditor(),
  },
};

const SHORTCUT_ACTIONS = Object.keys(SHORTCUTS) as ShortcutAction[];

const registeredShortcuts = new Map<ShortcutAction, string>();

function isShortcutAction(value: string): value is ShortcutAction {
  return Object.hasOwn(SHORTCUTS, value);
}

function registerShortcut(
  action: ShortcutAction,
  accelerator: string
): boolean {
  const definition = SHORTCUTS[action];
  if (definition.feature && !isFeatureSupported(definition.feature)) {
    return false;
  }

  try {
    const previous = registeredShortcuts.get(action);
    if (previous) {
      globalShortcut.unregister(previous);
    }

    if (!accelerator) {
      registeredShortcuts.delete(action);
      return true;
    }

    const success = globalShortcut.register(accelerator, () => {
      definition.run();
      flushPendingContinuations();
    });

    if (!success) {
      console.error(`Failed to register ${action} shortcut: ${accelerator}`);
      return false;
    }

    registeredShortcuts.set(action, accelerator);
    return true;
  } catch (error) {
    console.error(`Error registering shortcut for ${action}:`, error);
    return false;
  }
}

export function registerAllShortcuts(): void {
  const shortcuts = getConfig().shortcuts;

  for (const action of SHORTCUT_ACTIONS) {
    registerShortcut(action, SHORTCUTS[action].accelerator(shortcuts) ?? '');
  }
}

export function unregisterAllShortcuts(): void {
  globalShortcut.unregisterAll();
  registeredShortcuts.clear();
}

export function init(): void {
  registerAllShortcuts();

  ipcMain.on(
    'shortcuts:register',
    (_event, action: string, accelerator: string) => {
      if (!isShortcutAction(action)) {
        return;
      }
      registerShortcut(action, accelerator);
    }
  );

  app.on('will-quit', () => {
    unregisterAllShortcuts();
  });

  ipcMain.on('shortcuts:reload', () => {
    unregisterAllShortcuts();
    registerAllShortcuts();
    rebuildTrayMenu();
  });
}
