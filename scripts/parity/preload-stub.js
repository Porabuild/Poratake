// Stands in for `src/preload/preload.ts` so the Electron renderer can be drawn
// in an ordinary browser and compared against the GPUI shell, pixel for pixel.
//
// Loaded by `vite.renderer.mts` before the renderer entrypoint.
//
const query = new URLSearchParams(location.search);
if (query.has('traceMedia')) {
  window.parityMediaEvents = [];
  for (const type of ['loadeddata', 'error']) {
    document.addEventListener(
      type,
      event => {
        if (event.target instanceof HTMLVideoElement) {
          window.parityMediaEvents.push({
            type,
            readyState: event.target.readyState,
          });
        }
      },
      true
    );
  }
}
window.appPlatform = query.get('platform') || 'win32';

const APPEARANCE = {
  mode: query.get('theme') === 'light' ? 'light' : 'dark',
  theme: 'default',
};
const SHORTCUTS = undefined; // undefined => the renderer's own defaults

const TYPE = query.get('window') || 'settings';
/// `?image=/@fs/C:/path/to/capture.png` for the windows that show one.
const IMAGE = query.get('image');
const FILE = query.has('file') ? query.get('file') : 'C:\\parity.mp4';
const BLACKOUT_ORDER_FIXTURE = query.get('fixture') === 'blackout-order';
const BLACKOUT_SCALE_FIXTURE =
  query.get('fixture') === 'blackout-scale' || BLACKOUT_ORDER_FIXTURE;
const BLACKOUT_REOPEN_FIXTURE = query.get('fixture') === 'blackout-reopen';
const ORDER_SHAPE = {
  id: 'parity-shape',
  type: 'rectangle',
  x: 10,
  y: 6,
  width: 8,
  height: 8,
  stroke: '#ff0000',
  strokeWidth: 1,
  fill: '#ff0000',
};
const isList = channel => /list$|:list/.test(channel);

const windowParams = async () => {
  switch (TYPE) {
    case 'screenshot':
      return {
        filePath: FILE,
        imageUrl: IMAGE,
        ...(BLACKOUT_SCALE_FIXTURE || BLACKOUT_REOPEN_FIXTURE
          ? {
              width: Number(query.get('displayWidth') || 100),
              height: Number(query.get('displayHeight') || 80),
              ...(BLACKOUT_SCALE_FIXTURE
                ? {
                    editorState: {
                      wallpaper: {
                        aspectRatio: query.get('aspectRatio') || 'auto',
                        inset: Number(query.get('inset') || 0),
                        windowFrame: { style: query.get('frame') || 'none' },
                        noise: Number(query.get('noise') || 0),
                        gradient: query.has('noise')
                          ? {
                              id: 'parity-noise',
                              colors: ['#5080b0', '#5080b0'],
                              angle: 0,
                            }
                          : null,
                      },
                      layers: query.has('attachment')
                        ? [
                            {
                              id: 'parity-attachment',
                              base64: query.get('attachment'),
                              naturalWidth: 40,
                              naturalHeight: 22,
                              edge: query.get('attachmentEdge') || 'right',
                            },
                          ]
                        : [],
                      annotations: [
                        ...(BLACKOUT_ORDER_FIXTURE &&
                        query.get('shapeAfter') !== '1'
                          ? [
                              query.has('brokenSvg')
                                ? { ...ORDER_SHAPE, fill: '#ff0000" invalid<' }
                                : ORDER_SHAPE,
                            ]
                          : []),
                        {
                          id: 'parity-blackout',
                          type: 'redact',
                          x: 8.8,
                          y: 3.8,
                          width: 12.3,
                          height: 12.3,
                          style: query.get('redactStyle') || 'blackout',
                          intensity: 5,
                        },
                        ...(BLACKOUT_ORDER_FIXTURE &&
                        query.get('shapeAfter') === '1'
                          ? [
                              query.has('brokenSvg')
                                ? { ...ORDER_SHAPE, fill: '#ff0000" invalid<' }
                                : ORDER_SHAPE,
                            ]
                          : []),
                      ],
                    },
                  }
                : {}),
            }
          : {}),
        initialPreferences: { lastTool: 'pen', color: '#f43f5e' },
        screenshotSettings: {
          closeOnCopy: false,
          closeOnSave: false,
          format: 'png',
        },
      };
    case 'video-editor':
      return { filePath: FILE };
    case 'capture-preview':
      return {
        filePath: FILE,
        contentType:
          query.get('contentType') === 'video' ? 'video' : 'screenshot',
        imageUrl: IMAGE,
      };
    case 'recording-control':
      return {
        mode: 'pre-recording',
        targetName: null,
        systemAudio: true,
        micEnabled: false,
        selectedMicId: null,
        cameraEnabled: false,
        selectedCameraId: null,
        cameraLocked: false,
        selectedIOSDeviceId: null,
        selectedIOSDeviceName: null,
        isPaused: false,
        isStarting: false,
        elapsedSeconds: 0,
      };
    case 'area-overlay':
      return {
        sessionId: 1,
        displayId: 1,
        imageUrl: IMAGE,
        interactive: true,
        autoConfirm: false,
        repeatablePicks: false,
        showPrompt: query.get('intent') !== 'timer',
        rect:
          query.get('selected') === '1'
            ? { x: 90, y: 120, width: 520, height: 300 }
            : null,
        aspectRatio: null,
        toolbar: query.has('toolbar')
          ? {
              kind: 'all-in-one',
              recordingEnabled: true,
              ocrEnabled: true,
              activeMode: ['record', 'ocr'].includes(query.get('toolbar'))
                ? query.get('toolbar')
                : 'screenshot',
              activeTarget: ['window', 'screen'].includes(query.get('target'))
                ? query.get('target')
                : 'area',
            }
          : null,
        pickTargets: null,
        prompt: null,
      };
    default:
      return {};
  }
};

