import type { ReactNode } from 'react';
import { Button } from '@/renderer/components/ui/button';
import { Label } from '@/renderer/components/ui/label';
import { Edit3, FileUp } from 'lucide-react';

interface DataEditorSectionProps {
  label: string;
  onEdit: () => void;
  onImport: () => void;
  isImporting: boolean;
  editLabel?: string;
  importLabel?: string;
  children?: ReactNode;
}

export default function DataEditorSection({
  label,
  onEdit,
  onImport,
  isImporting,
  editLabel = 'Edit',
  importLabel = 'Import',
  children,
}: DataEditorSectionProps) {
  return (
    <div className="space-y-2 border-t border-border pt-4">
      <Label className="text-sm">{label}</Label>
      <div className="flex gap-2">
        <Button
          variant="tertiary"
          size="xs"
          onClick={onEdit}
          className="flex-1 gap-1"
        >
          <Edit3 className="size-3" />
          {editLabel}
        </Button>
        <Button
          variant="tertiary"
          size="xs"
          onClick={onImport}
          disabled={isImporting}
          className="flex-1 gap-1"
        >
          <FileUp className="size-3" />
          {isImporting ? 'Importing...' : importLabel}
        </Button>
      </div>
      {children && (
        <div className="text-xs text-muted-foreground">{children}</div>
      )}
    </div>
  );
}
