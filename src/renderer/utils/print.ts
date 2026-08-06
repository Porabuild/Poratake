export async function printImage(imageBase64: string): Promise<void> {
  await window.ipcRenderer.invoke('screenshot:print', imageBase64);
}
