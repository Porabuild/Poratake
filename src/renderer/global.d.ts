import type { IpcRenderer } from 'electron';

type IpcRendererBridge = Pick<IpcRenderer, 'send' | 'invoke'> & {
  on: (
    channel: Parameters<IpcRenderer['on']>[0],
    listener: Parameters<IpcRenderer['on']>[1]
  ) => () => void;
};

declare global {
  interface Window {
    appPlatform: NodeJS.Platform;
    ipcRenderer: IpcRendererBridge;
  }
}

export {};
