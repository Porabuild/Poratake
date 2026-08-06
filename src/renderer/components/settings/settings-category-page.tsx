import { useMemo } from 'react';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/renderer/components/ui/card';
import { Separator } from '@/renderer/components/ui/separator';
import SettingItemRenderer from './setting-item-renderer';
import { getSectionAction } from './section-actions';
import {
  getItemsByCategory,
  groupBySection,
  getSectionDescription,
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
  const categoryInfo = SETTINGS_CATEGORIES.find(c => c.id === category);
  const items = useMemo(() => getItemsByCategory(category), [category]);
  const visibleItems = useMemo(
    () =>
      items.filter(item => {
        if ('visibleWhen' in item && item.visibleWhen) {
          return item.visibleWhen(settings);
        }
        return true;
      }),
    [items, settings]
  );
  const sections = useMemo(() => groupBySection(visibleItems), [visibleItems]);

  if (!categoryInfo) return null;

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-medium">{categoryInfo.label}</h2>
        <p className="text-muted-foreground text-sm">
          {categoryInfo.description}
        </p>
      </div>

      {Array.from(sections.entries()).map(([sectionName, sectionItems]) => {
        const sectionDesc = getSectionDescription(category, sectionName);
        const sectionAction = getSectionAction(category, sectionName, settings);
        return (
          <Card key={sectionName}>
            <CardHeader className="flex flex-row items-start justify-between gap-4 pb-3">
              <div className="space-y-1.5">
                <CardTitle>{sectionName}</CardTitle>
                {sectionDesc && (
                  <CardDescription>{sectionDesc}</CardDescription>
                )}
              </div>
              {sectionAction && <div className="shrink-0">{sectionAction}</div>}
            </CardHeader>
            <CardContent className="space-y-0 pb-4">
              {sectionItems.map((item, index) => (
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
