import type { SettingsItem } from '../settings-registry';

export const DEVICES_ITEMS: SettingsItem[] = [
  {
    id: 'devices.microphone',
    category: 'devices',
    section: 'Microphone',
    type: 'microphone-device',
    label: 'Microphone',
    description: 'Choose which microphone is used for recordings',
    keywords: [
      'microphone',
      'mic',
      'audio',
      'input',
      'device',
      'test',
      'voice',
    ],
    feature: 'recording',
  },
  {
    id: 'devices.camera',
    category: 'devices',
    section: 'Camera',
    type: 'camera-device',
    label: 'Camera',
    description: 'Choose which camera is used for recordings',
    keywords: [
      'camera',
      'webcam',
      'video',
      'device',
      'test',
      'preview',
      'mirror',
      'flip',
    ],
    feature: 'recording',
  },
];
