import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import { Dropdown, Separator } from '@heroui/react';
import {
  ChevronDown,
  Circle,
  Mic,
  MicOff,
  Pause,
  Play,
  Smartphone,
  Square,
  Trash2,
  Video,
  VideoOff,
  Volume2,
  VolumeX,
  X,
} from 'lucide-react';
import ToolbarButton from '@/renderer/components/area-overlay/toolbar-button';
import ToolbarSurface from '@/renderer/components/area-overlay/toolbar-surface';
import { cn } from '@/renderer/lib/utils';
import type { MediaDeviceDescriptor, MediaDeviceLists } from '@/types/devices';
import type {
  RecordingControlAction,
  RecordingControlDeviceKind,
  RecordingControlState,
} from '@/types/recording-control';
import { isMacPlatform } from '@/renderer/utils/platform';

const TOGGLE_DEVICE_KEY = 'toggle';
const DEFAULT_DEVICE_KEY = 'default';
const EMPTY_MEDIA_DEVICES: MediaDeviceLists = {
  microphones: [],
  cameras: [],
  defaultMicrophoneId: null,
  defaultCameraId: null,
};

function formatElapsedTime(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainingSeconds = seconds % 60;
  const segments = [minutes, remainingSeconds];

  if (hours > 0) {
    segments.unshift(hours);
  }

  return segments.map(segment => segment.toString().padStart(2, '0')).join(':');
}

function ControlButton({
  label,
  children,
  ...props
}: React.ComponentProps<typeof ToolbarButton> & { label: string }) {
  return (
    <ToolbarButton aria-label={label} title={label} {...props}>
      {children}
    </ToolbarButton>
  );
}

interface DeviceDropdownProps {
  label: string;
  toggleLabel: string;
  devices: MediaDeviceDescriptor[];
  defaultDeviceId: string | null;
  selectedDeviceId: string | null;
  enabled: boolean;
  disabled: boolean;
  deviceLocked?: boolean;
  isOpen: boolean;
  icon: React.ReactNode;
  onOpenChange: (isOpen: boolean) => void;
  onExitComplete: () => void;
  onToggle: () => void;
  onSelect: (device: MediaDeviceDescriptor | null) => void;
}

function usePopoverExit(isOpen: boolean, onExitComplete: () => void) {
  const isOpenRef = useRef(isOpen);
  const wasMountedRef = useRef(false);
  const exitPendingRef = useRef(false);
  const onExitCompleteRef = useRef(onExitComplete);

  useLayoutEffect(() => {
    isOpenRef.current = isOpen;
    onExitCompleteRef.current = onExitComplete;
    if (!isOpen && exitPendingRef.current) {
      exitPendingRef.current = false;
      onExitComplete();
    }
  }, [isOpen, onExitComplete]);

  return useCallback((element: HTMLElement | null) => {
    if (element) {
      wasMountedRef.current = true;
      exitPendingRef.current = false;
      return;
    }

    if (!wasMountedRef.current) return;
    wasMountedRef.current = false;
    if (isOpenRef.current) {
      exitPendingRef.current = true;
      return;
    }
    onExitCompleteRef.current();
  }, []);
}

