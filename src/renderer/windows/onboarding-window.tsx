import { useState, useEffect, useCallback } from 'react';
import { Button } from '@/renderer/components/ui/button';
import {
  Check,
  X,
  Monitor,
  Accessibility,
  Shield,
  Command,
  Keyboard,
  ExternalLink,
  ChevronRight,
  ChevronLeft,
} from 'lucide-react';
import type { PermissionsState } from '@/types/permissions';
import type { SettingsConfig } from '@/types/settings';
import ShortcutInput from '@/renderer/components/settings/shortcut-input';

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
    <div className="bg-card rounded-lg border p-4">
      <div className="flex items-start gap-3">
        <div className="bg-primary/10 flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-full">
          {icon}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-2">
            <h3 className="font-medium">{title}</h3>
            <div
              className={`flex h-6 w-6 items-center justify-center rounded-full ${
                isGranted ? 'bg-green-500/20' : 'bg-destructive/20'
              }`}
            >
              {isGranted ? (
                <Check className="h-4 w-4 text-green-500" />
              ) : (
                <X className="text-destructive h-4 w-4" />
              )}
            </div>
          </div>
          <p className="text-muted-foreground mt-1 text-sm">{description}</p>
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
      {Array.from({ length: totalSteps }).map((_, index) => (
        <div
          key={index}
          className={`h-2 w-2 rounded-full transition-colors ${
            index === currentStep
              ? 'bg-primary'
              : index < currentStep
                ? 'bg-primary/50'
                : 'bg-muted'
          }`}
        />
      ))}
    </div>
  );
}

function WelcomeStep() {
  return (
    <div className="flex flex-col items-center text-center">
      <div className="bg-primary/10 mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full">
        <Command className="text-primary h-8 w-8" />
      </div>
      <h1 className="text-xl font-semibold">Welcome to Capty</h1>
      <p className="text-muted-foreground mt-2 text-sm">
        Your new screenshot tool for macOS
      </p>

      <div className="mt-6 space-y-4 text-left">
        <div className="bg-card flex items-start gap-3 rounded-lg border p-4">
          <div className="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-full bg-blue-500/10">
            <Monitor className="h-5 w-5 text-blue-500" />
          </div>
          <div>
            <h3 className="font-medium">Lives in Your Menu Bar</h3>
            <p className="text-muted-foreground mt-1 text-sm">
              Capty runs quietly in your menu bar. Click the icon to access all
              features or use keyboard shortcuts for quick captures.
            </p>
          </div>
        </div>

        <div className="bg-card flex items-start gap-3 rounded-lg border p-4">
          <div className="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-full bg-purple-500/10">
            <Keyboard className="h-5 w-5 text-purple-500" />
          </div>
          <div>
            <h3 className="font-medium">Powerful Shortcuts</h3>
            <p className="text-muted-foreground mt-1 text-sm">
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
    window.ipcRenderer.send(
      'shell:open-external',
      'x-apple.systempreferences:com.apple.preference.keyboard?Shortcuts'
    );
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
        <p className="text-muted-foreground mt-1 text-sm">
          To use Capty&apos;s shortcuts, you need to disable the default macOS
          screenshot shortcuts first.
        </p>
      </div>

      <div className="space-y-3">
        <div className="bg-card rounded-lg border p-4">
          <h3 className="mb-2 font-medium">Follow these steps:</h3>
          <ol className="text-muted-foreground space-y-2 text-sm">
            <li className="flex gap-2">
              <span className="bg-primary text-primary-foreground flex h-5 w-5 flex-shrink-0 items-center justify-center rounded-full text-xs font-medium">
                1
              </span>
              <span>Click the button below to open Keyboard Settings</span>
            </li>
            <li className="flex gap-2">
              <span className="bg-primary text-primary-foreground flex h-5 w-5 flex-shrink-0 items-center justify-center rounded-full text-xs font-medium">
                2
              </span>
              <span>
                Select <strong>Screenshots</strong> in the left sidebar
              </span>
            </li>
            <li className="flex gap-2">
              <span className="bg-primary text-primary-foreground flex h-5 w-5 flex-shrink-0 items-center justify-center rounded-full text-xs font-medium">
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

        <p className="text-muted-foreground text-center text-xs">
          You can skip this step, but Capty&apos;s shortcuts may conflict with
          macOS defaults.
        </p>
      </div>
    </div>
  );
}

interface ShortcutsStepProps {
  settings: SettingsConfig | null;
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
        <p className="text-muted-foreground mt-1 text-sm">
          Customize keyboard shortcuts for quick captures. Click to record a new
          shortcut.
        </p>
      </div>

      <div className="bg-card space-y-1 rounded-lg border p-4">
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

      <p className="text-muted-foreground mt-3 text-center text-xs">
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
        <div className="bg-primary/10 mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full">
          <Shield className="text-primary h-8 w-8" />
        </div>
        <h1 className="text-xl font-semibold">Setup Permissions</h1>
        <p className="text-muted-foreground mt-1 text-sm">
          Capty needs these permissions to work properly
        </p>
      </div>

      <div className="space-y-3">
        <PermissionItem
          title="Screen Recording"
          description="Required to capture screenshots of your screen"
          icon={<Monitor className="text-primary h-5 w-5" />}
          isGranted={isScreenRecordingGranted}
          onRequest={onOpenScreenRecording}
        />

        <PermissionItem
          title="Accessibility"
          description="Required to hide desktop icons when capturing"
          icon={<Accessibility className="text-primary h-5 w-5" />}
          isGranted={isAccessibilityGranted}
          onRequest={onOpenAccessibility}
        />
      </div>
    </div>
  );
}

const TOTAL_STEPS = 4;

export default function OnboardingWindow() {
  const [step, setStep] = useState(0);
  const [permissions, setPermissions] = useState<PermissionsState>({
    screenRecording: 'not-determined',
    accessibility: false,
    microphone: 'not-determined',
    camera: 'not-determined',
  });
  const [settings, setSettings] = useState<SettingsConfig | null>(null);

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
      'settings:get'
    )) as SettingsConfig;
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
  const canProceedOnLastStep = allPermissionsGranted;

  const renderStep = () => {
    switch (step) {
      case 0:
        return <WelcomeStep />;
      case 1:
        return <DisableMacOSShortcutsStep />;
      case 2:
        return (
          <ShortcutsStep
            settings={settings}
            onShortcutChange={handleShortcutChange}
          />
        );
      case 3:
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
    <div className="bg-background flex h-screen flex-col">
      {}
      <div
        className="h-8 w-full flex-none"
        style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
      />

      <div className="flex min-h-0 flex-1 flex-col overflow-auto p-6">
        {}
        <div className="flex-1">{renderStep()}</div>

        {}
        <div className="mt-6 space-y-4">
          {}
          <StepIndicator currentStep={step} totalSteps={TOTAL_STEPS} />

          {}
          <div className="flex gap-2">
            {step > 0 && (
              <Button variant="outline" onClick={handleBack} className="flex-1">
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
              <Button onClick={handleNext} className="flex-1">
                Next
                <ChevronRight className="ml-1 h-4 w-4" />
              </Button>
            )}
          </div>

          <Button
            variant="ghost"
            onClick={handleSkip}
            className="text-muted-foreground w-full"
          >
            Skip for now
          </Button>
        </div>
      </div>
    </div>
  );
}
