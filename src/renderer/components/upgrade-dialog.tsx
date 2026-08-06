import { useCallback } from 'react';
import { ExternalLink, Key, Sparkles } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/renderer/components/ui/dialog';
import { Button } from '@/renderer/components/ui/button';
import { PRO_FEATURES } from '@/types/entitlements';

interface UpgradeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  reason?: string;
}

export default function UpgradeDialog({
  open,
  onOpenChange,
  reason,
}: UpgradeDialogProps) {
  const handleUpgrade = useCallback(async () => {
    const url = (await window.ipcRenderer.invoke(
      'license:getCheckoutUrl'
    )) as string;
    window.ipcRenderer.send('open-external', url);
  }, []);

  const handleEnterKey = useCallback(() => {
    window.ipcRenderer.send('license:open-activation');
    onOpenChange(false);
  }, [onOpenChange]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <div className="bg-primary/10 mb-1 flex h-12 w-12 items-center justify-center rounded-full">
            <Sparkles className="text-primary h-6 w-6" />
          </div>
          <DialogTitle>Upgrade to Capty Pro</DialogTitle>
          <DialogDescription>
            {reason ??
              'Unlock the full power of Capty with a one-time Pro license.'}
          </DialogDescription>
        </DialogHeader>

        <ul className="space-y-2">
          {PRO_FEATURES.map(feature => (
            <li
              key={feature}
              className="flex items-center gap-2 text-sm font-medium"
            >
              <Sparkles className="text-primary h-3.5 w-3.5 shrink-0" />
              {feature}
            </li>
          ))}
        </ul>

        <DialogFooter>
          <Button variant="outline" onClick={handleEnterKey}>
            <Key className="mr-2 h-4 w-4" />
            Enter License Key
          </Button>
          <Button onClick={handleUpgrade}>
            <ExternalLink className="mr-2 h-4 w-4" />
            Upgrade to Pro
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
