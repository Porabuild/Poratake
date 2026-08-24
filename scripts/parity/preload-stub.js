// Stands in for `src/preload/preload.ts` so the Electron renderer can be drawn
// in an ordinary browser and compared against the GPUI shell, pixel for pixel.
//
// Inject as an on-new-document script (Chrome DevTools
// `Page.addScriptToEvaluateOnNewDocument`) before loading
// `http://localhost:5599/index.html?window=<type>#<tab>`.
//
// Set APPEARANCE, ACCENT and SHORTCUTS to whatever the GPUI app is reading from
// its dev config -- otherwise a colour or an accelerator will differ for a
// reason that has nothing to do with the shells.
window.appPlatform = 'win32';

const APPEARANCE = { mode: 'dark', theme: 'default' };
const ACCENT = '#007AFF';
const SHORTCUTS = undefined; // undefined => the renderer's own defaults

const query = new URLSearchParams(location.search);
const TYPE = query.get('window') || 'settings';
/// `?image=/@fs/C:/path/to/capture.png` for the windows that show one.
const IMAGE = query.get('image');
const isList = channel => /list$|:list/.test(channel);

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
  send: channel => {
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
    if (channel === 'window:get-load-data') return { type: TYPE, params: {} };
    if (channel === 'settings:get-ui') {
      return SHORTCUTS
        ? { appearance: APPEARANCE, shortcuts: SHORTCUTS }
        : { appearance: APPEARANCE };
    }
    if (channel === 'settings:get-appearance') return APPEARANCE;
    if (channel === 'system:preferences:get-accent-color') return ACCENT;
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
    if (channel === 'app:getVersion') return '0.9.5';
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
