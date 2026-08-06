import { useEffect, useState } from 'react';

function updatePrimaryCssVariable(color: string) {
  document.documentElement.style.setProperty('--primary', color);
}

export function useAccentColor() {
  const [accentColor, setAccentColor] = useState<string>('#007AFF');

  useEffect(() => {
    window.ipcRenderer
      .invoke('system:preferences:get-accent-color')
      .then((color: string) => {
        setAccentColor(color);
        updatePrimaryCssVariable(color);
      });

    const handleAccentColorChanged = (
      _event: Electron.IpcRendererEvent,
      color: string
    ) => {
      setAccentColor(color);
      updatePrimaryCssVariable(color);
    };

    window.ipcRenderer.on(
      'system:preferences:accent-color-changed',
      handleAccentColorChanged
    );

    return () => {
      window.ipcRenderer.off(
        'system:preferences:accent-color-changed',
        handleAccentColorChanged
      );
    };
  }, []);

  return accentColor;
}
