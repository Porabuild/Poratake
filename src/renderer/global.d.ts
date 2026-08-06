import { IpcRenderer } from 'electron';

declare global {
  interface Window {
    ipcRenderer: {
      on: (
        channel: string,
        listener: (event: Electron.IpcRendererEvent, ...args: unknown[]) => void
      ) => IpcRenderer;
      off: (
        channel: string,
        listener: (event: Electron.IpcRendererEvent, ...args: unknown[]) => void
      ) => IpcRenderer;
      send: (channel: string, ...args: unknown[]) => void;
      invoke: (channel: string, ...args: unknown[]) => Promise<unknown>;
    };
  }
}

export {};
