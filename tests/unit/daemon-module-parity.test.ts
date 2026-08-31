import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { DAEMON_METHODS } from '@/types/daemon';

interface ContractModule {
  platforms: string[];
  shared: string[];
  linux: string[];
  events?: Record<string, string[]>;
  methodRequests?: Record<string, string>;
  requestSchemas?: Record<string, Record<string, unknown>>;
  errors?: string[];
  imageRequest?: Record<string, unknown>;
  showRequest?: Record<string, unknown>;
}

interface NeutralContract {
  geometry: Record<string, Record<string, number>>;
  records: Record<string, Record<string, unknown>>;
  modules: Record<string, ContractModule>;
}

function contract(): NeutralContract {
  return JSON.parse(
    fs.readFileSync(
      path.join(process.cwd(), 'src/types/daemon-contract.json'),
      'utf8'
    )
  ) as NeutralContract;
}

function sourceFiles(root: string, extension: string): string[] {
  return fs
    .readdirSync(root, { withFileTypes: true })
    .filter(entry => entry.isFile() && entry.name.endsWith(extension))
    .map(entry => path.join(root, entry.name));
}

describe('native daemon contract', () => {
  it('keeps every generated artifact current', () => {
    const result = spawnSync(
      'bun',
      ['scripts/generate-daemon-contract.mjs', '--check'],
      {
        cwd: process.cwd(),
        encoding: 'utf8',
        shell: process.platform === 'win32',
      }
    );

    expect(result.status).toBe(0);
  });

  it('uses the neutral schema for shared methods and platform modules', () => {
    const value = contract();

    expect(Object.keys(value.modules).toSorted()).toEqual(
      Object.keys(DAEMON_METHODS).toSorted()
    );
    for (const [module, methods] of Object.entries(DAEMON_METHODS)) {
      expect([...methods]).toEqual(value.modules[module].shared);
    }

    expect(
      Object.entries(value.modules)
        .filter(([, module]) => module.platforms.includes('macos'))
        .map(([name]) => name)
        .toSorted()
    ).toEqual(
      Object.entries(value.modules)
        .filter(([, module]) => module.platforms.includes('windows'))
        .map(([name]) => name)
        .toSorted()
    );
    expect(
      Object.entries(value.modules)
        .filter(([, module]) => module.platforms.includes('linux'))
        .map(([name]) => name)
        .toSorted()
    ).toEqual([
      'desktop-wallpaper',
      'freeze-screen',
      'print',
      'qrcode',
      'screen-recorder',
      'screenshot',
      'scroll-capture',
      'timer-control',
      'window-selector',
    ]);
  });

  it('keeps capture and recorder request invariants language-neutral', () => {
    const value = contract();

    expect(value.records.displayCaptureContext).toEqual({
      fields: [
        'x',
        'y',
        'width',
        'height',
        'scaleFactor',
        'displayOriginX',
        'displayOriginY',
        'displayId',
      ],
      required: ['x', 'y', 'width', 'height'],
      types: {
        x: 'integer',
        y: 'integer',
        width: 'positive-integer',
        height: 'positive-integer',
        scaleFactor: 'number-0.25-through-8',
        displayOriginX: 'integer',
        displayOriginY: 'integer',
        displayId: 'optional-unsigned-integer',
      },
      defaults: {
        scaleFactor: 1,
        displayOriginX: 0,
        displayOriginY: 0,
      },
    });

    expect(value.modules.screenshot.methodRequests).toEqual({
      'capture-area': 'captureArea',
      'capture-window': 'captureWindow',
    });
    expect(value.modules.screenshot.requestSchemas?.captureArea).toMatchObject({
      captureContext: 'displayCaptureContext',
      fields: ['path', 'cached', 'windowId'],
      required: ['path'],
    });
    expect(value.modules.screenshot.requestSchemas?.captureWindow).toEqual({
      fields: ['windowId', 'path'],
      required: ['windowId', 'path'],
      types: { windowId: 'integer', path: 'path' },
    });

    const recorder = value.modules['screen-recorder'];
    expect(recorder.methodRequests).toEqual({
      start: 'start',
      setMicrophone: 'microphone',
      setSystemAudio: 'toggle',
      setCamera: 'toggle',
    });
    expect(recorder.requestSchemas?.start).toMatchObject({
      required: ['outputPath'],
      invariants: [
        'x-y-width-height-all-or-none',
        'width-height-positive-when-present',
      ],
    });
    expect(recorder.requestSchemas?.microphone).toMatchObject({
      fields: ['enabled', 'deviceId', 'deviceName'],
      required: ['enabled'],
      types: {
        enabled: 'boolean',
        deviceId: 'nullable-string',
        deviceName: 'nullable-string',
      },
    });
  });

  it('keeps shared scroll and timer behavior in the schema', () => {
    const value = contract();
    const scroll = value.modules['scroll-capture'];

    expect(scroll.requestSchemas?.start).toMatchObject({
      captureContext: 'displayCaptureContext',
      platformDefaults: {
        macos: { nativeControls: false },
        windows: { nativeControls: true },
        linux: { nativeControls: false },
      },
    });
    expect(scroll.events).toEqual({
      shared: ['scroll-capture:done', 'scroll-capture:cancelled'],
      macos: [
        'scroll-capture:started',
        'scroll-capture:frame',
        'scroll-capture:auto-scroll',
        'scroll-capture:cursor',
        'scroll-capture:scroll-ended',
      ],
      windows: ['scroll-capture:frame-captured', 'scroll-capture:scroll-ended'],
      linux: ['scroll-capture:frame-captured', 'scroll-capture:scroll-ended'],
    });
    expect(value.geometry.timerControl).toEqual({
      width: 140,
      height: 52,
      topMargin: 20,
    });
    expect(value.modules['timer-control'].showRequest).toMatchObject({
      required: ['x', 'y', 'duration', 'color', 'foregroundColor'],
      coordinateSpace: 'platform-screen',
      colors: 'six-digit-hex',
    });
  });

  it('keeps native dispatch free of handwritten method switches', () => {
    const rustModules = [
      ...sourceFiles(
        path.join(process.cwd(), 'src/main/daemon-win/src/modules'),
        '.rs'
      ),
      ...sourceFiles(
        path.join(process.cwd(), 'src/main/daemon-linux/src'),
        '.rs'
      ),
    ];
    for (const file of rustModules) {
      expect(fs.readFileSync(file, 'utf8')).not.toContain(
        'match request.method.as_str()'
      );
    }

    const swiftModules = sourceFiles(
      path.join(process.cwd(), 'src/main/daemon/Modules'),
      '.swift'
    );
    for (const file of swiftModules) {
      const source = fs.readFileSync(file, 'utf8');
      expect(source).not.toMatch(/^\s*case\s+"/m);
      expect(source).toMatch(/DaemonContract\.\w+\.Method\(rawValue: method\)/);
      expect(source).not.toMatch(/^\s*default:\s*$/m);
    }
  });

  it('round-trips macOS top-left display coordinates', () => {
    const primaryHeight = 1080;
    const screens = [
      { id: 1, x: 0, y: 0, width: 1920, height: 1080 },
      { id: 2, x: 1920, y: 180, width: 1440, height: 900 },
      { id: 3, x: -1280, y: 56, width: 1280, height: 1024 },
      { id: 4, x: 0, y: 1080, width: 1920, height: 1080 },
      { id: 5, x: 0, y: -900, width: 1440, height: 900 },
    ];
    const topLeftScreens = screens.map(screen => ({
      ...screen,
      topLeftY: primaryHeight - (screen.y + screen.height),
    }));

    for (const screen of topLeftScreens) {
      const selected = topLeftScreens.find(
        candidate =>
          candidate.x === screen.x && candidate.topLeftY === screen.topLeftY
      );
      expect(selected?.id).toBe(screen.id);
      expect(primaryHeight - screen.topLeftY - screen.height).toBe(screen.y);
    }
  });
});
