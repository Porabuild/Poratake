import type { LucideIcon } from 'lucide-react';
import {
  Settings,
  Camera,
  Video,
  Webcam,
  HardDrive,
  Keyboard,
  Cloud,
  KeyRound,
  Info,
  Palette,
} from 'lucide-react';
import type { SettingsConfig } from '@/types/settings';
import type { FeatureId } from '@/types/capabilities';
import { isFeatureSupported } from '@/renderer/utils/capabilities';
import { GENERAL_ITEMS } from './registry/general';
import { SCREENSHOT_ITEMS } from './registry/screenshot';
import { RECORDING_ITEMS } from './registry/recording';
import { DEVICES_ITEMS } from './registry/devices';
import { STORAGE_ITEMS } from './registry/storage';
import { SHORTCUTS_ITEMS } from './registry/shortcuts';
import { CLOUD_ITEMS } from './registry/cloud';
import { APPEARANCE_ITEMS } from './registry/appearance';

export interface SettingsCategory {
  id: string;
  label: string;
  icon: LucideIcon;
  searchable: boolean;
  feature?: FeatureId;
}

interface BaseItem {
  id: string;
  category: string;
  section: string;
  label: string;
  description: string;
  keywords: string[];
  feature?: FeatureId;
}

export interface SwitchItem extends BaseItem {
  type: 'switch';
  getValue: (s: SettingsConfig) => boolean;
  setValue: (s: SettingsConfig, v: boolean) => Partial<SettingsConfig>;
  onBeforeChange?: (
    s: SettingsConfig,
    v: boolean
  ) => Promise<boolean> | boolean;
  disabled?: (s: SettingsConfig) => boolean;
  visibleWhen?: (s: SettingsConfig) => boolean;
}

interface SelectItem extends BaseItem {
  type: 'select';
  options: { value: string; label: string }[];
  getValue: (s: SettingsConfig) => string;
  setValue: (s: SettingsConfig, v: string) => Partial<SettingsConfig>;
  visibleWhen?: (s: SettingsConfig) => boolean;
}

interface SliderItem extends BaseItem {
  type: 'slider';
  min: number;
  max: number;
  step: number;
  getValue: (s: SettingsConfig) => number;
  setValue: (s: SettingsConfig, v: number) => Partial<SettingsConfig>;
  visibleWhen?: (s: SettingsConfig) => boolean;
}

interface ShortcutItem extends BaseItem {
  type: 'shortcut';
  singleKey?: boolean;
  getValue: (s: SettingsConfig) => string;
  setValue: (s: SettingsConfig, v: string) => Partial<SettingsConfig>;
}

interface InputItem extends BaseItem {
  type: 'input';
  placeholder?: string;
  inputType?: 'text' | 'password';
  hint?: string;
  getValue: (s: SettingsConfig) => string;
  setValue: (s: SettingsConfig, v: string) => Partial<SettingsConfig>;
  visibleWhen?: (s: SettingsConfig) => boolean;
}

interface PathPickerItem extends BaseItem {
  type: 'path-picker';
  pathType: 'screenshots' | 'recordings';
}

interface NamingPatternItem extends BaseItem {
  type: 'naming-pattern';
}

interface CloudTestConnectionItem extends BaseItem {
  type: 'cloud-test-connection';
  visibleWhen?: (s: SettingsConfig) => boolean;
}

interface CaptyCloudAccessItem extends BaseItem {
  type: 'capty-cloud-access';
  visibleWhen?: (s: SettingsConfig) => boolean;
}

interface RestHeadersItem extends BaseItem {
  type: 'rest-headers';
  visibleWhen?: (s: SettingsConfig) => boolean;
}

interface MicrophoneDeviceItem extends BaseItem {
  type: 'microphone-device';
}

interface CameraDeviceItem extends BaseItem {
  type: 'camera-device';
}

export type SettingsItem =
  | SwitchItem
  | SelectItem
  | SliderItem
  | ShortcutItem
  | InputItem
  | PathPickerItem
  | NamingPatternItem
  | CaptyCloudAccessItem
  | CloudTestConnectionItem
  | RestHeadersItem
  | MicrophoneDeviceItem
  | CameraDeviceItem;

const ALL_SETTINGS_CATEGORIES: SettingsCategory[] = [
  {
    id: 'general',
    label: 'General',
    icon: Settings,
    searchable: true,
  },
  {
    id: 'appearance',
    label: 'Appearance',
    icon: Palette,
    searchable: true,
  },
  {
    id: 'screenshot',
    label: 'Screenshot',
    icon: Camera,
    searchable: true,
  },
  {
    id: 'recording',
    label: 'Recording',
    icon: Video,
    searchable: true,
    feature: 'recording',
  },
  {
    id: 'devices',
    label: 'Devices',
    icon: Webcam,
    searchable: true,
    feature: 'recording',
  },
  {
    id: 'storage',
    label: 'Storage',
    icon: HardDrive,
    searchable: true,
  },
  {
    id: 'shortcuts',
    label: 'Shortcuts',
    icon: Keyboard,
    searchable: true,
  },
  {
    id: 'cloud',
    label: 'Cloud',
    icon: Cloud,
    searchable: true,
  },
  {
    id: 'license',
    label: 'License',
    icon: KeyRound,
    searchable: false,
  },
  {
    id: 'about',
    label: 'About',
    icon: Info,
    searchable: false,
  },
];

export const SETTINGS_CATEGORIES: SettingsCategory[] =
  ALL_SETTINGS_CATEGORIES.filter(
    category => !category.feature || isFeatureSupported(category.feature)
  );

const SUPPORTED_CATEGORY_IDS = new Set(SETTINGS_CATEGORIES.map(c => c.id));

export const SEARCHABLE_CATEGORIES = SETTINGS_CATEGORIES.filter(
  c => c.searchable
);
export const SPECIAL_CATEGORIES = SETTINGS_CATEGORIES.filter(
  c => !c.searchable
);

export const SETTINGS_ITEMS: SettingsItem[] = [
  ...GENERAL_ITEMS,
  ...APPEARANCE_ITEMS,
  ...SCREENSHOT_ITEMS,
  ...RECORDING_ITEMS,
  ...DEVICES_ITEMS,
  ...STORAGE_ITEMS,
  ...SHORTCUTS_ITEMS,
  ...CLOUD_ITEMS,
].filter(
  item =>
    SUPPORTED_CATEGORY_IDS.has(item.category) &&
    (!item.feature || isFeatureSupported(item.feature))
);

export function matchesSettingsQuery(
  item: SettingsItem,
  query: string
): boolean {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (terms.length === 0) return true;

  const searchable = [
    item.label,
    item.description,
    item.section,
    ...item.keywords,
  ]
    .join(' ')
    .toLowerCase();

  return terms.every(term => searchable.includes(term));
}

export function searchSettings(query: string): SettingsItem[] {
  if (!query.trim()) return [];

  return SETTINGS_ITEMS.filter(item => matchesSettingsQuery(item, query));
}

export function getItemsByCategory(category: string): SettingsItem[] {
  return SETTINGS_ITEMS.filter(item => item.category === category);
}

export function groupBySection(
  items: SettingsItem[]
): Map<string, SettingsItem[]> {
  const map = new Map<string, SettingsItem[]>();
  for (const item of items) {
    const existing = map.get(item.section) ?? [];
    existing.push(item);
    map.set(item.section, existing);
  }
  return map;
}
