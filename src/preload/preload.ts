import { ipcRenderer, contextBridge } from 'electron';

contextBridge.exposeInMainWorld('appPlatform', process.platform);

contextBridge.exposeInMainWorld('ipcRenderer', {
  on(...args: Parameters<typeof ipcRenderer.on>) {
    const [channel, listener] = args;
    const wrappedListener = (
      event: Electron.IpcRendererEvent,
      ...listenerArgs: unknown[]
    ) => listener(event, ...listenerArgs);
    ipcRenderer.on(channel, wrappedListener);
    return () => {
      ipcRenderer.off(channel, wrappedListener);
    };
  },
  send(...args: Parameters<typeof ipcRenderer.send>) {
    const [channel, ...omit] = args;
    return ipcRenderer.send(channel, ...omit);
  },
  invoke(...args: Parameters<typeof ipcRenderer.invoke>) {
    const [channel, ...omit] = args;
    return ipcRenderer.invoke(channel, ...omit);
  },
});
