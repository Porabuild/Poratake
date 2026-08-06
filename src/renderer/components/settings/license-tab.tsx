import { useEffect, useState, useCallback } from 'react';
import {
  Calendar,
  AlertTriangle,
  Trash2,
  Key,
  ExternalLink,
  Sparkles,
} from 'lucide-react';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/renderer/components/ui/card';
import { Button } from '@/renderer/components/ui/button';
import { Separator } from '@/renderer/components/ui/separator';
import type { LicenseStatus } from '@/types/license';
import { PRO_FEATURES } from '@/types/entitlements';
import appIcon from '@build/icon.png';

interface LicenseInfo {
  email: string;
  expiresAt: string | null;
  isLifetime: boolean;
}

interface LicenseState {
  status: LicenseStatus;
  info: LicenseInfo | null;
}

export default function LicenseTab() {
  const [license, setLicense] = useState<LicenseState | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const [showConfirmation, setShowConfirmation] = useState(false);

  const loadData = useCallback(async () => {
    const licenseData = (await window.ipcRenderer.invoke(
      'license:getStatus'
    )) as LicenseState;
    setLicense(licenseData);
  }, []);

  useEffect(() => {
    loadData();
    const handler = () => loadData();
    window.ipcRenderer.on('license:changed', handler);
    return () => {
      window.ipcRenderer.off('license:changed', handler);
    };
  }, [loadData]);

  const handleActivateLicense = useCallback(() => {
    window.ipcRenderer.send('license:open-activation');
  }, []);

  const handlePurchaseLicense = useCallback(async () => {
    const url = (await window.ipcRenderer.invoke(
      'license:getCheckoutUrl'
    )) as string;
    window.ipcRenderer.send('open-external', url);
  }, []);

  const handleDeleteClick = useCallback(() => {
    setShowConfirmation(true);
  }, []);

  const handleCancelDelete = useCallback(() => {
    setShowConfirmation(false);
  }, []);

  const handleConfirmDelete = useCallback(async () => {
    setIsDeleting(true);
    try {
      await window.ipcRenderer.invoke('license:deactivate');
      window.ipcRenderer.send('license:deleted');
      setShowConfirmation(false);
      await loadData();
    } catch (error) {
      console.error('Failed to delete license:', error);
    } finally {
      setIsDeleting(false);
    }
  }, [loadData]);

  const formatDate = (dateString: string) => {
    const date = new Date(dateString);
    return date.toLocaleDateString('en-US', {
      year: 'numeric',
      month: 'long',
      day: 'numeric',
    });
  };

  const getStatusColor = (status: LicenseStatus) => {
    switch (status) {
      case 'valid':
      case 'offline_valid':
        return 'text-green-500';
      case 'expired':
      case 'offline_expired':
        return 'text-red-500';
      default:
        return 'text-muted-foreground';
    }
  };

  const getStatusText = (status: LicenseStatus) => {
    switch (status) {
      case 'valid':
        return 'Pro · Active';
      case 'offline_valid':
        return 'Pro · Active (Offline)';
      case 'expired':
        return 'Expired';
      case 'offline_expired':
        return 'Expired (Offline)';
      case 'invalid':
        return 'Invalid';
      case 'device_mismatch':
        return 'Device Mismatch';
      default:
        return 'Free';
    }
  };

  const isExpired = (expiresAt: string | null) => {
    if (!expiresAt) return false;
    return new Date(expiresAt) < new Date();
  };

  if (!license) {
    return (
      <div className="flex items-center justify-center py-8">
        <p className="text-muted-foreground">Loading license information...</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-medium">License</h2>
        <p className="text-muted-foreground text-sm">
          Manage your Capty license
        </p>
      </div>

      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center gap-3">
            <img src={appIcon} alt="Capty" className="h-10 w-10 rounded-lg" />
            <div>
              <CardTitle>License Status</CardTitle>
              <CardDescription>
                <span className={getStatusColor(license.status)}>
                  {getStatusText(license.status)}
                </span>
              </CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          {license.info ? (
            <>
              <div className="space-y-3">
                <div className="flex items-center justify-between text-sm">
                  <span className="text-muted-foreground">Email</span>
                  <span className="font-medium">{license.info.email}</span>
                </div>
                <Separator />
                <div className="flex items-center justify-between text-sm">
                  <div className="flex items-center gap-2">
                    <Calendar className="text-muted-foreground h-4 w-4" />
                    <span className="text-muted-foreground">
                      {license.info.isLifetime
                        ? 'License Type'
                        : 'Will receive updates until'}
                    </span>
                  </div>
                  <span
                    className={`font-medium ${isExpired(license.info.expiresAt) ? 'text-red-500' : ''}`}
                  >
                    {license.info.isLifetime
                      ? 'Lifetime'
                      : license.info.expiresAt
                        ? formatDate(license.info.expiresAt)
                        : 'Unknown'}
                  </span>
                </div>
              </div>

              <Separator />

              {!showConfirmation ? (
                <div className="pt-2">
                  <Button
                    variant="destructive"
                    onClick={handleDeleteClick}
                    className="w-full"
                  >
                    <Trash2 className="mr-2 h-4 w-4" />
                    Delete License
                  </Button>
                  <p className="text-muted-foreground mt-2 text-center text-xs">
                    This will deactivate your license from this device
                  </p>
                </div>
              ) : (
                <div className="space-y-3 rounded-lg border border-red-500/20 bg-red-500/10 p-4">
                  <div className="flex items-start gap-3">
                    <AlertTriangle className="mt-0.5 h-5 w-5 shrink-0 text-red-500" />
                    <div className="space-y-1">
                      <p className="font-medium text-red-500">
                        Delete License?
                      </p>
                      <p className="text-muted-foreground text-sm">
                        This will deactivate your license from this device and
                        return Capty to the free tier. You will need to re-enter
                        your license key to restore Pro features.
                      </p>
                    </div>
                  </div>
                  <div className="flex gap-2">
                    <Button
                      variant="outline"
                      onClick={handleCancelDelete}
                      disabled={isDeleting}
                      className="flex-1"
                    >
                      Cancel
                    </Button>
                    <Button
                      variant="destructive"
                      onClick={handleConfirmDelete}
                      disabled={isDeleting}
                      className="flex-1"
                    >
                      {isDeleting ? 'Deleting...' : 'Delete'}
                    </Button>
                  </div>
                </div>
              )}
            </>
          ) : (
            <>
              <div className="space-y-2">
                <p className="text-sm font-medium">Unlock Capty Pro</p>
                <ul className="space-y-1.5">
                  {PRO_FEATURES.map(feature => (
                    <li
                      key={feature}
                      className="text-muted-foreground flex items-center gap-2 text-sm"
                    >
                      <Sparkles className="text-primary h-3.5 w-3.5 shrink-0" />
                      {feature}
                    </li>
                  ))}
                </ul>
              </div>

              <Separator />

              <div className="space-y-2 pt-2">
                <Button onClick={handlePurchaseLicense} className="w-full">
                  <ExternalLink className="mr-2 h-4 w-4" />
                  Upgrade to Pro
                </Button>
                <Button
                  variant="outline"
                  onClick={handleActivateLicense}
                  className="w-full"
                >
                  <Key className="mr-2 h-4 w-4" />
                  Enter License Key
                </Button>
              </div>
            </>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
