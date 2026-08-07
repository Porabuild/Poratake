import { useEffect } from 'react';

export function useTransparentBody(enabled: boolean): void {
  useEffect(() => {
    if (!enabled) return;

    const previous = document.body.style.backgroundColor;
    document.body.style.backgroundColor = 'transparent';

    return () => {
      document.body.style.backgroundColor = previous;
    };
  }, [enabled]);
}