function DeviceDropdown({
  label,
  toggleLabel,
  devices,
  defaultDeviceId,
  selectedDeviceId,
  enabled,
  disabled,
  deviceLocked = false,
  isOpen,
  icon,
  onOpenChange,
  onExitComplete,
  onToggle,
  onSelect,
}: DeviceDropdownProps) {
  const handlePopoverRef = usePopoverExit(isOpen, onExitComplete);

  const lockedDeviceId = selectedDeviceId ?? defaultDeviceId;
  const selectedKeys = [
    ...(enabled ? [TOGGLE_DEVICE_KEY] : []),
    selectedDeviceId ?? DEFAULT_DEVICE_KEY,
  ];
  const isDeviceLocked = (deviceId: string) =>
    deviceLocked && deviceId !== lockedDeviceId;
  const isDefaultLocked =
    deviceLocked &&
    selectedDeviceId !== null &&
    selectedDeviceId !== defaultDeviceId;
  const defaultDevice =
    devices.find(device => device.id === defaultDeviceId) ?? null;
  const defaultLabel = defaultDevice
    ? `System Default (${defaultDevice.label})`
    : 'System Default';

  const handleAction = (key: React.Key) => {
    if (key === TOGGLE_DEVICE_KEY) {
      onToggle();
      return;
    }

    if (key === DEFAULT_DEVICE_KEY) {
      if (!isDefaultLocked) onSelect(null);
      return;
    }

    const device = devices.find(item => item.id === key);
    if (device && !isDeviceLocked(device.id)) onSelect(device);
  };

  return (
    <Dropdown isOpen={isOpen} onOpenChange={onOpenChange}>
      <Dropdown.Trigger
        aria-label={label}
        aria-pressed={enabled}
        isDisabled={disabled}
        className="inline-flex h-8 w-12 min-w-12 flex-row items-center justify-center gap-1 rounded-3xl px-1.5 whitespace-nowrap text-white/85 outline-none [--button-fg:rgb(255_255_255/0.85)] hover:bg-white/15 hover:text-white hover:[--button-fg:white] disabled:pointer-events-none disabled:opacity-35"
      >
        {icon}
        <ChevronDown className="size-3" />
      </Dropdown.Trigger>
      <Dropdown.Popover
        ref={handlePopoverRef}
        placement="bottom"
        className="max-w-64 min-w-56"
      >
        <Dropdown.Menu
          aria-label={`${label} devices`}
          selectionMode="multiple"
          selectedKeys={selectedKeys}
          onAction={handleAction}
          className="max-h-56 overflow-y-auto"
        >
          <Dropdown.Item id={TOGGLE_DEVICE_KEY} textValue={toggleLabel}>
            <span className="min-w-0 flex-1 truncate">{toggleLabel}</span>
            <Dropdown.ItemIndicator className="text-foreground" />
          </Dropdown.Item>
          <Separator />
          <Dropdown.Item
            id={DEFAULT_DEVICE_KEY}
            textValue="System Default"
            isDisabled={isDefaultLocked}
          >
            <span className="min-w-0 flex-1 truncate">{defaultLabel}</span>
            <Dropdown.ItemIndicator className="text-foreground" />
          </Dropdown.Item>
          {devices.map(device => (
            <Dropdown.Item
              key={device.id}
              id={device.id}
              textValue={device.label}
              isDisabled={isDeviceLocked(device.id)}
            >
              <span className="min-w-0 flex-1 truncate">{device.label}</span>
              <Dropdown.ItemIndicator className="text-foreground" />
            </Dropdown.Item>
          ))}
        </Dropdown.Menu>
      </Dropdown.Popover>
    </Dropdown>
  );
}

const IOS_NONE_KEY = 'none';

function CountdownSurface({
  seconds,
  onCancel,
}: {
  seconds: number;
  onCancel: () => void;
}) {
  return (
    <ToolbarSurface className="gap-3 px-4 py-2.5">
      <span className="text-3xl leading-none font-semibold text-foreground tabular-nums">
        {seconds}
      </span>
      <div className="flex flex-col gap-0.5">
        <span className="text-xs font-medium text-foreground">
          Recording starts soon
        </span>
        <span className="text-xs text-muted-foreground">
          Press Escape to cancel
        </span>
      </div>
      <ControlButton label="Cancel countdown" onClick={onCancel}>
        <X className="size-4" />
      </ControlButton>
    </ToolbarSurface>
  );
}

interface IOSDeviceDropdownProps {
  devices: MediaDeviceDescriptor[];
  selectedDeviceId: string | null;
  disabled: boolean;
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  onExitComplete: () => void;
  onSelect: (device: MediaDeviceDescriptor | null) => void;
}

