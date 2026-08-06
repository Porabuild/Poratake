import { useCallback, useEffect, useState } from 'react';

export function usePro() {
  const [isPro, setIsPro] = useState(true);
  const [upgradeOpen, setUpgradeOpen] = useState(false);

  const refresh = useCallback(async () => {
    const pro = (await window.ipcRenderer.invoke('license:isPro')) as boolean;
    setIsPro(pro);
  }, []);

  useEffect(() => {
    refresh();
    const handler = () => refresh();
    window.ipcRenderer.on('license:changed', handler);
    return () => {
      window.ipcRenderer.off('license:changed', handler);
    };
  }, [refresh]);

  const requirePro = useCallback(() => {
    if (isPro) {
      return true;
    }
    setUpgradeOpen(true);
    return false;
  }, [isPro]);

  return { isPro, requirePro, upgradeOpen, setUpgradeOpen };
}
