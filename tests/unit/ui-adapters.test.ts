import React from 'react';
import type * as ReactModule from 'react';
import { describe, expect, it, vi } from 'vitest';
import type { SettingsConfig } from '@/types/settings';
import { DEFAULT_SETTINGS } from '@/types/settings';

const mocks = vi.hoisted(() => ({
  tooltip: vi.fn(),
  tooltipTrigger: vi.fn(),
  tooltipContent: vi.fn(),
  separator: vi.fn(),
  separatorVariants: vi.fn(() => 'separator'),
  button: vi.fn(),
  label: vi.fn(),
  switch: vi.fn(),
  deviceSelect: vi.fn(),
  startTest: vi.fn(),
  stopTest: vi.fn(),
  onUpdate: vi.fn(),
}));

vi.mock('react', async importOriginal => {
  const actual = await importOriginal<typeof ReactModule>();
  return {
    ...actual,
    useCallback: (callback: unknown) => callback,
    useRef: (initial: unknown) => ({ current: initial }),
  };
});

vi.mock('@heroui/react', () => ({
  Tooltip: Object.assign(mocks.tooltip, {
    Trigger: mocks.tooltipTrigger,
    Content: mocks.tooltipContent,
  }),
  Separator: mocks.separator,
  separatorVariants: mocks.separatorVariants,
}));

vi.mock('@/renderer/components/ui/button', () => ({
  Button: mocks.button,
}));

vi.mock('@/renderer/components/ui/label', () => ({
  Label: mocks.label,
}));

vi.mock('@/renderer/components/ui/switch', () => ({
  Switch: mocks.switch,
}));

vi.mock('@/renderer/components/settings/devices/device-select', () => ({
  default: mocks.deviceSelect,
}));

vi.mock('@/renderer/hooks/use-media-devices', () => ({
  useMediaDevices: () => ({
    cameras: [],
    defaultCameraId: null,
    refresh: vi.fn(),
  }),
  useDeviceTest: () => ({
    testing: false,
    startTest: mocks.startTest,
    stopTest: mocks.stopTest,
  }),
}));

function findElement(
  node: React.ReactNode,
  type: React.ElementType
): React.ReactElement<Record<string, unknown>> | null {
  if (!React.isValidElement(node)) return null;
  if (node.type === type) {
    return node as React.ReactElement<Record<string, unknown>>;
  }
  const children = (node.props as { children?: React.ReactNode }).children;
  for (const child of React.Children.toArray(children)) {
    const match = findElement(child, type);
    if (match) return match;
  }
  return null;
}

describe('renderer UI adapters', () => {
  it('composes tooltip trigger behavior into its child', async () => {
    const { TooltipTrigger } = await import('@/renderer/components/ui/tooltip');
    const childRef = { current: null as HTMLElement | null };
    const triggerRef = { current: null as HTMLElement | null };
    const onClick = vi.fn();
    const childFocus = vi.fn();
    const triggerFocus = vi.fn();
    const child = React.createElement(
      'button',
      {
        'aria-label': 'Capture',
        className: 'child',
        onClick,
        onFocus: childFocus,
        ref: childRef,
      },
      'Capture icon'
    );
    const trigger = TooltipTrigger({
      asChild: true,
      children: child,
    }) as React.ReactElement<{
      render: (props: React.HTMLAttributes<HTMLElement>) => React.ReactElement;
    }>;
    const rendered = trigger.props.render({
      className: 'trigger',
      onFocus: triggerFocus,
      role: 'button',
      tabIndex: 0,
      ref: triggerRef,
      children: undefined,
    } as React.HTMLAttributes<HTMLElement>);
    const element = {} as HTMLElement;
    const ref = rendered.props.ref as React.RefCallback<HTMLElement>;
    ref(element);
    rendered.props.onFocus({});

    expect(rendered.type).toBe('button');
    expect(rendered.props['aria-label']).toBe('Capture');
    expect(rendered.props.className).toBe('trigger child');
    expect(rendered.props.children).toBe('Capture icon');
    expect(rendered.props.onClick).toBe(onClick);
    expect(childFocus).toHaveBeenCalledOnce();
    expect(triggerFocus).toHaveBeenCalledOnce();
    expect(childRef.current).toBe(element);
    expect(triggerRef.current).toBe(element);
  });

  it('keeps visual separators out of the accessibility tree', async () => {
    const { Separator } = await import('@/renderer/components/ui/separator');
    const decorative = Separator({});
    const semantic = Separator({ decorative: false });

    expect(decorative.props.role).toBe('presentation');
    expect(decorative.props['aria-hidden']).toBe(true);
    expect(mocks.separatorVariants).toHaveBeenCalledOnce();
    expect(semantic.props.role).toBeUndefined();
    expect(semantic.props['aria-hidden']).toBeUndefined();
  });

  it('preserves a rapid camera selection when mirror changes', async () => {
    vi.stubGlobal('React', React);
    const { default: CameraDeviceSetting } =
      await import('@/renderer/components/settings/devices/camera-device-setting');
    const settings: SettingsConfig = {
      ...DEFAULT_SETTINGS,
      recording: {
        ...DEFAULT_SETTINGS.recording,
        camera: {
          ...DEFAULT_SETTINGS.recording.camera,
          selectedDeviceId: 'camera-a',
          selectedDeviceName: 'Camera A',
          flipped: false,
        },
      },
    };
    const tree = CameraDeviceSetting({
      settings,
      onUpdate: mocks.onUpdate,
    });
    const select = findElement(tree, mocks.deviceSelect);
    const mirror = findElement(tree, mocks.switch);

    expect(select).not.toBeNull();
    expect(mirror).not.toBeNull();
    (select!.props.onSelect as (device: { id: string; label: string }) => void)(
      {
        id: 'camera-b',
        label: 'Camera B',
      }
    );
    (mirror!.props.onCheckedChange as (checked: boolean) => void)(true);

    const lastUpdate = mocks.onUpdate.mock.calls[1][0] as SettingsConfig;
    expect(lastUpdate.recording.camera).toMatchObject({
      selectedDeviceId: 'camera-b',
      selectedDeviceName: 'Camera B',
      flipped: true,
    });
  });
});
