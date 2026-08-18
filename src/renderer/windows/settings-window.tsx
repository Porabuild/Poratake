import { useState, useEffect, useCallback, useRef } from 'react';
import AboutTab from '@/renderer/components/settings/about-tab';
import SettingsSidebar from '@/renderer/components/settings/settings-sidebar';
import SettingsCategoryPage from '@/renderer/components/settings/settings-category-page';
import SettingsSearchResults from '@/renderer/components/settings/settings-search-results';
import type { SettingsConfig, SettingsUiConfig } from '@/types/settings';
import { DEFAULT_SETTINGS } from '@/types/settings';
import { SETTINGS_CATEGORIES } from '@/renderer/components/settings/settings-registry';

const ALL_TABS = new Set(SETTINGS_CATEGORIES.map(c => c.id));
const DEFAULT_TAB = 'general';

function getTabFromHash(): string {
  const hash = window.location.hash.slice(1);
  return ALL_TABS.has(hash) ? hash : DEFAULT_TAB;
}

export default function SettingsWindow() {
  const [settings, setSettings] = useState<SettingsConfig>(DEFAULT_SETTINGS);
  const [activeTab, setActiveTab] = useState(getTabFromHash);
  const [searchQuery, setSearchQuery] = useState('');
  const [isLoading, setIsLoading] = useState(true);
  const updateSequence = useRef(0);

  useEffect(() => {
    const loadInitialData = async () => {
      const loadedSettings = (await window.ipcRenderer.invoke(
        'settings:get-ui'
      )) as SettingsUiConfig;
      setSettings({ ...DEFAULT_SETTINGS, ...loadedSettings });
      setIsLoading(false);
    };
    loadInitialData();
  }, []);

  useEffect(() => {
    const handleHashChange = () => {
      setActiveTab(getTabFromHash());
    };
    window.addEventListener('hashchange', handleHashChange);
    return () => window.removeEventListener('hashchange', handleHashChange);
  }, []);

  useEffect(() => {
    const handleNavigateTab = (_event: unknown, tab: string) => {
      if (ALL_TABS.has(tab)) {
        setActiveTab(tab);
        setSearchQuery('');
        window.location.hash = tab;
      }
    };
    window.ipcRenderer.on('navigate-tab', handleNavigateTab);
    return () => {
      window.ipcRenderer.off('navigate-tab', handleNavigateTab);
    };
  }, []);

  const handleTabChange = useCallback((value: string) => {
    setActiveTab(value);
    window.location.hash = value;
  }, []);

  const handleUpdate = useCallback(async (updates: Partial<SettingsConfig>) => {
    const sequence = ++updateSequence.current;
    const updatedSettings = (await window.ipcRenderer.invoke(
      'settings:update',
      updates
    )) as SettingsUiConfig;
    if (sequence === updateSequence.current) {
      setSettings({ ...DEFAULT_SETTINGS, ...updatedSettings });
    }

    if (updates.shortcuts) {
      window.ipcRenderer.send('shortcuts:reload');
    }

    if (updates.preview) {
      window.ipcRenderer.send('capture-preview:reposition');
    }
  }, []);

  if (isLoading) {
    return (
      <div className="flex h-screen w-full items-center justify-center bg-background">
        <p className="text-muted-foreground">Loading...</p>
      </div>
    );
  }

  const isSearching = searchQuery.trim().length > 0;

  const renderContent = () => {
    if (isSearching) {
      return (
        <SettingsSearchResults
          query={searchQuery}
          settings={settings}
          onUpdate={handleUpdate}
        />
      );
    }

    switch (activeTab) {
      case 'about':
        return <AboutTab />;
      default:
        return (
          <SettingsCategoryPage
            category={activeTab}
            settings={settings}
            onUpdate={handleUpdate}
          />
        );
    }
  };

  return (
    <div className="poratake-settings-shell flex h-screen w-full bg-content">
      <SettingsSidebar
        activeCategory={activeTab}
        searchQuery={searchQuery}
        onCategoryChange={handleTabChange}
        onSearchChange={setSearchQuery}
      />

      <div className="poratake-settings-content flex min-w-0 flex-1 flex-col bg-content">
        <div
          className="h-10 w-full shrink-0"
          style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
        />
        <main className="min-h-0 flex-1 overflow-auto px-6 pt-3 pb-8">
          {renderContent()}
        </main>
      </div>
    </div>
  );
}
