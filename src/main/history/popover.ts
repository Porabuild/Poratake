import {
  app,
  BrowserWindow,
  screen,
  ipcMain,
  type WebContents,
} from 'electron';
import path from 'path';
import { isDev, devServerUrl } from '@/main/utils/env';

let historyPopover: BrowserWindow | null = null;
let isReady = false;

const POPOVER_WIDTH = 400;
const POPOVER_HEIGHT = 500;
const POPOVER_GAP = 8;

function calculatePosition(trayBounds?: Electron.Rectangle): {
  x: number;
  y: number;
} {
  const primaryDisplay = screen.getPrimaryDisplay();
  const { width: screenWidth, height: screenHeight } =
    primaryDisplay.workAreaSize;

  if (trayBounds) {
    const x = Math.round(
      trayBounds.x + trayBounds.width / 2 - POPOVER_WIDTH / 2
    );
    const y = trayBounds.y + trayBounds.height + POPOVER_GAP;
    const maxY = screenHeight - POPOVER_HEIGHT - POPOVER_GAP;
    return { x, y: Math.min(y, maxY) };
  }

  return {
    x: Math.round((screenWidth - POPOVER_WIDTH) / 2),
    y: Math.round((screenHeight - POPOVER_HEIGHT) / 2),
  };
}

export function preloadHistoryPopover(): void {
  if (historyPopover && !historyPopover.isDestroyed()) {
    return;
  }

  historyPopover = new BrowserWindow({
    width: POPOVER_WIDTH,
    height: POPOVER_HEIGHT,
    x: -9999,
    y: -9999,
    frame: false,
    transparent: false,
    resizable: false,
    movable: false,
    alwaysOnTop: true,
    skipTaskbar: true,
    show: false,
    vibrancy: 'popover',
    visualEffectState: 'active',
    hasShadow: true,
    roundedCorners: true,
    backgroundColor: '#00000000',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      devTools: isDev,
    },
  });

  if (devServerUrl) {
    historyPopover.loadURL(`${devServerUrl}/history.html`);
  } else {
    historyPopover.loadFile(path.join(__dirname, '../dist/history.html'));
  }

  historyPopover.webContents.on('did-finish-load', () => {
    isReady = true;
  });

  historyPopover.on('blur', () => {
    closeHistoryPopover();
  });

  historyPopover.on('closed', () => {
    historyPopover = null;
    isReady = false;
  });
}

export function showHistoryPopover(trayBounds?: Electron.Rectangle): void {
  if (!historyPopover || historyPopover.isDestroyed()) {
    preloadHistoryPopover();
  }

  if (!historyPopover) return;

  historyPopover.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });

  const { x, y } = calculatePosition(trayBounds);
  historyPopover.setPosition(x, y);

  if (isReady) {
    historyPopover.webContents.send('history:refresh');
  }

  app.focus({ steal: true });
  historyPopover.show();
  historyPopover.focus();
}

export function closeHistoryPopover(): void {
  if (historyPopover && !historyPopover.isDestroyed()) {
    historyPopover.close();
  }
}

export function toggleHistoryPopover(trayBounds?: Electron.Rectangle): void {
  if (
    historyPopover &&
    !historyPopover.isDestroyed() &&
    historyPopover.isVisible()
  ) {
    closeHistoryPopover();
  } else {
    showHistoryPopover(trayBounds);
  }
}

export function getHistoryPopover(): BrowserWindow | null {
  return historyPopover;
}

export function isHistoryPopoverWebContents(sender: WebContents): boolean {
  return historyPopover?.webContents === sender;
}

export function isHistoryPopoverVisible(): boolean {
  return (
    historyPopover !== null &&
    !historyPopover.isDestroyed() &&
    historyPopover.isVisible()
  );
}

ipcMain.on('history:closePopover', event => {
  if (!isHistoryPopoverWebContents(event.sender)) return;

  closeHistoryPopover();
});

ipcMain.on('history:ready', event => {
  if (!isHistoryPopoverWebContents(event.sender)) return;
  if (!isHistoryPopoverVisible()) return;

  event.sender.send('history:refresh');
});
