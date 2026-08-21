import { registerHistoryIpc } from './ipc.ts';
import { loadHistory } from './store.ts';

export {
  closeHistoryPopover,
  getHistoryPopover,
  isHistoryPopoverVisible,
  isHistoryPopoverWebContents,
  preloadHistoryPopover,
  showHistoryPopover,
  toggleHistoryPopover,
} from './popover';
export { getVideoRecordingFeatures } from './media.ts';
export {
  addToHistory,
  clearHistory,
  deleteHistoryItem,
  getHistory,
  getHistoryItem,
  getHistoryItemByPath,
  getHistorySummaries,
  loadHistory,
  setHistoryFileReleaseHandler,
  updateHistoryItem,
  updateHistoryItemByPath,
  updateHistoryItemPath,
} from './store.ts';

export async function init(): Promise<void> {
  await loadHistory();
  registerHistoryIpc();
}
