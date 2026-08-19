import { debugLog } from '@/main/utils/debug-log';

const SAMPLE_INTERVAL_MS = 100;
const REPORT_THRESHOLD_MS = 150;

export function monitorEventLoopLag(): void {
  let lastSample = performance.now();

  setInterval(() => {
    const now = performance.now();
    const lag = now - lastSample - SAMPLE_INTERVAL_MS;
    lastSample = now;

    if (lag > REPORT_THRESHOLD_MS) {
      debugLog('main-lag', `event loop blocked ${Math.round(lag)}ms`);
    }
  }, SAMPLE_INTERVAL_MS);
}
