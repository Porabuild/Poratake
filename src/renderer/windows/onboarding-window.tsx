import { useState, useEffect, useCallback } from 'react';
import { Button } from '@/renderer/components/ui/button';
import {
  Check,
  X,
  Monitor,
  Accessibility,
  Shield,
  Keyboard,
  ExternalLink,
  ChevronRight,
  ChevronLeft,
} from 'lucide-react';
import type { PermissionsState } from '@/types/permissions';
import type { SettingsUiConfig } from '@/types/settings';
import ShortcutInput from '@/renderer/components/settings/shortcut-input';
import { isMacPlatform } from '@/renderer/utils/platform';
import appIcon from '@build/icon.png';

interface PermissionItemProps {
  title: string;
  description: string;
  icon: React.ReactNode;
  isGranted: boolean;
  onRequest: () => void;
}

function PermissionItem({
  title,
  description,
  icon,
  isGranted,
  onRequest,
}: PermissionItemProps) {
  return (
    <div className="rounded-md bg-muted p-3">
      <div className="flex items-start gap-3">
        <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full bg-primary/10">
          {icon}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-2">
            <h3 className="text-sm font-medium">{title}</h3>
            <div
              className={`flex h-5 w-5 items-center justify-center rounded-full ${
                isGranted ? 'bg-green-500/20' : 'bg-destructive/20'
              }`}
            >
              {isGranted ? (
                <Check className="h-3 w-3 text-green-500" />
              ) : (
                <X className="h-3 w-3 text-destructive" />
              )}
            </div>
          </div>
          <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
          {!isGranted && (
            <Button
              variant="outline"
              size="sm"
              className="mt-3"
              onClick={onRequest}
            >
              Open System Preferences
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}

interface StepIndicatorProps {
  currentStep: number;
  totalSteps: number;
}

function StepIndicator({ currentStep, totalSteps }: StepIndicatorProps) {
  return (
    <div className="flex items-center justify-center gap-2">
      {Array.from({ length: totalSteps }, (_, position) => (
        <div
          key={position}
          className={`h-2 w-2 rounded-full transition-colors ${
            position === currentStep
              ? 'bg-primary'
              : position < currentStep
                ? 'bg-primary/50'
                : 'bg-muted'
          }`}
        />
      ))}
    </div>
  );
}

type OnboardingStep =
  | 'welcome'
  | 'disable-macos-shortcuts'
  | 'shortcuts'
  | 'permissions';

const ONBOARDING_STEPS: OnboardingStep[] = isMacPlatform()
  ? ['welcome', 'disable-macos-shortcuts', 'shortcuts', 'permissions']
  : ['welcome', 'shortcuts'];

function WelcomeStep() {
  return (
    <div className="flex flex-col items-center text-center">
      <img
        src={appIcon}
        alt="Poratake"
        className="mx-auto mb-4 h-16 w-16 rounded-2xl"
      />
      <h1 className="text-xl font-semibold">Welcome to Poratake</h1>
      <p className="mt-2 text-sm text-muted-foreground">
        Your new screenshot tool
      </p>

      <div className="mt-6 space-y-3 text-left">
        <div className="flex items-start gap-3 rounded-md bg-muted p-3">
          <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full bg-blue-500/10">
            <Monitor className="h-4 w-4 text-blue-500" />
          </div>
          <div>
            <h3 className="text-sm font-medium">
              {isMacPlatform()
                ? 'Lives in Your Menu Bar'
                : 'Lives in Your System Tray'}
            </h3>
            <p className="mt-0.5 text-xs text-muted-foreground">
              Poratake runs quietly in your{' '}
              {isMacPlatform() ? 'menu bar' : 'system tray'}. Click the icon to
              access all features or use keyboard shortcuts for quick captures.
            </p>
          </div>
        </div>

        <div className="flex items-start gap-3 rounded-md bg-muted p-3">
          <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full bg-purple-500/10">
            <Keyboard className="h-4 w-4 text-purple-500" />
          </div>
          <div>
            <h3 className="text-sm font-medium">Powerful Shortcuts</h3>
            <p className="mt-0.5 text-xs text-muted-foreground">
              Capture screenshots instantly with customizable keyboard shortcuts
              for area, window, and full screen captures.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}

function DisableMacOSShortcutsStep() {
  const handleOpenKeyboardSettings = () => {
    window.ipcRenderer.send('onboarding:openKeyboardSettings');
  };

  return (
    <div className="flex flex-col">
      <div className="mb-4 text-center">
        <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-orange-500/10">
          <Keyboard className="h-8 w-8 text-orange-500" />
        </div>
        <h1 className="text-xl font-semibold">
          Disable macOS Screenshot Shortcuts
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">
          To use Poratake&apos;s shortcuts, you need to disable the default
          macOS screenshot shortcuts first.
        </p>
      </div>

      <div className="space-y-3">
        <div className="rounded-md bg-muted p-3">
          <h3 className="mb-2 text-sm font-medium">Follow these steps:</h3>
          <ol className="space-y-2 text-xs text-muted-foreground">
            <li className="flex gap-2">
              <span className="flex h-4 w-4 flex-shrink-0 items-center justify-center rounded-full bg-primary text-xs font-medium text-primary-foreground">
                1
              </span>
              <span>Click the button below to open Keyboard Settings</span>
            </li>
            <li className="flex gap-2">
              <span className="flex h-4 w-4 flex-shrink-0 items-center justify-center rounded-full bg-primary text-xs font-medium text-primary-foreground">
                2
              </span>
              <span>
                Select <strong>Screenshots</strong> in the left sidebar
              </span>
            </li>
            <li className="flex gap-2">
              <span className="flex h-4 w-4 flex-shrink-0 items-center justify-center rounded-full bg-primary text-xs font-medium text-primary-foreground">
                3
              </span>
              <span>
                Uncheck <strong>all screenshot shortcuts</strong> (⌘⇧3, ⌘⇧4,
                ⌘⇧5)
              </span>
            </li>
          </ol>
        </div>

        <Button
          variant="outline"
          className="w-full"
          onClick={handleOpenKeyboardSettings}
        >
          <ExternalLink className="mr-2 h-4 w-4" />
          Open Keyboard Settings
        </Button>

        <p className="text-center text-xs text-muted-foreground">
          You can skip this step, but Poratake&apos;s shortcuts may conflict
          with macOS defaults.
        </p>
      </div>
    </div>
  );
}

interface ShortcutsStepProps {
  settings: SettingsUiConfig | null;
  onShortcutChange: (
    type: 'area' | 'window' | 'screen',
    shortcut: string
  ) => void;
}

function ShortcutsStep({ settings, onShortcutChange }: ShortcutsStepProps) {
  if (!settings) return null;

  return (
    <div className="flex flex-col">
      <div className="mb-4 text-center">
        <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-green-500/10">
          <Keyboard className="h-8 w-8 text-green-500" />
        </div>
        <h1 className="text-xl font-semibold">Set Your Shortcuts</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Customize keyboard shortcuts for quick captures. Click to record a new
          shortcut.
        </p>
      </div>

      <div className="space-y-1 rounded-md bg-muted p-3">
        <ShortcutInput
          label="Capture Area"
          value={settings.shortcuts.screenshot.area}
          onChange={shortcut => onShortcutChange('area', shortcut)}
        />
        <div className="border-t" />
        <ShortcutInput
          label="Capture Window"
          value={settings.shortcuts.screenshot.window}
          onChange={shortcut => onShortcutChange('window', shortcut)}
        />
        <div className="border-t" />
        <ShortcutInput
          label="Capture Full Screen"
          value={settings.shortcuts.screenshot.screen}
          onChange={shortcut => onShortcutChange('screen', shortcut)}
        />
      </div>

      <p className="mt-3 text-center text-xs text-muted-foreground">
        You can change these anytime in Settings → Shortcuts
      </p>
    </div>
  );
}

interface PermissionsStepProps {
  permissions: PermissionsState;
  onOpenScreenRecording: () => void;
  onOpenAccessibility: () => void;
}

function PermissionsStep({
  permissions,
  onOpenScreenRecording,
  onOpenAccessibility,
}: PermissionsStepProps) {
  const isScreenRecordingGranted = permissions.screenRecording === 'granted';
  const isAccessibilityGranted = permissions.accessibility;

  return (
    <div className="flex flex-col">
      <div className="mb-4 text-center">
        <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-primary/10">
          <Shield className="h-8 w-8 text-primary" />
        </div>
        <h1 className="text-xl font-semibold">Setup Permissions</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Poratake needs these permissions to work properly
        </p>
      </div>

      <div className="space-y-3">
        <PermissionItem
          title="Screen Recording"
          description="Required to capture screenshots of your screen"
          icon={<Monitor className="h-4 w-4 text-primary" />}
          isGranted={isScreenRecordingGranted}
          onRequest={onOpenScreenRecording}
        />

        <PermissionItem
          title="Accessibility"
          description="Required to hide desktop icons when capturing"
          icon={<Accessibility className="h-4 w-4 text-primary" />}
          isGranted={isAccessibilityGranted}
          onRequest={onOpenAccessibility}
        />
      </div>
    </div>
  );
}

const TOTAL_STEPS = ONBOARDING_STEPS.length;

export default function OnboardingWindow() {
  const [step, setStep] = useState(0);
  const [permissions, setPermissions] = useState<PermissionsState>({
    screenRecording: 'not-determined',
    accessibility: false,
    microphone: 'not-determined',
    camera: 'not-determined',
  });
  const [settings, setSettings] = useState<SettingsUiConfig | null>(null);

  const isScreenRecordingGranted = permissions.screenRecording === 'granted';
  const isAccessibilityGranted = permissions.accessibility;
  const allPermissionsGranted =
    isScreenRecordingGranted && isAccessibilityGranted;

  const checkPermissions = useCallback(async () => {
    const status = (await window.ipcRenderer.invoke(
      'permissions:getStatus'
    )) as PermissionsState;
    setPermissions(status);
  }, []);

  const loadSettings = useCallback(async () => {
    const loadedSettings = (await window.ipcRenderer.invoke(
      'settings:get-ui'
    )) as SettingsUiConfig;
    setSettings(loadedSettings);
  }, []);

  useEffect(() => {
    checkPermissions();
    loadSettings();

    const interval = setInterval(checkPermissions, 1000);

    return () => clearInterval(interval);
  }, [checkPermissions, loadSettings]);

  const handleOpenScreenRecording = () => {
    window.ipcRenderer.send('permissions:openScreenRecording');
  };

  const handleOpenAccessibility = () => {
    window.ipcRenderer.send('permissions:openAccessibility');
  };

  const handleShortcutChange = async (
    type: 'area' | 'window' | 'screen',
    shortcut: string
  ) => {
    if (!settings) return;

    const newSettings = {
      ...settings,
      shortcuts: {
        ...settings.shortcuts,
        screenshot: {
          ...settings.shortcuts.screenshot,
          [type]: shortcut,
        },
      },
    };

    setSettings(newSettings);
    await window.ipcRenderer.invoke('settings:update', {
      shortcuts: newSettings.shortcuts,
    });
  };

  const handleContinue = () => {
    window.ipcRenderer.send('onboarding:complete');
  };

  const handleSkip = () => {
    window.ipcRenderer.send('onboarding:skip');
  };

  const handleNext = () => {
    if (step < TOTAL_STEPS - 1) {
      setStep(step + 1);
    }
  };

  const handleBack = () => {
    if (step > 0) {
      setStep(step - 1);
    }
  };

  const isLastStep = step === TOTAL_STEPS - 1;
  const currentStep = ONBOARDING_STEPS[step];
  const canProceedOnLastStep =
    currentStep === 'permissions' ? allPermissionsGranted : true;

  const renderStep = () => {
    switch (currentStep) {
      case 'welcome':
        return <WelcomeStep />;
      case 'disable-macos-shortcuts':
        return <DisableMacOSShortcutsStep />;
      case 'shortcuts':
        return (
          <ShortcutsStep
            settings={settings}
            onShortcutChange={handleShortcutChange}
          />
        );
      case 'permissions':
        return (
          <PermissionsStep
            permissions={permissions}
            onOpenScreenRecording={handleOpenScreenRecording}
            onOpenAccessibility={handleOpenAccessibility}
          />
        );
      default:
        return null;
    }
  };

  return (
    <div className="flex h-screen flex-col bg-background">
      {/* Drag strip standing in for the hidden native title bar */}
      <div
        className="h-8 w-full flex-none"
        style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
      />

      <div className="flex min-h-0 flex-1 flex-col overflow-auto p-6">
        <div className="flex-1">{renderStep()}</div>

        <div className="mt-6 space-y-4">
          <StepIndicator currentStep={step} totalSteps={TOTAL_STEPS} />

          <div className="flex gap-2">
            {step > 0 && (
              <Button variant="ghost" onClick={handleBack} className="flex-1">
                <ChevronLeft className="mr-1 h-4 w-4" />
                Back
              </Button>
            )}

            {isLastStep ? (
              <Button
                onClick={handleContinue}
                disabled={!canProceedOnLastStep}
                className="flex-1"
              >
                Get Started
              </Button>
            ) : (
              <Button
                variant="tertiary"
                onClick={handleNext}
                className="flex-1"
              >
                Next
                <ChevronRight className="ml-1 h-4 w-4" />
              </Button>
            )}
          </div>

          <Button
            variant="ghost"
            onClick={handleSkip}
            className="w-full text-muted-foreground"
          >
            Skip for now
          </Button>
        </div>
      </div>
    </div>
  );
}
