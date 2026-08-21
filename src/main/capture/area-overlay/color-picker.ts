import { app } from 'electron';
import type { BrowserWindow, Display } from 'electron';
import fs from 'fs';
import path from 'path';
import { randomUUID } from 'crypto';
import { pathToFileURL } from 'url';
import { captureRegionToFile } from '@/main/capture/screenshot/native-capture';

export interface ColorPickerHost {
  entries: { window: BrowserWindow }[];
  displays: Map<number, Display>;
  freeze: boolean;
  colorPickerActive: boolean;
  colorFramePaths: Map<number, string>;
  colorFrameVersions: Map<number, number>;
}

export function removeColorFrame(
  host: ColorPickerHost,
  displayId: number
): void {
  const filePath = host.colorFramePaths.get(displayId);
  if (!filePath) return;

  host.colorFramePaths.delete(displayId);
  void fs.promises.unlink(filePath).catch(() => {});
}

export function setColorPickerActive(
  host: ColorPickerHost,
  active: boolean,
  setEscapeShortcutEnabled: (enabled: boolean) => void
): void {
  if (host.colorPickerActive === active) return;

  host.colorPickerActive = active;
  setEscapeShortcutEnabled(!active);

  for (const entry of host.entries) {
    if (entry.window.isDestroyed()) continue;
    entry.window.webContents.send('area-overlay:set-color-picker', active);
  }

  if (active) return;

  for (const displayId of host.displays.keys()) {
    const version = (host.colorFrameVersions.get(displayId) ?? 0) + 1;
    host.colorFrameVersions.set(displayId, version);
    removeColorFrame(host, displayId);
  }
}

export async function captureColorFrame(
  host: ColorPickerHost,
  display: Display,
  isCurrent: () => boolean
): Promise<{ url: string } | null> {
  const version = (host.colorFrameVersions.get(display.id) ?? 0) + 1;
  host.colorFrameVersions.set(display.id, version);
  removeColorFrame(host, display.id);

  const filePath = path.join(
    app.getPath('temp'),
    `poratake-color-frame-${randomUUID()}.png`
  );
  const captured = await captureRegionToFile(display.bounds, filePath, {
    cached: host.freeze,
  });
  if (
    !captured ||
    !isCurrent() ||
    !host.colorPickerActive ||
    host.colorFrameVersions.get(display.id) !== version
  ) {
    void fs.promises.unlink(filePath).catch(() => {});
    return null;
  }

  host.colorFramePaths.set(display.id, filePath);
  return { url: pathToFileURL(filePath).href };
}