function IOSDeviceDropdown({
  devices,
  selectedDeviceId,
  disabled,
  isOpen,
  onOpenChange,
  onExitComplete,
  onSelect,
}: IOSDeviceDropdownProps) {
  const handlePopoverRef = usePopoverExit(isOpen, onExitComplete);
  const selectedKeys = [selectedDeviceId ?? IOS_NONE_KEY];

  const handleAction = (key: React.Key) => {
    if (key === IOS_NONE_KEY) {
      onSelect(null);
      return;
    }

    const device = devices.find(item => item.id === key);
    if (device) onSelect(device);
  };

  return (
    <Dropdown isOpen={isOpen} onOpenChange={onOpenChange}>
      <Dropdown.Trigger
        aria-label="Select iPhone or iPad"
        aria-pressed={selectedDeviceId !== null}
        isDisabled={disabled}
        className={cn(
          'inline-flex h-8 w-12 min-w-12 flex-row items-center justify-center gap-1 rounded-3xl px-1.5 whitespace-nowrap outline-none hover:bg-white/15 disabled:pointer-events-none disabled:opacity-35',
          selectedDeviceId
            ? 'text-primary hover:text-primary'
            : 'text-white/85 hover:text-white'
        )}
      >
        <Smartphone className="size-4" />
        <ChevronDown className="size-3" />
      </Dropdown.Trigger>
      <Dropdown.Popover
        ref={handlePopoverRef}
        placement="bottom"
        className="max-w-64 min-w-56"
      >
        <Dropdown.Menu
          aria-label="iPhone and iPad devices"
          selectionMode="multiple"
          selectedKeys={selectedKeys}
          onAction={handleAction}
          className="max-h-56 overflow-y-auto"
        >
          <Dropdown.Item id={IOS_NONE_KEY} textValue="None">
            <span className="min-w-0 flex-1 truncate">None</span>
            <Dropdown.ItemIndicator className="text-foreground" />
          </Dropdown.Item>
          <Separator />
          {devices.map(device => (
            <Dropdown.Item
              key={device.id}
              id={device.id}
              textValue={device.label}
            >
              <span className="min-w-0 flex-1 truncate">{device.label}</span>
              <Dropdown.ItemIndicator className="text-foreground" />
            </Dropdown.Item>
          ))}
        </Dropdown.Menu>
      </Dropdown.Popover>
    </Dropdown>
  );
}

