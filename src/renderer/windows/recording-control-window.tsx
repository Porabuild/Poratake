import { useCallback, useEffect, useRef, useState } from 'react';
import { Dropdown, Separator } from '@heroui/react';
import {
  ChevronDown,
  Circle,
  Mic,
  MicOff,
  Pause,
  Play,
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
import type { MediaDeviceDescriptor, MediaDeviceLists } from '@/types/devices';
import type {
  RecordingControlAction,
  RecordingControlDeviceKind,
  RecordingControlState,
} from '@/types/recording-control';

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
  const isOpenRef = useRef(isOpen);
  const wasMountedRef = useRef(false);
  const onExitCompleteRef = useRef(onExitComplete);
  isOpenRef.current = isOpen;
  onExitCompleteRef.current = onExitComplete;

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

  const handlePopoverRef = useCallback((element: HTMLElement | null) => {
    if (element) {
      wasMountedRef.current = true;
      return;
    }

    if (!wasMountedRef.current || isOpenRef.current) return;
    wasMountedRef.current = false;
    onExitCompleteRef.current();
  }, []);

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

export default function RecordingControlWindow({
  params,
}: {
  params: RecordingControlState;
}) {
  const [state, setState] = useState(params);
  const [devices, setDevices] = useState(EMPTY_MEDIA_DEVICES);
  const [openDeviceMenu, setOpenDeviceMenu] =
    useState<RecordingControlDeviceKind | null>(null);
  const openDeviceMenuRef = useRef<RecordingControlDeviceKind | null>(null);
  const isRecording = state.mode === 'recording';

  useEffect(() => {
    window.ipcRenderer.send('recording-control:ready');
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

  const refreshDevices = useCallback(async () => {
    try {
      const availableDevices: MediaDeviceLists =
        await window.ipcRenderer.invoke('recording-control:devices');
      setDevices(availableDevices);
    } catch (error) {
      console.error('Failed to list recording devices:', error);
    }
  }, []);

  const handleDeviceMenuOpenChange = useCallback(
    (kind: RecordingControlDeviceKind, isOpen: boolean) => {
      const nextOpenDeviceMenu = isOpen ? kind : null;
      openDeviceMenuRef.current = nextOpenDeviceMenu;
      setOpenDeviceMenu(nextOpenDeviceMenu);
      if (!isOpen) return;

      window.ipcRenderer.send('recording-control:device-menu-open', true);
      void refreshDevices();
    },
    [refreshDevices]
  );

  const handleDeviceMenuExit = useCallback(() => {
    if (openDeviceMenuRef.current) return;
    window.ipcRenderer.send('recording-control:device-menu-open', false);
  }, []);

  const handleDeviceSelect = useCallback(
    (
      kind: RecordingControlDeviceKind,
      device: MediaDeviceDescriptor | null
    ) => {
      const action = kind === 'microphone' ? 'select-mic' : 'select-camera';
      window.ipcRenderer.send('recording-control:action', action, {
        deviceId: device?.id ?? null,
        deviceName: device?.label ?? null,
      });
    },
    []
  );

  const handleDeviceToggle = useCallback(
    (kind: RecordingControlDeviceKind) => {
      sendAction(kind === 'microphone' ? 'toggle-mic' : 'toggle-camera');
    },
    [sendAction]
  );

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || isRecording || state.isStarting) return;
      sendAction('cancel');
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isRecording, sendAction, state.isStarting]);

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

  return (
    <div className="flex h-screen w-screen items-start justify-center pt-1">
      <ToolbarSurface>
        {state.targetName ? (
          <>
            <span
              className="text-foreground max-w-32 truncate px-1 text-xs"
              title={state.targetName}
            >
              {state.targetName}
            </span>
            <div className="bg-border/70 mx-0.5 h-5 w-px" />
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
              <Square className="text-destructive size-3.5 fill-current" />
            </ControlButton>
            <span className="text-foreground min-w-16 px-1 text-center font-mono text-xs tabular-nums">
              {formatElapsedTime(state.elapsedSeconds)}
            </span>
            <div className="bg-border/70 mx-0.5 h-5 w-px" />
            {state.cameraLocked ? cameraDropdown : null}
            {microphoneDropdown}
            {systemAudioButton}
            <div className="bg-border/70 mx-0.5 h-5 w-px" />
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
            <div className="bg-border/70 mx-0.5 h-5 w-px" />
            {cameraDropdown}
            {microphoneDropdown}
            {systemAudioButton}
            <div className="bg-border/70 mx-0.5 h-5 w-px" />
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
    </div>
  );
}
