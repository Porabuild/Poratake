import { useRef, useEffect } from 'react';
import { Search, X } from 'lucide-react';
import { Input } from '@/renderer/components/ui/input';
import { Button } from '@/renderer/components/ui/button';
import { Separator } from '@/renderer/components/ui/separator';
import { cn } from '@/renderer/lib/utils';
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
    <div className="flex h-full w-52 shrink-0 flex-col border-r">
      <div className="p-3">
        <div className="relative">
          <Search className="text-muted-foreground absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" />
          <Input
            ref={inputRef}
            value={searchQuery}
            onChange={e => onSearchChange(e.target.value)}
            placeholder="Search settings..."
            className="h-8 pr-8 pl-8 text-sm"
          />
          {searchQuery && (
            <Button
              variant="ghost"
              size="icon"
              className="absolute top-1/2 right-0.5 size-7 -translate-y-1/2"
              onClick={() => onSearchChange('')}
            >
              <X className="size-3.5" />
            </Button>
          )}
        </div>
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
                'flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-sm transition-colors',
                isActive
                  ? 'bg-accent text-accent-foreground'
                  : 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'
              )}
            >
              <Icon className="size-4 shrink-0" />
              {category.label}
            </button>
          );
        })}
      </nav>

      <div className="pb-3">
        <Separator className="mb-2" />
        <div className="space-y-0.5 px-2">
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
                  'flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-sm transition-colors',
                  isActive
                    ? 'bg-accent text-accent-foreground'
                    : 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'
                )}
              >
                <Icon className="size-4 shrink-0" />
                {category.label}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