export default function RecordingControlWindow({
  params,
}: {
  params: RecordingControlState;
}) {
  const [state, setState] = useState(params);
  const [devices, setDevices] = useState(EMPTY_MEDIA_DEVICES);
  const [iosDevices, setIosDevices] = useState<MediaDeviceDescriptor[]>([]);
  const [openDeviceMenu, setOpenDeviceMenu] =
    useState<RecordingControlDeviceKind | null>(null);
  const openDeviceMenuRef = useRef<RecordingControlDeviceKind | null>(null);
  const toolbarRef = useRef<HTMLDivElement | null>(null);
  const isRecording = state.mode === 'recording';
  const isMac = isMacPlatform();

  useLayoutEffect(() => {
    openDeviceMenuRef.current = null;
    setOpenDeviceMenu(null);
    setState(params);
  }, [params]);

  useEffect(() => {
    const toolbar = toolbarRef.current;
    if (!toolbar) return;

    const report = () => {
      const width = Math.round(toolbar.offsetWidth);
      if (width > 0) {
        window.ipcRenderer.send('recording-control:content-width', width);
      }
    };

    report();
    const observer = new ResizeObserver(report);
    observer.observe(toolbar);
    return () => {
      observer.disconnect();
    };
  }, []);

  useEffect(() => {
    window.ipcRenderer.send('recording-control:ready');
    (document.activeElement as HTMLElement | null)?.blur();
  }, []);

  useEffect(() => {
    const handleUpdate = (
      _event: unknown,
      update: Partial<RecordingControlState>
    ) => setState(current => ({ ...current, ...update }));

    window.ipcRenderer.on('recording-control:update', handleUpdate);
    return () => {
      window.ipcRenderer.off('recording-control:update', handleUpdate);
    };
  }, []);

  const sendAction = useCallback((action: RecordingControlAction) => {
    window.ipcRenderer.send('recording-control:action', action);
  }, []);

  const handleDeviceSelect = useCallback(
    (
      kind: RecordingControlDeviceKind,
      device: MediaDeviceDescriptor | null
    ) => {
      const actions: Record<
        RecordingControlDeviceKind,
        RecordingControlAction
      > = {
        microphone: 'select-mic',
        camera: 'select-camera',
        'ios-device': 'select-ios-device',
      };
      window.ipcRenderer.send('recording-control:action', actions[kind], {
        deviceId: device?.id ?? null,
        deviceName: device?.label ?? null,
      });
    },
    []
  );

  const refreshDevices = useCallback(async (kind: 'microphone' | 'camera') => {
    try {
      const availableDevices: MediaDeviceLists =
        await window.ipcRenderer.invoke('recording-control:devices', kind);
      setDevices(current => {
        if (kind === 'microphone') {
          return {
            ...current,
            microphones: availableDevices.microphones,
            defaultMicrophoneId: availableDevices.defaultMicrophoneId,
          };
        }
        return {
          ...current,
          cameras: availableDevices.cameras,
          defaultCameraId: availableDevices.defaultCameraId,
        };
      });
    } catch (error) {
      console.error('Failed to list recording devices:', error);
    }
  }, []);

  const selectedIOSDeviceIdRef = useRef(state.selectedIOSDeviceId);
  useLayoutEffect(() => {
    selectedIOSDeviceIdRef.current = state.selectedIOSDeviceId;
  }, [state.selectedIOSDeviceId]);

  const refreshIOSDevices = useCallback(async () => {
    try {
      const availableDevices: MediaDeviceDescriptor[] =
        await window.ipcRenderer.invoke('recording-control:ios-devices');
      setIosDevices(availableDevices);
      const selectedId = selectedIOSDeviceIdRef.current;
      if (
        selectedId !== null &&
        !availableDevices.some(device => device.id === selectedId)
      ) {
        handleDeviceSelect('ios-device', null);
      }
    } catch (error) {
      console.error('Failed to list iOS devices:', error);
    }
  }, [handleDeviceSelect]);

  const handleDeviceMenuOpenChange = useCallback(
    (kind: RecordingControlDeviceKind, isOpen: boolean) => {
      const nextOpenDeviceMenu = isOpen ? kind : null;
      openDeviceMenuRef.current = nextOpenDeviceMenu;
      setOpenDeviceMenu(nextOpenDeviceMenu);
      if (!isOpen) return;

      window.ipcRenderer.send('recording-control:device-menu-open', true);
      if (kind === 'ios-device') {
        void refreshIOSDevices();
        return;
      }
      void refreshDevices(kind);
    },
    [refreshDevices, refreshIOSDevices]
  );

  const handleDeviceMenuExit = useCallback(() => {
    if (openDeviceMenuRef.current) return;
    window.ipcRenderer.send('recording-control:device-menu-open', false);
  }, []);

  const handleDeviceToggle = useCallback(
    (kind: RecordingControlDeviceKind) => {
      sendAction(kind === 'microphone' ? 'toggle-mic' : 'toggle-camera');
    },
    [sendAction]
  );

  useEffect(() => {
    const isCountdownActive = state.countdownSeconds != null;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        event.key !== 'Escape' ||
        isRecording ||
        (state.isStarting && !isCountdownActive)
      ) {
        return;
      }
      sendAction('cancel');
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isRecording, sendAction, state.isStarting, state.countdownSeconds]);

  const cameraDropdown = (
    <DeviceDropdown
      label="Select camera"
      toggleLabel="Camera"
      disabled={state.isStarting}
      deviceLocked={isRecording}
      enabled={state.cameraEnabled}
      selectedDeviceId={state.selectedCameraId}
      defaultDeviceId={devices.defaultCameraId}
      devices={devices.cameras}
      isOpen={openDeviceMenu === 'camera'}
      onOpenChange={isOpen => handleDeviceMenuOpenChange('camera', isOpen)}
      onExitComplete={handleDeviceMenuExit}
      onToggle={() => handleDeviceToggle('camera')}
      onSelect={device => handleDeviceSelect('camera', device)}
      icon={
        state.cameraEnabled ? (
          <Video className="size-4" />
        ) : (
          <VideoOff className="size-4" />
        )
      }
    />
  );

  const microphoneDropdown = (
    <DeviceDropdown
      label="Select microphone"
      toggleLabel="Microphone"
      disabled={state.isStarting}
      enabled={state.micEnabled}
      selectedDeviceId={state.selectedMicId}
      defaultDeviceId={devices.defaultMicrophoneId}
      devices={devices.microphones}
      isOpen={openDeviceMenu === 'microphone'}
      onOpenChange={isOpen => handleDeviceMenuOpenChange('microphone', isOpen)}
      onExitComplete={handleDeviceMenuExit}
      onToggle={() => handleDeviceToggle('microphone')}
      onSelect={device => handleDeviceSelect('microphone', device)}
      icon={
        state.micEnabled ? (
          <Mic className="size-4" />
        ) : (
          <MicOff className="size-4" />
        )
      }
    />
  );

  const systemAudioButton = (
    <ControlButton
      label={
        state.systemAudio ? 'Turn system sounds off' : 'Turn system sounds on'
      }
      disabled={state.isStarting}
      aria-pressed={state.systemAudio}
      onClick={() => sendAction('toggle-system-audio')}
    >
      {state.systemAudio ? (
        <Volume2 className="size-4" />
      ) : (
        <VolumeX className="size-4" />
      )}
    </ControlButton>
  );

  const iosDeviceDropdown = (
    <IOSDeviceDropdown
      devices={iosDevices}
      selectedDeviceId={state.selectedIOSDeviceId}
      disabled={state.isStarting}
      isOpen={openDeviceMenu === 'ios-device'}
      onOpenChange={isOpen => handleDeviceMenuOpenChange('ios-device', isOpen)}
      onExitComplete={handleDeviceMenuExit}
      onSelect={device => handleDeviceSelect('ios-device', device)}
    />
  );

  return (
    <div className="flex h-screen w-screen flex-col items-center gap-2 pt-1">
      <ToolbarSurface ref={toolbarRef}>
        {state.targetName ? (
          <>
            <span
              className="max-w-32 truncate px-1 text-xs text-foreground"
              title={state.targetName}
            >
              {state.targetName}
            </span>
            <div className="mx-0.5 h-5 w-px bg-border/70" />
          </>
        ) : null}
        {isRecording ? (
          <>
            <ControlButton
              label={state.isPaused ? 'Resume recording' : 'Pause recording'}
              onClick={() => sendAction(state.isPaused ? 'resume' : 'pause')}
            >
              {state.isPaused ? (
                <Play className="size-4" />
              ) : (
                <Pause className="size-4" />
              )}
            </ControlButton>
            <ControlButton
              label="Stop recording"
              onClick={() => sendAction('stop')}
            >
              <Square className="size-3.5 fill-current text-destructive" />
            </ControlButton>
            <span className="min-w-16 px-1 text-center font-mono text-xs text-foreground tabular-nums">
              {formatElapsedTime(state.elapsedSeconds)}
            </span>
            <div className="mx-0.5 h-5 w-px bg-border/70" />
            {state.cameraLocked ? cameraDropdown : null}
            {microphoneDropdown}
            {systemAudioButton}
            <div className="mx-0.5 h-5 w-px bg-border/70" />
            <ControlButton
              label="Discard recording"
              onClick={() => sendAction('delete')}
            >
              <Trash2 className="size-4" />
            </ControlButton>
          </>
        ) : (
          <>
            <ControlButton
              label="Start recording"
              disabled={state.isStarting}
              className="text-primary hover:text-primary"
              onClick={() => sendAction('start')}
            >
              <Circle className="size-3.5 fill-current" />
            </ControlButton>
            <div className="mx-0.5 h-5 w-px bg-border/70" />
            {cameraDropdown}
            {microphoneDropdown}
            {systemAudioButton}
            {isMac ? iosDeviceDropdown : null}
            <div className="mx-0.5 h-5 w-px bg-border/70" />
            <ControlButton
              label="Close"
              disabled={state.isStarting}
              onClick={() => sendAction('cancel')}
            >
              <X className="size-4" />
            </ControlButton>
          </>
        )}
      </ToolbarSurface>
      {state.countdownSeconds != null ? (
        <CountdownSurface
          seconds={state.countdownSeconds}
          onCancel={() => sendAction('cancel')}
        />
      ) : null}
    </div>
  );
}
