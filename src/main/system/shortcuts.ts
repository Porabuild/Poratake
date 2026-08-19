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
import { isFeatureSupported } from '@/main/system/capabilities';
import type { FeatureId } from '@/types/capabilities';

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

const registeredShortcuts = new Map<string, string>();

const ACTION_FEATURES: Partial<Record<ShortcutAction, FeatureId>> = {
  window: 'screenshot-window',
  captureText: 'ocr',
  scanQRCode: 'qrcode',
  timerCapture: 'timer-capture',
  recordArea: 'recording',
  recordScreen: 'recording',
  recordWindow: 'recording',
  allInOne: 'all-in-one',
};

function isActionSupported(action: ShortcutAction): boolean {
  const feature = ACTION_FEATURES[action];
  return !feature || isFeatureSupported(feature);
}

function registerScreenshotShortcut(
  action: CaptureMode,
  accelerator: string
): boolean {
  try {
    const oldAccelerator = registeredShortcuts.get(action);
    if (oldAccelerator) {
      globalShortcut.unregister(oldAccelerator);
    }

    if (accelerator) {
      const success = globalShortcut.register(accelerator, () => {
        screenshot(action);
      });

      if (success) {
        registeredShortcuts.set(action, accelerator);
        return true;
      } else {
        console.error(`Failed to register shortcut: ${accelerator}`);
        return false;
      }
    } else {
      registeredShortcuts.delete(action);
      return true;
    }
  } catch (error) {
    console.error(`Error registering shortcut for ${action}:`, error);
    return false;
  }
}

function registerCaptureTextShortcut(accelerator: string): boolean {
  try {
    const oldAccelerator = registeredShortcuts.get('captureText');
    if (oldAccelerator) {
      globalShortcut.unregister(oldAccelerator);
    }

    if (accelerator) {
      const success = globalShortcut.register(accelerator, () => {
        captureText();
      });

      if (success) {
        registeredShortcuts.set('captureText', accelerator);
        return true;
      } else {
        console.error(
          `Failed to register captureText shortcut: ${accelerator}`
        );
        return false;
      }
    } else {
      registeredShortcuts.delete('captureText');
      return true;
    }
  } catch (error) {
    console.error('Error registering captureText shortcut:', error);
    return false;
  }
}

function registerScanQRCodeShortcut(accelerator: string): boolean {
  try {
    const oldAccelerator = registeredShortcuts.get('scanQRCode');
    if (oldAccelerator) {
      globalShortcut.unregister(oldAccelerator);
    }

    if (accelerator) {
      const success = globalShortcut.register(accelerator, () => {
        scanQRCode();
      });

      if (success) {
        registeredShortcuts.set('scanQRCode', accelerator);
        return true;
      } else {
        console.error(`Failed to register scanQRCode shortcut: ${accelerator}`);
        return false;
      }
    } else {
      registeredShortcuts.delete('scanQRCode');
      return true;
    }
  } catch (error) {
    console.error('Error registering scanQRCode shortcut:', error);
    return false;
  }
}

function registerTimerCaptureShortcut(accelerator: string): boolean {
  try {
    const oldAccelerator = registeredShortcuts.get('timerCapture');
    if (oldAccelerator) {
      globalShortcut.unregister(oldAccelerator);
    }

    if (accelerator) {
      const success = globalShortcut.register(accelerator, () => {
        timerCapture();
      });

      if (success) {
        registeredShortcuts.set('timerCapture', accelerator);
        return true;
      } else {
        console.error(
          `Failed to register timerCapture shortcut: ${accelerator}`
        );
        return false;
      }
    } else {
      registeredShortcuts.delete('timerCapture');
      return true;
    }
  } catch (error) {
    console.error('Error registering timerCapture shortcut:', error);
    return false;
  }
}

function registerRecordingShortcut(
  action: 'recordArea' | 'recordScreen' | 'recordWindow',
  accelerator: string
): boolean {
  try {
    const oldAccelerator = registeredShortcuts.get(action);
    if (oldAccelerator) {
      globalShortcut.unregister(oldAccelerator);
    }

    if (accelerator) {
      const success = globalShortcut.register(accelerator, () => {
        if (action === 'recordArea') {
          recordArea();
        } else if (action === 'recordScreen') {
          recordScreen();
        } else {
          recordWindow();
        }
      });

      if (success) {
        registeredShortcuts.set(action, accelerator);
        return true;
      } else {
        console.error(`Failed to register ${action} shortcut: ${accelerator}`);
        return false;
      }
    } else {
      registeredShortcuts.delete(action);
      return true;
    }
  } catch (error) {
    console.error(`Error registering ${action} shortcut:`, error);
    return false;
  }
}

