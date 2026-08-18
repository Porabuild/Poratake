// @vitest-environment happy-dom
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_SETTINGS } from '@/types/settings';
import type { SettingsConfig } from '@/types/settings';

const mocks = vi.hoisted(() => ({
  onUpdate: vi.fn(),
  send: vi.fn(),
  invoke: vi.fn(),
}));

vi.mock('@/renderer/components/settings/devices/device-select', async () => {
  const { createElement: createMockElement } = await import('react');
  return {
    default: ({ onSelect }: { onSelect: (device: unknown) => void }) =>
      createMockElement(
        'button',
        {
          'aria-label': 'Select mock camera',
          onClick: () => onSelect({ id: 'camera-b', label: 'Camera B' }),
        },
        'Select camera'
      ),
  };
});

vi.mock('@/renderer/components/ui/switch', async () => {
  const { createElement: createMockElement } = await import('react');
  return {
    Switch: ({
      onCheckedChange,
    }: {
      onCheckedChange: (value: boolean) => void;
    }) =>
      createMockElement(
        'button',
        { 'aria-label': 'Toggle mirror', onClick: () => onCheckedChange(true) },
        'Mirror'
      ),
  };
});

vi.mock('@/renderer/components/ui/button', async () => {
  const { createElement: createMockElement } = await import('react');
  return {
    Button: ({ children, ...props }: React.ComponentProps<'button'>) =>
      createMockElement('button', props, children),
  };
});

vi.mock('@/renderer/components/ui/label', async () => {
  const { createElement: createMockElement } = await import('react');
  return {
    Label: ({ children, ...props }: React.ComponentProps<'label'>) =>
      createMockElement('label', props, children),
  };
});

vi.mock('@/renderer/hooks/use-media-devices', () => ({
  useMediaDevices: () => ({
    cameras: [],
    defaultCameraId: null,
    refresh: vi.fn(),
  }),
  useDeviceTest: () => ({
    testing: false,
    startTest: vi.fn(),
    stopTest: vi.fn(),
  }),
}));

vi.mock('@/renderer/components/area-overlay/toolbar-button', async () => {
  const { createElement: createMockElement } = await import('react');
  return {
    default: ({ children, ...props }: React.ComponentProps<'button'>) =>
      createMockElement('button', props, children),
  };
});

vi.mock('@/renderer/components/area-overlay/toolbar-surface', async () => {
  const { createElement: createMockElement, forwardRef } =
    await import('react');
  return {
    default: forwardRef<HTMLDivElement, React.ComponentProps<'div'>>(
      function ToolbarSurface({ children, ...props }, ref) {
        return createMockElement('div', { ...props, ref }, children);
      }
    ),
  };
});

vi.mock('@heroui/react', async () => {
  const React = await import('react');
  const DropdownContext = React.createContext<{
    isOpen: boolean;
    onOpenChange: (isOpen: boolean) => void;
  } | null>(null);

  function DropdownRoot({
    isOpen,
    onOpenChange,
    children,
  }: {
    isOpen: boolean;
    onOpenChange: (isOpen: boolean) => void;
    children: React.ReactNode;
  }) {
    return React.createElement(
      DropdownContext.Provider,
      { value: { isOpen, onOpenChange } },
      children
    );
  }

  function Trigger({
    children,
    isDisabled,
    ...props
  }: React.ComponentProps<'button'> & { isDisabled?: boolean }) {
    const dropdown = React.useContext(DropdownContext);
    return React.createElement(
      'button',
      {
        ...props,
        disabled: isDisabled,
        onClick: () => dropdown?.onOpenChange(!dropdown.isOpen),
      },
      children
    );
  }

  const Popover = React.forwardRef<
    HTMLDivElement,
    React.ComponentProps<'div'> & { placement?: string }
  >(function Popover({ children, placement: _placement, ...props }, ref) {
    const dropdown = React.useContext(DropdownContext);
    React.useLayoutEffect(() => {
      if (!dropdown?.isOpen) return;

      const element = document.createElement('div');
      if (typeof ref === 'function') {
        ref(element);
      } else if (ref) {
        ref.current = element;
      }

      return () => {
        if (typeof ref === 'function') {
          ref(null);
        } else if (ref) {
          ref.current = null;
        }
      };
    }, [dropdown?.isOpen, ref]);

    if (!dropdown?.isOpen) return null;
    return React.createElement('div', props, children);
  });

  const Container = ({ children }: { children?: React.ReactNode }) =>
    React.createElement('div', null, children);

  const Dropdown = Object.assign(DropdownRoot, {
    Trigger,
    Popover,
    Menu: Container,
    Item: Container,
    ItemIndicator: Container,
  });

  return {
    Dropdown,
    Separator: () => React.createElement('hr'),
  };
});

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  vi.clearAllMocks();
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
  window.appPlatform = 'win32';
  window.ipcRenderer = {
    send: mocks.send,
    invoke: mocks.invoke.mockResolvedValue({
      microphones: [],
      cameras: [],
      defaultMicrophoneId: null,
      defaultCameraId: null,
    }),
    on: vi.fn(),
    off: vi.fn(),
  } as never;
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      disconnect() {}
    }
  );
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  vi.unstubAllGlobals();
});

