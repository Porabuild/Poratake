import { useMemo } from 'react';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/renderer/components/ui/card';
import { Separator } from '@/renderer/components/ui/separator';
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
      <div className="flex flex-col items-center justify-center py-16">
        <p className="text-muted-foreground text-sm">
          No settings found for &quot;{query}&quot;
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <p className="text-muted-foreground text-sm">
        {results.length} {results.length === 1 ? 'result' : 'results'}
      </p>

      {SETTINGS_CATEGORIES.filter(c => grouped.has(c.id)).map(category => {
        const items = grouped.get(category.id)!;

        return (
          <Card key={category.id}>
            <CardHeader className="pb-3">
              <CardTitle className="flex items-center gap-2 text-sm">
                <category.icon className="size-4" />
                {category.label}
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-0 pb-4">
              {items.map((item, index) => (
                <div key={item.id}>
                  {index > 0 && <Separator className="my-1" />}
                  <SettingItemRenderer
                    item={item}
                    settings={settings}
                    onUpdate={onUpdate}
                  />
                </div>
              ))}
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}
