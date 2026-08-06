import {
  Camera,
  Video,
  ArrowUpDown,
  LayoutGrid,
  LayoutList,
} from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import type {
  HistoryFilterType,
  HistorySortOrder,
  HistoryLayout,
} from '@/types/history';

interface HistoryToolbarProps {
  filter: HistoryFilterType;
  sortOrder: HistorySortOrder;
  layout: HistoryLayout;
  onFilterChange: (filter: HistoryFilterType) => void;
  onSortOrderChange: (order: HistorySortOrder) => void;
  onLayoutChange: (layout: HistoryLayout) => void;
}

const FILTER_OPTIONS: {
  value: HistoryFilterType;
  label: string;
  icon?: React.ReactNode;
}[] = [
  { value: 'all', label: 'All' },
  {
    value: 'screenshot',
    label: 'Screenshots',
    icon: <Camera className="h-3 w-3" />,
  },
  { value: 'video', label: 'Videos', icon: <Video className="h-3 w-3" /> },
];

export default function HistoryToolbar({
  filter,
  sortOrder,
  layout,
  onFilterChange,
  onSortOrderChange,
  onLayoutChange,
}: HistoryToolbarProps) {
  return (
    <div className="flex items-center justify-between gap-1 px-3 py-1.5">
      <div className="flex items-center gap-0.5">
        {FILTER_OPTIONS.map(option => (
          <Button
            key={option.value}
            variant={filter === option.value ? 'secondary' : 'ghost'}
            size="sm"
            onClick={() => onFilterChange(option.value)}
            className="h-6 px-2 text-xs"
          >
            {option.icon && <span className="mr-1">{option.icon}</span>}
            {option.label}
          </Button>
        ))}
      </div>
      <div className="flex items-center gap-0.5">
        <Button
          variant="ghost"
          size="icon"
          onClick={() =>
            onSortOrderChange(sortOrder === 'newest' ? 'oldest' : 'newest')
          }
          className="text-muted-foreground hover:text-foreground h-6 w-6"
          title={sortOrder === 'newest' ? 'Newest first' : 'Oldest first'}
        >
          <ArrowUpDown className="h-3.5 w-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          onClick={() => onLayoutChange(layout === 'grid' ? 'list' : 'grid')}
          className="text-muted-foreground hover:text-foreground h-6 w-6"
          title={layout === 'grid' ? 'Switch to list' : 'Switch to grid'}
        >
          {layout === 'grid' ? (
            <LayoutList className="h-3.5 w-3.5" />
          ) : (
            <LayoutGrid className="h-3.5 w-3.5" />
          )}
        </Button>
      </div>
    </div>
  );
}
