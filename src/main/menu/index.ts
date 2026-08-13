import {
  app,
  Tray,
  Menu,
  nativeImage,
  NativeImage,
  nativeTheme,
  dialog,
  BrowserWindow,
  shell,
} from 'electron';
import path from 'path';
import fs from 'fs';
import { updateConfig } from '@/main/settings';
import screenshot from '@/main/capture/screenshot';
import captureText from '@/main/capture/ocr';
import scanQRCode from '@/main/capture/qrcode';
import timerCapture from '@/main/capture/timer-capture';
import scrollCapture from '@/main/capture/scroll-capture';
import recordArea, {
  isRecording,
  recordScreen,
  recordWindow,
} from '@/main/capture/video';
import { createOrShowSettingsWindow } from '@/main/settings';
import { preloadHistoryPopover, toggleHistoryPopover } from '@/main/history';
import {
  hideDesktopIcons,
  showDesktopIcons,
  areDesktopIconsHidden,
  isSupported as isDesktopIconsSupported,
  checkAccessibilityPermission,
} from '@/main/capture/desktop-icons';
import { getConfig } from '@/main/settings';
import { getUpdateState } from '@/main/update';
import { isProduction } from '@/main/utils/env.ts';
import startAllInOne from '@/main/capture/all-in-one';
import {
  openImageInEditor,
  openClipboardInEditor,
} from '@/main/capture/screenshot/open-editor';
import { openImageToPin } from '@/main/capture/screenshot/pin';
import { openVideoInEditor } from '@/main/capture/video/video-editor';
import { isFeatureSupported } from '@/main/system/capabilities';
import type { FeatureId } from '@/types/capabilities';
import { isMac, isWindows } from '@/main/utils/platform';
import { getPublicAssetPath } from '@/main/utils/paths';

if (!isMac) {
  Menu.setApplicationMenu(null);
}

type GatedMenuItem = Electron.MenuItemConstructorOptions & {
  feature?: FeatureId;
};

function pruneMenuItems(
  items: GatedMenuItem[]
): Electron.MenuItemConstructorOptions[] {
  const result: Electron.MenuItemConstructorOptions[] = [];

  for (const { feature, ...item } of items) {
    if (feature && !isFeatureSupported(feature)) {
      continue;
    }

    const isSeparator = item.type === 'separator';
    const previous = result[result.length - 1];
    if (isSeparator && (!previous || previous.type === 'separator')) {
      continue;
    }

    result.push(item);
  }

  while (result.length && result[result.length - 1].type === 'separator') {
    result.pop();
  }

  return result;
}

let tray: Tray | null = null;
let menuIcons: Record<string, NativeImage | undefined> | null = null;
let isThemeListenerAttached = false;

const MENU_ICON_SIZE = 16;

function getIconsDir(): string {
  if (isProduction) {
    return path.join(process.resourcesPath, 'menu-icons');
  }
  return path.join(app.getAppPath(), 'src/main/menu/icons');
}

function tintToMenuForeground(icon: NativeImage): NativeImage {
  if (!nativeTheme.shouldUseDarkColors) {
    return icon;
  }

  const { width, height } = icon.getSize();
  const bitmap = icon.toBitmap();

  for (let i = 0; i < bitmap.length; i += 4) {
    const alpha = bitmap[i + 3];
    bitmap[i] = alpha;
    bitmap[i + 1] = alpha;
    bitmap[i + 2] = alpha;
  }

  return nativeImage.createFromBuffer(bitmap, { width, height });
}

function createMenuIcon(iconName: string): NativeImage | undefined {
  try {
    const pngPath = path.join(getIconsDir(), `${iconName}.png`);
    if (!fs.existsSync(pngPath)) {
      console.warn(`Menu icon not found: ${pngPath}`);
      return undefined;
    }

    const icon = nativeImage.createFromPath(pngPath);
    if (icon.isEmpty()) {
      console.warn(`Menu icon is empty: ${pngPath}`);
      return undefined;
    }

    const resized = icon.resize({
      width: MENU_ICON_SIZE,
      height: MENU_ICON_SIZE,
    });

    if (isMac) {
      resized.setTemplateImage(true);
      return resized;
    }

    return tintToMenuForeground(resized);
  } catch (e) {
    console.error(`Failed to load menu icon: ${iconName}`, e);
    return undefined;
  }
}

