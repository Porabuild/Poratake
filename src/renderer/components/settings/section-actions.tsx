import { ExternalLink } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import type { SettingsConfig } from '@/types/settings';

const CAPTY_CLOUD_REPO_URL = 'https://github.com/capty-app/cloud';

export function getSectionAction(
  category: string,
  section: string,
  settings: SettingsConfig
): React.ReactNode {
  if (
    category === 'cloud' &&
    section === 'Cloud Upload' &&
    settings.cloud.activeProvider === 'rest'
  ) {
    return (
      <Button
        variant="outline"
        size="sm"
        onClick={() =>
          window.ipcRenderer.send('open-external', CAPTY_CLOUD_REPO_URL)
        }
      >
        <ExternalLink className="mr-2 size-4" />
        Self-host Capty Cloud
      </Button>
    );
  }
  return null;
}
