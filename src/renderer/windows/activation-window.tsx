import { useState, FormEvent } from 'react';
import { ExternalLink, Key, Loader2 } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import { Input } from '@/renderer/components/ui/input';
import { InputError } from '@/renderer/components/ui/input-error';
import { Label } from '@/renderer/components/ui/label';
import { Separator } from '@/renderer/components/ui/separator';

interface ActivationResult {
  valid: boolean;
  error?: string;
  message?: string;
}

const ERROR_MESSAGES: Record<string, string> = {
  email_not_found: 'No account found with this email address.',
  invalid_key: 'Invalid license key for this email.',
  expired: 'This license has expired. Please renew to continue.',
  revoked: 'This license has been revoked.',
  version_not_entitled:
    'Your license does not cover this version. Please renew.',
  network_error:
    'Unable to connect to license server. Please check your internet.',
};

export default function ActivationWindow() {
  const [email, setEmail] = useState('');
  const [licenseKey, setLicenseKey] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [licenseError, setLicenseError] = useState<string | null>(null);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();

    if (!email || !licenseKey) {
      setLicenseError('Please enter both email and license key');
      return;
    }

    setIsLoading(true);
    setLicenseError(null);

    try {
      const result = (await window.ipcRenderer.invoke(
        'license:activate',
        email,
        licenseKey
      )) as ActivationResult;

      if (result.valid) {
        window.ipcRenderer.send('license:activated');
      } else {
        const errorMessage =
          result.message ||
          ERROR_MESSAGES[result.error || ''] ||
          'License activation failed. Please try again.';
        setLicenseError(errorMessage);
      }
    } catch {
      setLicenseError('An unexpected error occurred. Please try again.');
    } finally {
      setIsLoading(false);
    }
  };

  const openLogin = async () => {
    const url = (await window.ipcRenderer.invoke(
      'license:getLoginUrl'
    )) as string;
    window.open(url, '_blank');
  };

  const openCheckout = async () => {
    const url = (await window.ipcRenderer.invoke(
      'license:getCheckoutUrl'
    )) as string;
    window.open(url, '_blank');
  };

  const handleClose = () => {
    window.ipcRenderer.send('license:close');
  };

  return (
    <div className="bg-background flex h-screen flex-col">
      <div
        className="h-8 w-full flex-none"
        style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
      />

      <div className="flex min-h-0 flex-1 flex-col p-6">
        <div className="mb-6 text-center">
          <div className="bg-primary/10 mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full">
            <Key className="text-primary h-8 w-8" />
          </div>
          <h1 className="text-xl font-semibold">Activate Capty License</h1>
          <p className="text-muted-foreground mt-1 text-sm">
            Enter your email and license key to unlock Pro features
          </p>
          <p className="text-muted-foreground mt-2 text-xs">
            Activation is provided by Capty. Your email, license key, device
            identifier, device name, platform, and Poratake version are sent to
            capty.app. Capty&apos;s terms govern service access for modified
            builds.
          </p>
        </div>

        <form onSubmit={handleSubmit}>
          <div className="grid gap-6">
            <div className="grid gap-2">
              <Label htmlFor="email">Email Address</Label>
              <Input
                id="email"
                type="email"
                autoFocus
                tabIndex={1}
                autoComplete="email"
                placeholder="you@example.com"
                value={email}
                onChange={e => setEmail(e.target.value)}
                disabled={isLoading}
              />
            </div>

            <div className="grid gap-2">
              <Label htmlFor="license-key">License Key</Label>
              <Input
                id="license-key"
                type="text"
                tabIndex={2}
                autoComplete="off"
                placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
                value={licenseKey}
                onChange={e => setLicenseKey(e.target.value)}
                disabled={isLoading}
                className="font-mono"
              />
            </div>

            <InputError message={licenseError} />

            <Button
              type="submit"
              tabIndex={3}
              disabled={isLoading || !email || !licenseKey}
              className="w-full"
            >
              {isLoading ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Activating...
                </>
              ) : (
                'Activate License'
              )}
            </Button>
          </div>
        </form>

        <div className="my-6 flex items-center gap-3">
          <Separator className="flex-1" />
          <span className="text-muted-foreground text-xs">or</span>
          <Separator className="flex-1" />
        </div>

        <div className="flex gap-2">
          <Button variant="outline" onClick={openLogin} className="flex-1">
            <Key className="mr-2 h-4 w-4" />
            Retrieve License Key
          </Button>
          <Button variant="outline" onClick={openCheckout} className="flex-1">
            <ExternalLink className="mr-2 h-4 w-4" />
            Purchase
          </Button>
        </div>

        <div className="text-muted-foreground mt-3 flex justify-center gap-3 text-xs">
          <button
            type="button"
            className="hover:text-foreground transition-colors"
            onClick={() => window.open('https://capty.app/terms', '_blank')}
          >
            Capty Terms
          </button>
          <button
            type="button"
            className="hover:text-foreground transition-colors"
            onClick={() => window.open('https://capty.app/privacy', '_blank')}
          >
            Capty Privacy
          </button>
        </div>

        <div className="mt-auto pt-6">
          <Button
            variant="ghost"
            onClick={handleClose}
            className="text-muted-foreground w-full"
          >
            Close
          </Button>
        </div>
      </div>
    </div>
  );
}