function getMenuIcons(): Record<string, NativeImage | undefined> {
  if (!menuIcons) {
    menuIcons = {
      allInOne: createMenuIcon('box'),
      captureScreen: createMenuIcon('monitor'),
      captureArea: createMenuIcon('scan'),
      captureWindow: createMenuIcon('app-window'),
      captureText: createMenuIcon('text-cursor'),
      scanQRCode: createMenuIcon('qr-code'),
      timerCapture: createMenuIcon('timer-reset'),
      scrollCapture: createMenuIcon('scroll'),
      history: createMenuIcon('history'),
      openInEditor: createMenuIcon('pencil'),
      clipboardInEditor: createMenuIcon('pencil'),
      openInVideoEditor: createMenuIcon('film'),
      pin: createMenuIcon('pin'),
      desktopIcons: createMenuIcon('monitor-dot'),
      settings: createMenuIcon('settings'),
      aperture: createMenuIcon('aperture'),
      quit: createMenuIcon('power'),
    };
  }
  return menuIcons;
}

function buildContextMenu(): Menu {
  const icons = getMenuIcons();
  const config = getConfig();
  const screenshotShortcuts = config.shortcuts.screenshot;
  const recordingShortcuts = config.shortcuts.recording;
  const updateState = getUpdateState();

  const menuItems: GatedMenuItem[] = [];

  if (updateState.status === 'ready') {
    menuItems.push(
      {
        label: `Update Ready (v${updateState.latestVersion})`,
        icon: createMenuIcon('rotate-ccw'),
        click: () => {
          createOrShowSettingsWindow('about');
        },
      },
      { type: 'separator' }
    );
  } else if (updateState.status === 'downloading') {
    menuItems.push(
      {
        label: `Downloading Update (${updateState.downloadProgress}%)...`,
        icon: createMenuIcon('rotate-ccw'),
        enabled: false,
      },
      { type: 'separator' }
    );
  } else if (updateState.status === 'available') {
    menuItems.push(
      {
        label: `Update Available (v${updateState.latestVersion})`,
        icon: createMenuIcon('rotate-ccw'),
        enabled: false,
      },
      { type: 'separator' }
    );
  }

  menuItems.push(
    {
      feature: 'all-in-one',
      label: 'All-in-one',
      icon: icons.allInOne,
      accelerator: config.shortcuts.allInOne || undefined,
      click: () => {
        startAllInOne();
      },
    },
    {
      type: 'separator',
    },
    {
      label: 'Capture Screen',
      icon: icons.captureScreen,
      accelerator: screenshotShortcuts.screen || undefined,
      click: () => {
        screenshot('screen');
      },
    },
    {
      label: 'Capture Area',
      icon: icons.captureArea,
      accelerator: screenshotShortcuts.area || undefined,
      click: () => {
        screenshot('area');
      },
    },
    {
      feature: 'screenshot-window',
      label: 'Capture Window',
      icon: icons.captureWindow,
      accelerator: screenshotShortcuts.window || undefined,
      click: () => {
        screenshot('window');
      },
    },
    {
      feature: 'scroll-capture',
      label: 'Scroll Capture',
      icon: icons.scrollCapture,
      accelerator: config.shortcuts.scrollCapture || undefined,
      click: () => {
        scrollCapture();
      },
    },
    {
      feature: 'ocr',
      label: 'Capture Text (OCR)',
      icon: icons.captureText,
      accelerator: config.shortcuts.captureText || undefined,
      click: () => {
        captureText();
      },
    },
    {
      feature: 'qrcode',
      label: 'Scan QR Code',
      icon: icons.scanQRCode,
      accelerator: config.shortcuts.scanQRCode || undefined,
      click: () => {
        scanQRCode();
      },
    },
    {
      feature: 'timer-capture',
      label: 'Timer Capture',
      icon: icons.timerCapture,
      accelerator: config.shortcuts.timerCapture || undefined,
      click: () => {
        timerCapture();
      },
    },
    {
      label: 'Open in Editor',
      icon: icons.openInEditor,
      accelerator: config.shortcuts.openInEditor || undefined,
      click: () => {
        openImageInEditor();
      },
    },
    {
      label: 'Open Clipboard in Editor',
      icon: icons.clipboardInEditor,
      accelerator: config.shortcuts.clipboardInEditor || undefined,
      click: () => {
        openClipboardInEditor();
      },
    },
    {
      label: 'Pin',
      icon: icons.pin,
      click: () => {
        openImageToPin();
      },
    },
    {
      type: 'separator',
    },
    {
      feature: 'recording',
      label: 'Record Screen',
      icon: icons.captureScreen,
      accelerator: recordingShortcuts.screen || undefined,
      click: () => {
        if (isRecording()) {
          return;
        }
        recordScreen();
      },
    },
    {
      feature: 'recording',
      label: 'Record Area',
      icon: icons.captureArea,
      accelerator: recordingShortcuts.area || undefined,
      click: () => {
        if (isRecording()) {
          return;
        }
        recordArea();
      },
    },
    {
      feature: 'recording',
      label: 'Record Window',
      icon: icons.captureWindow,
      accelerator: recordingShortcuts.window || undefined,
      click: () => {
        if (isRecording()) {
          return;
        }
        recordWindow();
      },
    },
    {
      feature: 'video-editor',
      label: 'Open in Video Editor',
      icon: icons.openInVideoEditor,
      click: () => {
        openVideoInEditor();
      },
    },
    {
      type: 'separator',
    },
    {
      label: 'History',
      icon: icons.history,
      accelerator: config.shortcuts.history || undefined,
      click: () => {
        if (tray) {
          toggleHistoryPopover(tray.getBounds());
        }
      },
    },
    {
      feature: 'desktop-icons',
      label: areDesktopIconsHidden()
        ? 'Show Desktop Icons'
        : 'Hide Desktop Icons',
      icon: icons.desktopIcons,
      visible: isDesktopIconsSupported(),
      click: () => {
        const updateMenu = () => {
          if (tray) {
            tray.setContextMenu(buildContextMenu());
          }
        };
        if (areDesktopIconsHidden()) {
          showDesktopIcons('menu').then(updateMenu);
        } else {
          if (!checkAccessibilityPermission(true)) {
            return;
          }
          hideDesktopIcons('menu').then(updateMenu);
        }
      },
    },
    {
      type: 'separator',
    },
    {
      label: 'Settings...',
      icon: icons.settings,
      click: () => {
        createOrShowSettingsWindow();
      },
    },
    {
      label: isWindows ? 'Hide Tray Icon' : 'Hide Menu Bar Icon',
      icon: createMenuIcon('eye-off'),
      click: async () => {
        const options = {
          type: 'warning' as const,
          title: isWindows ? 'Hide Tray Icon' : 'Hide Menu Bar Icon',
          message: isWindows
            ? 'Are you sure you want to hide the tray icon?'
            : 'Are you sure you want to hide the menu bar icon?',
          detail: isWindows
            ? 'The app will continue running in the background. To restore the tray icon, launch Poratake again from the Start menu.'
            : 'The app will continue running in the background. To restore the menu bar icon, launch the app again (double-click Poratake in Applications).',
          buttons: ['Hide Icon', 'Cancel'],
          defaultId: 0,
          cancelId: 1,
        };

        const win = BrowserWindow.getFocusedWindow();
        const result = win
          ? await dialog.showMessageBox(win, options)
          : await dialog.showMessageBox(options);

        if (result.response !== 0) {
          return;
        }

        updateConfig({
          general: { ...config.general, hideMenuBarIcon: true },
        });
        destroyTray();
      },
    },
    {
      label: 'Poratake Issues',
      icon: icons.aperture,
      click: () => {
        shell.openExternal('https://github.com/Porabuild/Poratake/issues');
      },
    },
    {
      label: 'Quit',
      icon: icons.quit,
      click: () => {
        app.quit();
      },
    }
  );

  return Menu.buildFromTemplate(pruneMenuItems(menuItems));
}