// A few windows wait for a push from the main process before they render
// anything: the history window sends `history:ready` and then sits on
// `Loading...` until `history:refresh` comes back. So `on` has to actually
// record listeners and `send` has to answer.
const listeners = new Map();
const REPLIES = { 'history:ready': 'history:refresh' };

window.ipcRenderer = {
  on: (channel, handler) => {
    const handlers = listeners.get(channel) ?? [];
    handlers.push(handler);
    listeners.set(channel, handlers);
    return () => {
      const rest = (listeners.get(channel) ?? []).filter(h => h !== handler);
      listeners.set(channel, rest);
    };
  },
  off: () => {},
  send: (channel, ...args) => {
    if (BLACKOUT_SCALE_FIXTURE && channel === 'screenshot:save-edited') {
      window.dispatchEvent(
        new CustomEvent('parity:screenshot-saved', { detail: args })
      );
    }
    const reply = REPLIES[channel];
    if (!reply) return;
    // Asynchronously, like the real IPC round trip.
    setTimeout(() => {
      for (const handler of listeners.get(reply) ?? []) handler({});
    }, 0);
  },
  invoke: async channel => {
    // `App.tsx` renders nothing until this resolves, and it is what decides
    // which window is shown.
    if (channel === 'window:get-load-data') {
      return { type: TYPE, params: await windowParams() };
    }
    if (channel === 'settings:get-ui') {
      return SHORTCUTS
        ? { appearance: APPEARANCE, shortcuts: SHORTCUTS }
        : { appearance: APPEARANCE };
    }
    if (channel === 'settings:get-appearance') return APPEARANCE;
    if (channel === 'video-editor:getVideoMetadata') {
      return {
        width: 1280,
        height: 720,
        duration: 12,
        fileSize: 0,
        bitrate: 0,
      };
    }
    if (
      [
        'video-editor:getState',
        'video-editor:getCursorData',
        'video-editor:getCameraData',
        'video-editor:getKeyboardData',
        'video-editor:getSubtitleData',
      ].includes(channel)
    ) {
      return null;
    }
    if (channel === 'video-editor:getAudioPaths') {
      return {
        systemAudioPath: null,
        micAudioPath: null,
        hasEmbeddedAudio: false,
      };
    }
    // A handful of channels are read as arrays or strings rather than merged
    // over a default, so an empty object would throw.
    // The image editor loads its capture as base64 over IPC, so serve the
    // file the `image` query parameter points at. `/@fs/<absolute path>` lets
    // Vite serve a file from outside the project.
    if (channel === 'screenshot:read-file') {
      if (!IMAGE) return '';
      const bytes = new Uint8Array(await (await fetch(IMAGE)).arrayBuffer());
      let binary = '';
      for (const byte of bytes) binary += String.fromCharCode(byte);
      return btoa(binary);
    }
    if (channel === 'storage:getTokens') return [];
    if (channel === 'storage:previewFilename') return 'Screenshot.png';
    // The About tab renders these straight into the tree.
    if (channel === 'app:getVersion') return '0.9.6';
    if (channel === 'update:getState') return { status: 'idle', progress: 0 };
    // `devices:list` resolves to a `MediaDeviceLists`, not an array.
    if (channel === 'devices:list') {
      return {
        microphones: [],
        cameras: [],
        defaultMicrophoneId: null,
        defaultCameraId: null,
      };
    }
    if (isList(channel)) return [];
    // Everything else: an empty object, which every caller merges over its own
    // defaults.
    return {};
  },
};