describe('React ref commit timing', () => {
  it('uses committed camera props and preserves rapid camera actions', async () => {
    const { default: CameraDeviceSetting } =
      await import('@/renderer/components/settings/devices/camera-device-setting');
    const createSettings = (id: string, label: string): SettingsConfig => ({
      ...DEFAULT_SETTINGS,
      recording: {
        ...DEFAULT_SETTINGS.recording,
        camera: {
          ...DEFAULT_SETTINGS.recording.camera,
          selectedDeviceId: id,
          selectedDeviceName: label,
          flipped: false,
        },
      },
    });

    await act(async () => {
      root.render(
        createElement(CameraDeviceSetting, {
          settings: createSettings('camera-a', 'Camera A'),
          onUpdate: mocks.onUpdate,
        })
      );
    });
    await act(async () => {
      root.render(
        createElement(CameraDeviceSetting, {
          settings: createSettings('camera-c', 'Camera C'),
          onUpdate: mocks.onUpdate,
        })
      );
    });

    const mirror = container.querySelector<HTMLButtonElement>(
      '[aria-label="Toggle mirror"]'
    );
    const select = container.querySelector<HTMLButtonElement>(
      '[aria-label="Select mock camera"]'
    );
    expect(mirror).not.toBeNull();
    expect(select).not.toBeNull();

    act(() => mirror?.click());
    expect(mocks.onUpdate).toHaveBeenLastCalledWith(
      expect.objectContaining({
        recording: expect.objectContaining({
          camera: expect.objectContaining({
            selectedDeviceId: 'camera-c',
            flipped: true,
          }),
        }),
      })
    );

    act(() => {
      select?.click();
      mirror?.click();
    });
    expect(mocks.onUpdate).toHaveBeenLastCalledWith(
      expect.objectContaining({
        recording: expect.objectContaining({
          camera: expect.objectContaining({
            selectedDeviceId: 'camera-b',
            selectedDeviceName: 'Camera B',
            flipped: true,
          }),
        }),
      })
    );
  });

  it('reports a closed device menu after the popover ref detaches', async () => {
    const { default: RecordingControlWindow } =
      await import('@/renderer/windows/recording-control-window');
    const params = {
      mode: 'pre-recording' as const,
      targetName: null,
      systemAudio: true,
      micEnabled: false,
      micMuted: false,
      selectedMicId: null,
      cameraEnabled: false,
      selectedCameraId: null,
      selectedIOSDeviceId: null,
      selectedIOSDeviceName: null,
      cameraLocked: false,
      isPaused: false,
      isStarting: false,
      elapsedSeconds: 0,
    };

    await act(async () => {
      root.render(createElement(RecordingControlWindow, { params }));
    });

    const getCameraTrigger = () =>
      container.querySelector<HTMLButtonElement>(
        '[aria-label="Select camera"]'
      );
    expect(getCameraTrigger()).not.toBeNull();

    await act(async () => getCameraTrigger()?.click());
    expect(mocks.send).toHaveBeenCalledWith(
      'recording-control:device-menu-open',
      true
    );

    await act(async () => getCameraTrigger()?.click());
    expect(mocks.send).toHaveBeenCalledWith(
      'recording-control:device-menu-open',
      false
    );
  });
});
