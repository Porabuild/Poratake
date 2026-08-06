import {
  app,
  Tray,
  Menu,
  nativeImage,
  NativeImage,
  dialog,
  BrowserWindow,
  shell,
} from 'electron';
import { isPro } from '@/main/license/validation';
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

let tray: Tray | null = null;
let menuIcons: Record<string, NativeImage | undefined> | null = null;

const MENU_ICON_SIZE = 16;

function getIconsDir(): string {
  if (isProduction) {
    return path.join(process.resourcesPath, 'menu-icons');
  }
  return path.join(app.getAppPath(), 'src/main/menu/icons');
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
    resized.setTemplateImage(true);
    return resized;
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

  const menuItems: Electron.MenuItemConstructorOptions[] = [];

  if (!isPro()) {
    menuItems.push(
      {
        label: 'Upgrade to Capty Pro',
        click: () => {
          createOrShowSettingsWindow('license');
        },
      },
      { type: 'separator' }
    );
  }

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
      label: 'Capture Window',
      icon: icons.captureWindow,
      accelerator: screenshotShortcuts.window || undefined,
      click: () => {
        screenshot('window');
      },
    },
    {
      label: 'Scroll Capture',
      icon: icons.scrollCapture,
      accelerator: config.shortcuts.scrollCapture || undefined,
      click: () => {
        scrollCapture();
      },
    },
    {
      label: 'Capture Text (OCR)',
      icon: icons.captureText,
      accelerator: config.shortcuts.captureText || undefined,
      click: () => {
        captureText();
      },
    },
    {
      label: 'Scan QR Code',
      icon: icons.scanQRCode,
      accelerator: config.shortcuts.scanQRCode || undefined,
      click: () => {
        scanQRCode();
      },
    },
    {
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
      label: 'Hide Menu Bar Icon',
      icon: createMenuIcon('eye-off'),
      click: async () => {
        const options = {
          type: 'warning' as const,
          title: 'Hide Menu Bar Icon',
          message: 'Are you sure you want to hide the menu bar icon?',
          detail:
            'The app will continue running in the background. To restore the menu bar icon, launch the app again (double-click Capty in Applications).',
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
      label: 'Feature Request / Bug Report',
      icon: icons.aperture,
      click: () => {
        shell.openExternal('https://capty.app/roadmap');
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

  return Menu.buildFromTemplate(menuItems);
}

function getTrayIconPath(): string {
  if (isProduction) {
    return path.join(process.resourcesPath, 'menu-icons', 'iconTemplate.png');
  }
  return path.join(app.getAppPath(), 'src/main/menu/dev/iconTemplate.png');
}

export const init = async () => {
  const config = getConfig();

  if (config.general.hideMenuBarIcon) {
    return;
  }

  if (tray) {
    return;
  }

  const trayIconPath = getTrayIconPath();
  tray = new Tray(nativeImage.createFromPath(trayIconPath));

  tray.setContextMenu(buildContextMenu());

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