function registerHistoryShortcut(accelerator: string): boolean {
  try {
    const oldAccelerator = registeredShortcuts.get('history');
    if (oldAccelerator) {
      globalShortcut.unregister(oldAccelerator);
    }

    if (accelerator) {
      const success = globalShortcut.register(accelerator, () => {
        const tray = getTray();
        toggleHistoryPopover(tray?.getBounds());
      });

      if (success) {
        registeredShortcuts.set('history', accelerator);
        return true;
      } else {
        console.error(`Failed to register history shortcut: ${accelerator}`);
        return false;
      }
    } else {
      registeredShortcuts.delete('history');
      return true;
    }
  } catch (error) {
    console.error('Error registering history shortcut:', error);
    return false;
  }
}

function registerAllInOneShortcut(accelerator: string): boolean {
  try {
    const oldAccelerator = registeredShortcuts.get('allInOne');
    if (oldAccelerator) {
      globalShortcut.unregister(oldAccelerator);
    }

    if (accelerator) {
      const success = globalShortcut.register(accelerator, () => {
        startAllInOne();
      });

      if (success) {
        registeredShortcuts.set('allInOne', accelerator);
        return true;
      } else {
        console.error(`Failed to register allInOne shortcut: ${accelerator}`);
        return false;
      }
    } else {
      registeredShortcuts.delete('allInOne');
      return true;
    }
  } catch (error) {
    console.error('Error registering allInOne shortcut:', error);
    return false;
  }
}

function registerOpenInEditorShortcut(accelerator: string): boolean {
  try {
    const oldAccelerator = registeredShortcuts.get('openInEditor');
    if (oldAccelerator) {
      globalShortcut.unregister(oldAccelerator);
    }

    if (accelerator) {
      const success = globalShortcut.register(accelerator, () => {
        openImageInEditor();
      });

      if (success) {
        registeredShortcuts.set('openInEditor', accelerator);
        return true;
      } else {
        console.error(
          `Failed to register openInEditor shortcut: ${accelerator}`
        );
        return false;
      }
    } else {
      registeredShortcuts.delete('openInEditor');
      return true;
    }
  } catch (error) {
    console.error('Error registering openInEditor shortcut:', error);
    return false;
  }
}

function registerClipboardInEditorShortcut(accelerator: string): boolean {
  try {
    const oldAccelerator = registeredShortcuts.get('clipboardInEditor');
    if (oldAccelerator) {
      globalShortcut.unregister(oldAccelerator);
    }

    if (accelerator) {
      const success = globalShortcut.register(accelerator, () => {
        openClipboardInEditor();
      });

      if (success) {
        registeredShortcuts.set('clipboardInEditor', accelerator);
        return true;
      } else {
        console.error(
          `Failed to register clipboardInEditor shortcut: ${accelerator}`
        );
        return false;
      }
    } else {
      registeredShortcuts.delete('clipboardInEditor');
      return true;
    }
  } catch (error) {
    console.error('Error registering clipboardInEditor shortcut:', error);
    return false;
  }
}

function registerShortcut(
  action: ShortcutAction,
  accelerator: string
): boolean {
  if (!isActionSupported(action)) {
    return false;
  }

  switch (action) {
    case 'captureText':
      return registerCaptureTextShortcut(accelerator);
    case 'scanQRCode':
      return registerScanQRCodeShortcut(accelerator);
    case 'timerCapture':
      return registerTimerCaptureShortcut(accelerator);
    case 'recordArea':
    case 'recordScreen':
    case 'recordWindow':
      return registerRecordingShortcut(action, accelerator);
    case 'history':
      return registerHistoryShortcut(accelerator);
    case 'allInOne':
      return registerAllInOneShortcut(accelerator);
    case 'openInEditor':
      return registerOpenInEditorShortcut(accelerator);
    case 'clipboardInEditor':
      return registerClipboardInEditorShortcut(accelerator);
    default:
      return registerScreenshotShortcut(action as CaptureMode, accelerator);
  }
}

export function registerAllShortcuts(): void {
  const config = getConfig();
  const shortcuts = config.shortcuts;

  registerShortcut('area', shortcuts.screenshot.area);
  registerShortcut('window', shortcuts.screenshot.window);
  registerShortcut('screen', shortcuts.screenshot.screen);
  registerShortcut('captureText', shortcuts.captureText);
  registerShortcut('scanQRCode', shortcuts.scanQRCode ?? '');
  registerShortcut('timerCapture', shortcuts.timerCapture ?? '');
  registerShortcut('recordArea', shortcuts.recording.area);
  registerShortcut('recordScreen', shortcuts.recording.screen);
  registerShortcut('recordWindow', shortcuts.recording.window);
  registerShortcut('history', shortcuts.history ?? '');
  registerShortcut('allInOne', shortcuts.allInOne ?? '');
  registerShortcut('openInEditor', shortcuts.openInEditor ?? '');
  registerShortcut('clipboardInEditor', shortcuts.clipboardInEditor ?? '');
}

export function unregisterAllShortcuts(): void {
  globalShortcut.unregisterAll();
  registeredShortcuts.clear();
}

export function init(): void {
  registerAllShortcuts();

  ipcMain.on(
    'shortcuts:register',
    (_event, action: CaptureMode, accelerator: string) => {
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
