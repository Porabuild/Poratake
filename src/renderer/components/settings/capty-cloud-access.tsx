import { useCallback, useEffect, useState } from 'react';
import { CheckCircle, Cloud, ExternalLink, KeyRound } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';

export default function CaptyCloudAccess() {
  const [hasAccess, setHasAccess] = useState<boolean | null>(null);

  const refreshAccess = useCallback(async () => {
    const access = (await window.ipcRenderer.invoke(
      'cloud:has-hosted-access'
    )) as boolean;
    setHasAccess(access);
  }, []);

  useEffect(() => {
    refreshAccess();
    window.ipcRenderer.on('license:changed', refreshAccess);
    return () => {
      window.ipcRenderer.off('license:changed', refreshAccess);
    };
  }, [refreshAccess]);

  const handleGetLicense = useCallback(async () => {
    const url = (await window.ipcRenderer.invoke(
      'license:getCheckoutUrl'
    )) as string;
    window.ipcRenderer.send('open-external', url);
  }, []);

  const handleEnterLicense = useCallback(() => {
    window.ipcRenderer.send('license:open-activation');
  }, []);

  if (hasAccess === null) {
    return (
      <p className="text-muted-foreground py-2 text-sm">
        Checking license access...
      </p>
    );
  }

  if (hasAccess) {
    return (
      <div className="bg-primary/5 flex items-start gap-3 rounded-md p-4">
        <CheckCircle className="text-primary mt-0.5 size-5 shrink-0" />
        <div className="space-y-1">
          <p className="text-sm font-medium">Ready to upload</p>
          <p className="text-muted-foreground text-xs">
            Your active license securely authenticates Capty Cloud. No storage
            credentials are required.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-muted/50 space-y-3 rounded-md p-4">
      <div className="flex items-start gap-3">
        <Cloud className="text-muted-foreground mt-0.5 size-5 shrink-0" />
        <div className="space-y-1">
          <p className="text-sm font-medium">Activate Capty Cloud</p>
          <p className="text-muted-foreground text-xs">
            Get a license or enter an existing key to use hosted uploads. You
            can also select Self-hosted cloud or S3-compatible storage without a
            license.
          </p>
        </div>
      </div>
      <div className="flex flex-wrap gap-2">
        <Button size="sm" onClick={handleGetLicense}>
          <ExternalLink className="mr-2 size-4" />
          Get a license
        </Button>
        <Button size="sm" variant="outline" onClick={handleEnterLicense}>
          <KeyRound className="mr-2 size-4" />
          Enter license key
        </Button>
      </div>
    </div>
  );
}
