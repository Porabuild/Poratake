import type { SettingsItem } from '../settings-registry';

export const STORAGE_ITEMS: SettingsItem[] = [
  {
    id: 'storage.namingPattern',
    category: 'storage',
    section: 'File Naming',
    type: 'naming-pattern',
    label: 'Naming pattern',
    description: 'Customize how files are named using tokens',
    keywords: ['name', 'pattern', 'filename', 'naming', 'tokens'],
  },
  {
    id: 'storage.screenshotsPath',
    category: 'storage',
    section: 'Save Locations',
    type: 'path-picker',
    label: 'Screenshots location',
    description: 'Choose where to save screenshot files',
    keywords: ['path', 'folder', 'directory', 'save location', 'screenshots'],
    pathType: 'screenshots',
  },
  {
    id: 'storage.recordingsPath',
    category: 'storage',
    section: 'Save Locations',
    type: 'path-picker',
    label: 'Recordings location',
    description: 'Choose where to save recording files',
    keywords: [
      'path',
      'folder',
      'directory',
      'save location',
      'recordings',
      'video',
    ],
    pathType: 'recordings',
  },
];
