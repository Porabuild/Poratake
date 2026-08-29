import type { SettingsItem } from '../settings-registry';

export const RECORDING_ITEMS: SettingsItem[] = [
  {
    id: 'recording.frameRate',
    category: 'recording',
    section: 'Quality',
    type: 'select',
    label: 'Frame rate',
    description: 'Record up to the selected display refresh rate',
    keywords: ['fps', 'frame', 'rate', 'refresh', 'smooth', 'recording'],
    options: [30, 60, 90, 120, 144, 165, 240].map(value => ({
      value: String(value),
      label: `${value} FPS`,
    })),
    getValue: s => String(s.recording.frameRate),
    setValue: (s, v) => ({
      recording: { ...s.recording, frameRate: Number(v) },
    }),
  },
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
