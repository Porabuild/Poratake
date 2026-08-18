import { useMemo } from 'react';
import SettingItemRenderer from './setting-item-renderer';
import {
  type SettingsItem,
  SETTINGS_CATEGORIES,
  searchSettings,
} from './settings-registry';
import type { SettingsConfig } from '@/types/settings';

interface SettingsSearchResultsProps {
  query: string;
  settings: SettingsConfig;
  onUpdate: (updates: Partial<SettingsConfig>) => void;
}

export default function SettingsSearchResults({
  query,
  settings,
  onUpdate,
}: SettingsSearchResultsProps) {
  const results = useMemo(() => searchSettings(query), [query]);

  const grouped = useMemo(() => {
    const map = new Map<string, SettingsItem[]>();
    for (const item of results) {
      const existing = map.get(item.category) ?? [];
      existing.push(item);
      map.set(item.category, existing);
    }
    return map;
  }, [results]);

  if (!query.trim()) return null;

  if (results.length === 0) {
    return (
      <div className="mx-auto flex max-w-[720px] flex-col items-center justify-center py-16">
        <p className="text-sm text-muted-foreground">
          No settings found for &quot;{query}&quot;
        </p>
      </div>
    );
  }

  return (
    <div className="mx-auto min-h-full max-w-[720px]">
      <h1 className="mb-2 text-lg font-semibold text-foreground">
        Search results
      </h1>
      <p className="mb-6 text-xs text-muted-foreground">
        {results.length} {results.length === 1 ? 'result' : 'results'}
      </p>

      <div className="space-y-7">
        {SETTINGS_CATEGORIES.filter(c => grouped.has(c.id)).map(category => {
          const items = grouped.get(category.id)!;

          return (
            <section key={category.id} className="space-y-4">
              <h2 className="flex items-center gap-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                <category.icon className="size-4" />
                {category.label}
              </h2>
              {items.map(item => (
                <SettingItemRenderer
                  key={item.id}
                  item={item}
                  settings={settings}
                  onUpdate={onUpdate}
                />
              ))}
            </section>
          );
        })}
      </div>
    </div>
  );
}
