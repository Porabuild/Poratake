import { useMemo, useState } from 'react';
import { Search, X } from 'lucide-react';
import SettingItemRenderer from './setting-item-renderer';
import {
  getItemsByCategory,
  groupBySection,
  matchesSettingsQuery,
  SETTINGS_CATEGORIES,
} from './settings-registry';
import type { SettingsConfig } from '@/types/settings';

interface SettingsCategoryPageProps {
  category: string;
  settings: SettingsConfig;
  onUpdate: (updates: Partial<SettingsConfig>) => void;
}

export default function SettingsCategoryPage({
  category,
  settings,
  onUpdate,
}: SettingsCategoryPageProps) {
  const [shortcutSearchQuery, setShortcutSearchQuery] = useState('');
  const categoryInfo = SETTINGS_CATEGORIES.find(c => c.id === category);
  const items = useMemo(() => getItemsByCategory(category), [category]);
  const isShortcutsCategory = category === 'shortcuts';
  const visibleItems = useMemo(
    () =>
      items.filter(item => {
        if ('visibleWhen' in item && item.visibleWhen) {
          if (!item.visibleWhen(settings)) return false;
        }

        return (
          !isShortcutsCategory ||
          matchesSettingsQuery(item, shortcutSearchQuery)
        );
      }),
    [isShortcutsCategory, items, settings, shortcutSearchQuery]
  );
  const sections = useMemo(() => groupBySection(visibleItems), [visibleItems]);

  if (!categoryInfo) return null;

  return (
    <div className="mx-auto min-h-full max-w-[720px]">
      <div className="mb-4 flex items-center justify-between gap-4">
        <h1 className="text-lg font-semibold text-foreground">
          {categoryInfo.label}
        </h1>

        {isShortcutsCategory && (
          <label className="flex h-8 w-64 items-center gap-2 rounded-field border-0 bg-field px-2.5 text-muted-foreground transition-colors focus-within:text-foreground">
            <Search className="size-3.5 shrink-0" />
            <input
              value={shortcutSearchQuery}
              onChange={event => setShortcutSearchQuery(event.target.value)}
              placeholder="Search shortcuts"
              aria-label="Search shortcuts"
              className="min-w-0 flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
            />
            {shortcutSearchQuery && (
              <button
                type="button"
                aria-label="Clear shortcut search"
                onClick={() => setShortcutSearchQuery('')}
                className="flex size-5 shrink-0 items-center justify-center rounded hover:text-foreground"
              >
                <X className="size-3.5" />
              </button>
            )}
          </label>
        )}
      </div>

      {isShortcutsCategory && visibleItems.length === 0 ? (
        <p className="py-12 text-center text-sm text-muted-foreground">
          No shortcuts found for &quot;{shortcutSearchQuery}&quot;
        </p>
      ) : (
        <div className={isShortcutsCategory ? 'space-y-4' : 'space-y-6'}>
          {Array.from(sections.entries()).map(([sectionName, sectionItems]) => {
            return (
              <section
                key={sectionName}
                className={isShortcutsCategory ? 'space-y-1' : 'space-y-4'}
              >
                {isShortcutsCategory && (
                  <h2 className="text-xs font-medium text-muted-foreground">
                    {sectionName}
                  </h2>
                )}
                {sectionItems.map(item => (
                  <SettingItemRenderer
                    key={item.id}
                    item={item}
                    settings={settings}
                    onUpdate={onUpdate}
                    compact={isShortcutsCategory}
                  />
                ))}
              </section>
            );
          })}
        </div>
      )}
    </div>
  );
}
