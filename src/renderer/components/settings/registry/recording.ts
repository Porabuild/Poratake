import type { SettingsItem } from '../settings-registry';

export const RECORDING_ITEMS: SettingsItem[] = [
  {
    id: 'recording.showPreview',
    category: 'recording',
    section: 'Behavior',
    type: 'switch',
    label: 'Show recording preview',
    description: 'Show a preview window after recording',
    keywords: ['preview', 'recording', 'video', 'record'],
    getValue: s => s.recording.showPreview,
    setValue: (s, v) => ({
      recording: { ...s.recording, showPreview: v },
    }),
  },
  {
    id: 'recording.startDelay',
    category: 'recording',
    section: 'Behavior',
    type: 'slider',
    min: 0,
    max: 10,
    step: 1,
    label: 'Start delay',
    description: 'Countdown shown before recording starts (seconds)',
    keywords: ['delay', 'countdown', 'timer', 'start', 'recording'],
    getValue: s => s.recording.startDelay,
    setValue: (s, v) => ({
      recording: { ...s.recording, startDelay: v },
    }),
  },
  {
    id: 'recording.autoZoom',
    category: 'recording',
    section: 'Behavior',
    type: 'switch',
    label: 'Auto zoom after recording',
    description:
      'Automatically generate zoom segments based on cursor clicks for new recordings',
    keywords: ['zoom', 'auto', 'recording', 'cursor', 'clicks'],
    getValue: s => s.recording.autoZoom,
    setValue: (s, v) => ({
      recording: { ...s.recording, autoZoom: v },
    }),
  },
];
