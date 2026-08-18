import { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { Trash2, Settings, ImageOff } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import HistoryItem from '@/renderer/components/history/history-item';
import HistoryListItem from '@/renderer/components/history/history-list-item';
import HistoryToolbar from '@/renderer/components/history/history-toolbar';
import type {
  HistoryItemSummary,
  HistoryFilterType,
  HistorySortOrder,
  HistoryLayout,
} from '@/types/history';
import type { SettingsUiConfig } from '@/types/settings';
import { shouldIgnoreGlobalKeyboardShortcuts } from '@/renderer/utils/keyboard';
import { useAppTheme } from '@/renderer/hooks/use-app-theme';

const GRID_COLUMNS = 2;

export default function HistoryWindow() {
  useAppTheme();

  const [items, setItems] = useState<HistoryItemSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [isKeyboardNavigationActive, setIsKeyboardNavigationActive] =
    useState(false);
  const [filter, setFilter] = useState<HistoryFilterType>('all');
  const [sortOrder, setSortOrder] = useState<HistorySortOrder>('newest');
  const [layout, setLayout] = useState<HistoryLayout>('grid');
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<Map<string, HTMLDivElement>>(new Map());

  const filteredItems = useMemo(() => {
    let result = items;

    if (filter !== 'all') {
      result = result.filter(item => item.type === filter);
    }

    if (sortOrder === 'oldest') {
      result = result.toReversed();
    }

    return result;
  }, [items, filter, sortOrder]);

  const persistPreferences = useCallback(
    (updates: {
      filter?: HistoryFilterType;
      sortOrder?: HistorySortOrder;
      layout?: HistoryLayout;
    }) => {
      window.ipcRenderer.invoke('settings:update', {
        history: {
          filter: updates.filter ?? filter,
          sortOrder: updates.sortOrder ?? sortOrder,
          layout: updates.layout ?? layout,
        },
      });
    },
    [filter, sortOrder, layout]
  );

  const handleFilterChange = useCallback(
    (value: HistoryFilterType) => {
      setFilter(value);
      persistPreferences({ filter: value });
    },
    [persistPreferences]
  );

  const handleSortOrderChange = useCallback(
    (value: HistorySortOrder) => {
      setSortOrder(value);
      persistPreferences({ sortOrder: value });
    },
    [persistPreferences]
  );

  const handleLayoutChange = useCallback(
    (value: HistoryLayout) => {
      setLayout(value);
      persistPreferences({ layout: value });
    },
    [persistPreferences]
  );

  const loadHistory = useCallback(async () => {
    try {
      const [history, settings] = (await Promise.all([
        window.ipcRenderer.invoke('history:get'),
        window.ipcRenderer.invoke('settings:get-ui'),
      ])) as [HistoryItemSummary[], SettingsUiConfig];

      setItems(history);

      if (settings.history) {
        if (settings.history.filter) setFilter(settings.history.filter);
        if (settings.history.sortOrder)
          setSortOrder(settings.history.sortOrder);
        if (settings.history.layout) setLayout(settings.history.layout);
      }
    } catch (error) {
      console.error('Failed to load history:', error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    const handleRefresh = () => {
      scrollContainerRef.current?.scrollTo(0, 0);
      setSelectedIndex(0);
      loadHistory();
    };
    window.ipcRenderer.on('history:refresh', handleRefresh);
    window.ipcRenderer.send('history:ready');

    return () => {
      window.ipcRenderer.off('history:refresh', handleRefresh);
    };
  }, [loadHistory]);

  useEffect(() => {
    setSelectedIndex(0);
    scrollContainerRef.current?.scrollTo(0, 0);
  }, [filter, sortOrder]);

  const handleDelete = useCallback(async (id: string) => {
    try {
      await window.ipcRenderer.invoke('history:delete', id);
      setItems(prev => {
        const newItems = prev.filter(item => item.id !== id);
        setSelectedIndex(currentIndex => {
          if (newItems.length === 0) return 0;
          if (currentIndex >= newItems.length) return newItems.length - 1;
          return currentIndex;
        });
        return newItems;
      });
    } catch (error) {
      console.error('Failed to delete history item:', error);
    }
  }, []);

  const handleClearAll = useCallback(async () => {
    try {
      const cleared = (await window.ipcRenderer.invoke(
        'history:clear'
      )) as boolean;
      if (!cleared) return;

      setItems([]);
    } catch (error) {
      console.error('Failed to clear history:', error);
    }
  }, []);

  const handleOpenItem = useCallback((item: HistoryItemSummary) => {
    if (item.type === 'video') {
      window.ipcRenderer.send('history:openVideo', item.id);
    } else {
      window.ipcRenderer.send('history:openScreenshot', item.id);
    }
    window.ipcRenderer.send('history:closePopover');
  }, []);

  const handleOpenSettings = useCallback(() => {
    window.ipcRenderer.send('open-settings');
    window.ipcRenderer.send('history:closePopover');
  }, []);

  const scrollToSelected = useCallback(
    (index: number) => {
      const item = filteredItems[index];
      if (!item) return;

      const element = itemRefs.current.get(item.id);
      if (element && scrollContainerRef.current) {
        element.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      }
    },
    [filteredItems]
  );

  useEffect(() => {
    if (filteredItems.length === 0) return;

    const columns = layout === 'grid' ? GRID_COLUMNS : 1;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (shouldIgnoreGlobalKeyboardShortcuts(e.target)) {
        return;
      }

      const key = e.key.toLowerCase();

      if (
        [
          'arrowup',
          'arrowdown',
          'arrowleft',
          'arrowright',
          'h',
          'j',
          'k',
          'l',
        ].includes(key)
      ) {
        e.preventDefault();
        setIsKeyboardNavigationActive(true);
        setSelectedIndex(prev => {
          let newIndex = prev;

          switch (key) {
            case 'arrowup':
            case 'k':
              newIndex = Math.max(0, prev - columns);
              break;
            case 'arrowdown':
            case 'j':
              newIndex = Math.min(filteredItems.length - 1, prev + columns);
              break;
            case 'arrowleft':
            case 'h':
              newIndex = Math.max(0, prev - 1);
              break;
            case 'arrowright':
            case 'l':
              newIndex = Math.min(filteredItems.length - 1, prev + 1);
              break;
          }

          if (newIndex !== prev) {
            setTimeout(() => scrollToSelected(newIndex), 0);
          }
          return newIndex;
        });
        return;
      }

      if (key === 'enter') {
        e.preventDefault();
        const selectedItem = filteredItems[selectedIndex];
        if (selectedItem) {
          handleOpenItem(selectedItem);
        }
        return;
      }

      if (key === 'backspace' || key === 'd') {
        e.preventDefault();
        const selectedItem = filteredItems[selectedIndex];
        if (selectedItem) {
          handleDelete(selectedItem.id);
        }
        return;
      }

      if (key === 'escape' || key === 'q') {
        e.preventDefault();
        window.ipcRenderer.send('history:closePopover');
        return;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [
    filteredItems,
    selectedIndex,
    scrollToSelected,
    handleDelete,
    handleOpenItem,
    layout,
  ]);

  const ItemComponent = layout === 'grid' ? HistoryItem : HistoryListItem;

  const hasItems = items.length > 0;
  const hasFilteredItems = filteredItems.length > 0;

  return (
    <div className="flex h-screen flex-col overflow-hidden rounded-xl">
      <div className="flex items-center justify-between border-b border-border px-4 py-3">
        <h1 className="text-sm font-medium text-foreground">History</h1>
        <div className="flex items-center gap-1">
          {hasItems && (
            <Button
              variant="ghost"
              size="sm"
              onClick={handleClearAll}
              className="h-7 px-2 text-xs text-muted-foreground hover:text-foreground"
            >
              <Trash2 className="mr-1 h-3 w-3" />
              Clear All
            </Button>
          )}
          <Button
            variant="ghost"
            size="icon"
            onClick={handleOpenSettings}
            className="h-7 w-7 text-muted-foreground hover:text-foreground"
          >
            <Settings className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {hasItems && (
        <HistoryToolbar
          filter={filter}
          sortOrder={sortOrder}
          layout={layout}
          onFilterChange={handleFilterChange}
          onSortOrderChange={handleSortOrderChange}
          onLayoutChange={handleLayoutChange}
        />
      )}

      <div ref={scrollContainerRef} className="flex-1 overflow-y-auto p-3">
        {loading ? (
          <div className="flex h-full items-center justify-center">
            <div className="text-sm text-muted-foreground">Loading...</div>
          </div>
        ) : !hasItems ? (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
            <ImageOff className="h-10 w-10" />
            <p className="text-sm">No captures yet</p>
            <p className="text-xs">Take a screenshot or record a video</p>
          </div>
        ) : !hasFilteredItems ? (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
            <ImageOff className="h-8 w-8" />
            <p className="text-sm">
              No {filter === 'screenshot' ? 'screenshots' : 'videos'} found
            </p>
          </div>
        ) : (
          <div
            className={
              layout === 'grid'
                ? 'grid grid-cols-2 gap-3'
                : 'flex flex-col gap-2'
            }
          >
            {filteredItems.map((item, index) => (
              <ItemComponent
                key={item.id}
                ref={el => {
                  if (el) {
                    itemRefs.current.set(item.id, el);
                  } else {
                    itemRefs.current.delete(item.id);
                  }
                }}
                item={item}
                isSelected={
                  isKeyboardNavigationActive && index === selectedIndex
                }
                onOpen={handleOpenItem}
                onDelete={handleDelete}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
