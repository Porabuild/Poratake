import { useCallback, useEffect, useState } from 'react';
import {
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
import type {
  RecordingControlAction,
  RecordingControlState,
} from '@/types/recording-control';

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

export default function RecordingControlWindow({
  params,
}: {
  params: RecordingControlState;
}) {
  const [state, setState] = useState(params);
  const isRecording = state.mode === 'recording';
  const micActive = state.micEnabled && !state.micMuted;

  useEffect(() => {
    document.body.classList.add('window-transparent');
    window.ipcRenderer.send('recording-control:ready');
    return () => document.body.classList.remove('window-transparent');
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

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || isRecording || state.isStarting) return;
      sendAction('cancel');
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isRecording, sendAction, state.isStarting]);

  return (
    <div className="flex h-screen w-screen items-center justify-center">
      <ToolbarSurface>
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
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90 hover:text-destructive-foreground"
              onClick={() => sendAction('stop')}
            >
              <Square className="size-3.5 fill-current" />
            </ControlButton>
            <span className="text-foreground min-w-16 px-1 text-center font-mono text-xs tabular-nums">
              {formatElapsedTime(state.elapsedSeconds)}
            </span>
            {state.micEnabled ? (
              <ControlButton
                label={micActive ? 'Mute microphone' : 'Unmute microphone'}
                aria-pressed={!micActive}
                onClick={() => sendAction('toggle-mic-mute')}
              >
                {micActive ? (
                  <Mic className="size-4" />
                ) : (
                  <MicOff className="size-4" />
                )}
              </ControlButton>
            ) : null}
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
              className="bg-primary text-primary-foreground hover:bg-primary/90 hover:text-primary-foreground"
              onClick={() => sendAction('start')}
            >
              <Circle className="size-3.5 fill-current" />
            </ControlButton>
            <div className="bg-border/70 mx-0.5 h-5 w-px" />
            <ControlButton
              label={state.cameraEnabled ? 'Turn camera off' : 'Turn camera on'}
              disabled={state.isStarting}
              aria-pressed={state.cameraEnabled}
              onClick={() => sendAction('toggle-camera')}
            >
              {state.cameraEnabled ? (
                <Video className="size-4" />
              ) : (
                <VideoOff className="size-4" />
              )}
            </ControlButton>
            <ControlButton
              label={
                state.micEnabled ? 'Turn microphone off' : 'Turn microphone on'
              }
              disabled={state.isStarting}
              aria-pressed={state.micEnabled}
              onClick={() => sendAction('toggle-mic')}
            >
              {state.micEnabled ? (
                <Mic className="size-4" />
              ) : (
                <MicOff className="size-4" />
              )}
            </ControlButton>
            <ControlButton
              label={
                state.systemAudio
                  ? 'Turn system sounds off'
                  : 'Turn system sounds on'
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
