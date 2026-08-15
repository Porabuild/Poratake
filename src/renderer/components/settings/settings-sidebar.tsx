import { useRef, useEffect } from 'react';
import { Search, X } from 'lucide-react';
import { Separator } from '@/renderer/components/ui/separator';
import { cn } from '@/renderer/lib/utils';
import { isMacPlatform } from '@/renderer/utils/platform';
import { SEARCHABLE_CATEGORIES, SPECIAL_CATEGORIES } from './settings-registry';

interface SettingsSidebarProps {
  activeCategory: string;
  searchQuery: string;
  onCategoryChange: (category: string) => void;
  onSearchChange: (query: string) => void;
}

export default function SettingsSidebar({
  activeCategory,
  searchQuery,
  onCategoryChange,
  onSearchChange,
}: SettingsSidebarProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  return (
    <aside className="poratake-settings-sidebar flex h-full w-60 shrink-0 flex-col">
      <div
        className={cn(
          'flex h-10 shrink-0 items-center px-4',
          isMacPlatform() && 'pl-20'
        )}
        style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
      >
        <p className="text-muted-foreground text-xs font-semibold tracking-[0.12em]">
          SETTINGS
        </p>
      </div>

      <div className="px-2 pb-2">
        <label className="text-muted-foreground focus-within:text-foreground hover:text-foreground flex cursor-text items-center gap-2 rounded-3xl px-2 py-1.5 transition-colors focus-within:bg-[var(--row-active)] hover:bg-[var(--row-hover)]">
          <Search className="size-4 shrink-0" />
          <input
            ref={inputRef}
            value={searchQuery}
            onChange={e => onSearchChange(e.target.value)}
            placeholder="Search settings"
            className="placeholder:text-muted-foreground text-foreground min-w-0 flex-1 bg-transparent text-sm outline-none"
          />
          {searchQuery && (
            <button
              type="button"
              aria-label="Clear settings search"
              className="text-muted-foreground hover:text-foreground flex size-5 shrink-0 items-center justify-center rounded"
              onClick={() => onSearchChange('')}
            >
              <X className="size-3.5" />
            </button>
          )}
        </label>
      </div>

      <nav className="flex-1 space-y-0.5 overflow-auto px-2">
        {SEARCHABLE_CATEGORIES.map(category => {
          const Icon = category.icon;
          const isActive = !searchQuery && activeCategory === category.id;

          return (
            <button
              key={category.id}
              onClick={() => {
                onSearchChange('');
                onCategoryChange(category.id);
              }}
              className={cn(
                'flex w-full items-center gap-2.5 rounded-3xl px-2.5 py-1.5 text-sm transition-colors',
                isActive
                  ? 'text-foreground bg-[var(--row-active)]'
                  : 'text-muted-foreground hover:text-foreground hover:bg-[var(--row-hover)]'
              )}
            >
              <Icon className="size-4 shrink-0" />
              {category.label}
            </button>
          );
        })}
      </nav>

      <div className="pb-2">
        <Separator className="mb-2" />
        <div className="space-y-1 px-2">
          {SPECIAL_CATEGORIES.map(category => {
            const Icon = category.icon;
            const isActive = !searchQuery && activeCategory === category.id;

            return (
              <button
                key={category.id}
                onClick={() => {
                  onSearchChange('');
                  onCategoryChange(category.id);
                }}
                className={cn(
                  'flex w-full items-center gap-2.5 rounded-3xl px-2.5 py-1.5 text-sm transition-colors',
                  isActive
                    ? 'text-foreground bg-[var(--row-active)]'
                    : 'text-muted-foreground hover:text-foreground hover:bg-[var(--row-hover)]'
                )}
              >
                <Icon className="size-4 shrink-0" />
                {category.label}
              </button>
            );
          })}
        </div>
      </div>
    </aside>
  );
}