function getTrayIconPath(): string {
  if (isWindows) {
    return getPublicAssetPath('tray-icon.png');
  }
  if (isProduction) {
    return path.join(process.resourcesPath, 'menu-icons', 'iconTemplate.png');
  }
  return path.join(app.getAppPath(), 'src/main/menu/dev/iconTemplate.png');
}

function createTrayIcon(): NativeImage {
  const icon = nativeImage.createFromPath(getTrayIconPath());
  if (isWindows && !icon.isEmpty()) {
    return icon.resize({ width: 16, height: 16 });
  }
  return icon;
}

export const init = async () => {
  const config = getConfig();

  if (config.general.hideMenuBarIcon) {
    return;
  }

  if (tray) {
    return;
  }

  tray = new Tray(createTrayIcon());

  tray.setContextMenu(buildContextMenu());

  if (!isMac && !isThemeListenerAttached) {
    isThemeListenerAttached = true;
    nativeTheme.on('updated', () => {
      menuIcons = null;
      rebuildTrayMenu();
    });
  }

  preloadHistoryPopover();
};

export function getTray(): Tray | null {
  return tray;
}

export function destroyTray(): void {
  if (tray) {
    tray.destroy();
    tray = null;
  }
}

export function rebuildTrayMenu(): void {
  if (tray) {
    tray.setContextMenu(buildContextMenu());
  }
}
