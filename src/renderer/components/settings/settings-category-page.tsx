import { useMemo, useState } from 'react';
import { Search, X } from 'lucide-react';
import SettingItemRenderer from './setting-item-renderer';
import { getSectionAction } from './section-actions';
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
        <h1 className="text-foreground text-lg font-semibold">
          {categoryInfo.label}
        </h1>

        {isShortcutsCategory && (
          <label className="border-border bg-background text-muted-foreground focus-within:border-ring focus-within:text-foreground flex h-8 w-64 items-center gap-2 rounded-lg border px-2.5 transition-colors">
            <Search className="size-3.5 shrink-0" />
            <input
              value={shortcutSearchQuery}
              onChange={event => setShortcutSearchQuery(event.target.value)}
              placeholder="Search shortcuts"
              aria-label="Search shortcuts"
              className="placeholder:text-muted-foreground text-foreground min-w-0 flex-1 bg-transparent text-sm outline-none"
            />
            {shortcutSearchQuery && (
              <button
                type="button"
                aria-label="Clear shortcut search"
                onClick={() => setShortcutSearchQuery('')}
                className="hover:text-foreground flex size-5 shrink-0 items-center justify-center rounded"
              >
                <X className="size-3.5" />
              </button>
            )}
          </label>
        )}
      </div>

      {isShortcutsCategory && visibleItems.length === 0 ? (
        <p className="text-muted-foreground py-12 text-center text-sm">
          No shortcuts found for &quot;{shortcutSearchQuery}&quot;
        </p>
      ) : (
        <div className={isShortcutsCategory ? 'space-y-4' : 'space-y-6'}>
          {Array.from(sections.entries()).map(([sectionName, sectionItems]) => {
            const sectionAction = getSectionAction(
              category,
              sectionName,
              settings
            );
            return (
              <section
                key={sectionName}
                className={isShortcutsCategory ? 'space-y-1' : 'space-y-4'}
              >
                {isShortcutsCategory && (
                  <h2 className="text-muted-foreground text-xs font-medium">
                    {sectionName}
                  </h2>
                )}
                {sectionAction && (
                  <div className="flex justify-end">{sectionAction}</div>
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
