import { ipcMain } from 'electron';
import type { WebContents } from 'electron';
import { randomUUID } from 'crypto';
import path from 'path';

interface ExportSession {
  id: string;
  controller: AbortController;
  sender: WebContents;
  handleDestroyed: () => void;
  outputPaths: Set<string>;
}

interface PendingExportAuthorization {
  sender: WebContents;
  handleDestroyed: () => void;
  outputPaths: Set<string>;
}

const exportSessions = new Map<number, ExportSession>();
const pendingExportAuthorizations = new Map<
  number,
  PendingExportAuthorization
>();

const UUID_PATTERN =
  '[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}';
const DERIVED_OUTPUT_PATTERNS = [
  /^\.temp\.mp4$/i,
  /^-keyboard-sound\.m4a$/i,
  new RegExp(`^\\.temp_music_${UUID_PATTERN}_\\d+\\.aac$`, 'i'),
  new RegExp(
    `^\\.temp-${UUID_PATTERN}\\.temp_(?:embedded|adjusted|mixed|audio_\\d+)\\.aac$`,
    'i'
  ),
];

function normalizePath(filePath: string): string {
  const resolved = path.resolve(filePath);
  return process.platform === 'win32' ? resolved.toLowerCase() : resolved;
}

function isDerivedOutputPath(rootPath: string, candidatePath: string): boolean {
  if (path.dirname(rootPath) !== path.dirname(candidatePath)) return false;
  if (!candidatePath.startsWith(rootPath)) return false;

  const suffix = candidatePath.slice(rootPath.length);
  return DERIVED_OUTPUT_PATTERNS.some(pattern => pattern.test(suffix));
}

function finishExportSession(senderId: number, sessionId: string): void {
  const session = exportSessions.get(senderId);
  if (!session || session.id !== sessionId) return;

  session.sender.removeListener('destroyed', session.handleDestroyed);
  exportSessions.delete(senderId);
}

function beginExportSession(sender: WebContents): string {
  const existing = exportSessions.get(sender.id);
  if (existing) {
    existing.controller.abort();
    finishExportSession(sender.id, existing.id);
  }

  const controller = new AbortController();
  const id = randomUUID();
  const handleDestroyed = () => {
    controller.abort();
    if (exportSessions.get(sender.id)?.id === id) {
      exportSessions.delete(sender.id);
    }
    pendingExportAuthorizations.delete(sender.id);
  };

  const pending = pendingExportAuthorizations.get(sender.id);
  if (pending) {
    pending.sender.removeListener('destroyed', pending.handleDestroyed);
    pendingExportAuthorizations.delete(sender.id);
  }
  const outputPaths = pending?.outputPaths ?? new Set<string>();
  exportSessions.set(sender.id, {
    id,
    controller,
    sender,
    handleDestroyed,
    outputPaths,
  });
  sender.once('destroyed', handleDestroyed);
  return id;
}

export function authorizeExportOutputPaths(
  sender: WebContents,
  outputPaths: string[]
): void {
  const existing = pendingExportAuthorizations.get(sender.id);
  if (existing) {
    existing.sender.removeListener('destroyed', existing.handleDestroyed);
  }

  const handleDestroyed = () => {
    pendingExportAuthorizations.delete(sender.id);
  };
  pendingExportAuthorizations.set(sender.id, {
    sender,
    handleDestroyed,
    outputPaths: new Set(outputPaths.map(normalizePath)),
  });
  sender.once('destroyed', handleDestroyed);
}

export function isExportOutputPathAllowed(
  senderId: number,
  filePath: string
): boolean {
  const session = exportSessions.get(senderId);
  if (!session) return false;

  const candidatePath = normalizePath(filePath);
  for (const outputPath of session.outputPaths) {
    if (
      candidatePath === outputPath ||
      isDerivedOutputPath(outputPath, candidatePath)
    ) {
      return true;
    }
  }

  return false;
}

export function getExportAbortSignal(
  senderId: number
): AbortSignal | undefined {
  return exportSessions.get(senderId)?.controller.signal;
}

export function registerExportSessionHandlers(): void {
  ipcMain.handle('video-editor:export:begin', event => {
    return beginExportSession(event.sender);
  });

  ipcMain.on('video-editor:export:cancel', (event, sessionId: string) => {
    const session = exportSessions.get(event.sender.id);
    if (!session || session.id !== sessionId) return;
    session.controller.abort();
  });

  ipcMain.handle('video-editor:export:finish', (event, sessionId: string) => {
    finishExportSession(event.sender.id, sessionId);
  });
}
