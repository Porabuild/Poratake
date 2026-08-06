import { useState, useEffect, useCallback } from 'react';
import LicenseTab from '@/renderer/components/settings/license-tab';
import AboutTab from '@/renderer/components/settings/about-tab';
import SettingsSidebar from '@/renderer/components/settings/settings-sidebar';
import SettingsCategoryPage from '@/renderer/components/settings/settings-category-page';
import SettingsSearchResults from '@/renderer/components/settings/settings-search-results';
import type { SettingsConfig } from '@/types/settings';
import { DEFAULT_SETTINGS } from '@/types/settings';
import { SETTINGS_CATEGORIES } from '@/renderer/components/settings/settings-registry';

const ALL_TABS = SETTINGS_CATEGORIES.map(c => c.id);
const DEFAULT_TAB = 'general';

export default function SettingsWindow() {
  const [settings, setSettings] = useState<SettingsConfig>(DEFAULT_SETTINGS);
  const [activeTab, setActiveTab] = useState(DEFAULT_TAB);
  const [searchQuery, setSearchQuery] = useState('');
  const [isLoading, setIsLoading] = useState(true);

  const getTabFromHash = useCallback(() => {
    const hash = window.location.hash.slice(1);
    return ALL_TABS.includes(hash) ? hash : DEFAULT_TAB;
  }, []);

  useEffect(() => {
    const loadInitialData = async () => {
      const loadedSettings = await window.ipcRenderer.invoke('settings:get');
      setSettings(loadedSettings);
      setIsLoading(false);
    };
    loadInitialData();
  }, []);

  useEffect(() => {
    if (!isLoading) {
      setActiveTab(getTabFromHash());
    }
  }, [isLoading, getTabFromHash]);

  useEffect(() => {
    const handleHashChange = () => {
      setActiveTab(getTabFromHash());
    };
    window.addEventListener('hashchange', handleHashChange);
    return () => window.removeEventListener('hashchange', handleHashChange);
  }, [getTabFromHash]);

  useEffect(() => {
    const handleNavigateTab = (_event: unknown, tab: string) => {
      if (ALL_TABS.includes(tab)) {
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
    const updatedSettings = await window.ipcRenderer.invoke(
      'settings:update',
      updates
    );
    setSettings(updatedSettings);

    if (updates.shortcuts) {
      window.ipcRenderer.send('shortcuts:reload');
    }
  }, []);

  if (isLoading) {
    return (
      <div className="bg-background flex h-screen w-full items-center justify-center">
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
      case 'license':
        return <LicenseTab />;
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
    <div className="bg-background flex h-screen w-full flex-col">
      <div
        className="flex h-10 w-full shrink-0 items-center justify-center border-b"
        style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
      >
        <span className="text-muted-foreground text-xs font-medium">
          Settings
        </span>
      </div>

      <div className="flex min-h-0 flex-1">
        <SettingsSidebar
          activeCategory={activeTab}
          searchQuery={searchQuery}
          onCategoryChange={handleTabChange}
          onSearchChange={setSearchQuery}
        />

        <div className="flex-1 overflow-auto px-6 py-4">{renderContent()}</div>
      </div>
    </div>
  );
}
