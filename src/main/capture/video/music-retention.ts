import { createHash, randomUUID } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {
  getEditorStatePath,
  getMusicFolderPath,
  getProjectFolder,
} from './recording-project';
import { writeFileAtomicSync } from '@/main/utils/atomic-file';
import { isValidEditorState } from './state-validation';
import type { MusicTrack } from '@/types/music';

const RETENTION_FOLDER = '.poratake-music-retention';
const SESSIONS_FOLDER = '.poratake-music-sessions';
const COLLECTING_PREFIX = '.collecting-';
const LEGACY_COLLECTING_FILE = '.collecting';

export function isMusicNameReserved(folder: string, fileName: string): boolean {
  const records = path.join(path.dirname(folder), RETENTION_FOLDER);
  try {
    if (!fs.lstatSync(records).isDirectory())
      throw new Error('Invalid retention folder');
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return false;
    throw error;
  }
  const hash = createHash('sha256').update(fileName, 'utf8').digest('hex');
  try {
    fs.lstatSync(path.join(records, `${hash}.json`));
    return true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return false;
    throw error;
  }
}

export function recordMusicCandidate(
  projectPath: string,
  fileName: string
): void {
  const folder = getMusicFolderPath(projectPath);
  if (
    !folder ||
    !fileName ||
    fileName === '.' ||
    fileName === '..' ||
    path.basename(fileName) !== fileName
  ) {
    throw new Error('Invalid music path');
  }
  if (!fs.lstatSync(folder).isDirectory())
    throw new Error('Invalid music folder');
  if (!fs.lstatSync(path.join(folder, fileName)).isFile()) {
    throw new Error('Not a music file');
  }
  const records = path.join(path.dirname(folder), RETENTION_FOLDER);
  let created = false;
  try {
    fs.mkdirSync(records);
    created = true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error;
  }
  if (!fs.lstatSync(records).isDirectory())
    throw new Error('Invalid retention folder');
  if (created) syncDirectory(path.dirname(records));
  const hash = createHash('sha256').update(fileName, 'utf8').digest('hex');
  const marker = path.join(records, `${hash}.json`);
  const contents = JSON.stringify({ fileName });
  const existing = readCandidateRecord(marker);
  if (existing) {
    if (existing.fileName !== fileName)
      throw new Error('Music retention marker name collision');
    syncDirectory(records);
    return;
  }
  writeFileAtomicSync(marker, contents);
}

export function recordRemovedMusic(
  projectPath: string,
  before: Pick<MusicTrack, 'source' | 'fileName'>[],
  after: Pick<MusicTrack, 'source' | 'fileName'>[]
): boolean {
  const retained = new Set(
    after.filter(track => track.source === 'music').map(track => track.fileName)
  );
  const removed = new Set(
    before
      .filter(track => track.source === 'music')
      .map(track => track.fileName)
  );
  let recorded = true;
  for (const fileName of removed) {
    if (retained.has(fileName)) continue;
    try {
      recordMusicCandidate(projectPath, fileName);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') continue;
      recorded = false;
      console.error('Could not record retained music:', error);
    }
  }
  return recorded;
}

interface SessionRecord {
  pid: number;
}

interface CandidateRecord {
  fileName: string;
}

export class MusicRetentionSession {
  private closed = false;

  private constructor(
    private projectPath: string,
    private markerPath: string
  ) {}

  static open(projectOrVideoPath: string): MusicRetentionSession | null {
    const projectPath = getProjectFolder(projectOrVideoPath);
    if (!projectPath) return null;
    const session = this.create(projectPath);
    collectMusicCandidates(projectPath, session.markerPath);
    return session;
  }

  private static create(projectPath: string): MusicRetentionSession {
    const directory = path.join(projectPath, SESSIONS_FOLDER);
    try {
      fs.mkdirSync(directory);
      syncDirectory(projectPath);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error;
    }
    if (!fs.lstatSync(directory).isDirectory())
      throw new Error('Invalid music session folder');
    if (hasActiveCollector(directory))
      throw new Error('Recording music is being cleaned up');

    const markerPath = path.join(
      directory,
      `session-${process.pid}-${randomUUID()}.json`
    );
    const descriptor = fs.openSync(markerPath, 'wx', 0o600);
    try {
      try {
        fs.writeFileSync(descriptor, JSON.stringify({ pid: process.pid }));
        fs.fsyncSync(descriptor);
      } finally {
        fs.closeSync(descriptor);
      }
      syncDirectory(directory);
      if (hasActiveCollector(directory))
        throw new Error('Recording music is being cleaned up');
    } catch (error) {
      discardSessionMarker(markerPath, directory);
      throw error;
    }
    const session = new MusicRetentionSession(projectPath, markerPath);
    return session;
  }

  lease(): MusicRetentionSession {
    return MusicRetentionSession.create(this.projectPath);
  }

  updateProject(projectOrVideoPath: string): void {
    const projectPath = getProjectFolder(projectOrVideoPath);
    if (!projectPath) return;
    this.markerPath = path.join(
      projectPath,
      SESSIONS_FOLDER,
      path.basename(this.markerPath)
    );
    this.projectPath = projectPath;
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    try {
      fs.unlinkSync(this.markerPath);
      syncDirectory(path.dirname(this.markerPath));
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') return;
    }
    const projectPath = this.projectPath;
    setImmediate(() => collectMusicCandidates(projectPath));
  }

  abandon(): void {
    this.closed = true;
  }
}

function syncDirectory(directory: string): void {
  if (process.platform === 'win32' || path.sep === '\\') return;
  const descriptor = fs.openSync(directory, 'r');
  try {
    fs.fsyncSync(descriptor);
  } finally {
    fs.closeSync(descriptor);
  }
}

function discardSessionMarker(marker: string, directory: string): void {
  try {
    fs.unlinkSync(marker);
  } catch {}
  try {
    syncDirectory(directory);
  } catch {}
}

function readSessionRecord(filePath: string): SessionRecord | null {
  try {
    const value: unknown = JSON.parse(fs.readFileSync(filePath, 'utf8'));
    if (
      !value ||
      typeof value !== 'object' ||
      !Number.isSafeInteger((value as SessionRecord).pid) ||
      (value as SessionRecord).pid <= 0
    ) {
      return null;
    }
    return value as SessionRecord;
  } catch {
    return null;
  }
}

function readCandidateRecord(filePath: string): CandidateRecord | null {
  let metadata: fs.Stats;
  try {
    metadata = fs.lstatSync(filePath);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return null;
    throw error;
  }
  if (!metadata.isFile()) throw new Error('Invalid music retention record');
  try {
    const value: unknown = JSON.parse(fs.readFileSync(filePath, 'utf8'));
    if (
      value &&
      typeof value === 'object' &&
      'fileName' in value &&
      typeof value.fileName === 'string'
    ) {
      return value as CandidateRecord;
    }
  } catch {}
  return null;
}

function isProcessAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ESRCH') return false;
    return true;
  }
}

function hasActiveCollector(directory: string): boolean {
  let entries: fs.Dirent[];
  try {
    entries = fs.readdirSync(directory, { withFileTypes: true });
  } catch {
    return true;
  }
  for (const entry of entries) {
    if (entry.name === LEGACY_COLLECTING_FILE) {
      if (!entry.isFile()) return true;
      const record = readSessionRecord(path.join(directory, entry.name));
      if (!record || isProcessAlive(record.pid)) return true;
      continue;
    }
    if (!entry.name.startsWith(COLLECTING_PREFIX)) continue;
    if (!entry.isFile()) return true;
    const owner = /^\.collecting-(\d+)-.+$/.exec(entry.name);
    const pid = owner ? Number(owner[1]) : NaN;
    if (!Number.isSafeInteger(pid) || pid <= 0) return true;
    if (isProcessAlive(pid)) return true;
    try {
      fs.unlinkSync(path.join(directory, entry.name));
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') return true;
    }
  }
  return false;
}

function sessionProcessId(directory: string, entry: fs.Dirent): number | null {
  const marker = path.join(directory, entry.name);
  const name = /^session-(\d+)-.+\.json$/.exec(entry.name);
  if (name) {
    const pid = Number(name[1]);
    if (!Number.isSafeInteger(pid) || pid <= 0) return null;
    const record = readSessionRecord(marker);
    return record && record.pid !== pid ? null : pid;
  }
  return readSessionRecord(marker)?.pid ?? null;
}

function sameMusicFile(left: string, right: string): boolean {
  try {
    const leftMetadata = fs.statSync(left, { bigint: true });
    const rightMetadata = fs.statSync(right, { bigint: true });
    if (leftMetadata.dev !== rightMetadata.dev) return false;
    if (leftMetadata.ino !== 0n && rightMetadata.ino !== 0n) {
      return leftMetadata.ino === rightMetadata.ino;
    }
    return (
      process.platform === 'win32' &&
      path.resolve(left).toLowerCase() === path.resolve(right).toLowerCase()
    );
  } catch {
    return false;
  }
}

function collectMusicCandidates(
  projectPath: string,
  ignoredSession?: string
): void {
  const directory = path.join(projectPath, SESSIONS_FOLDER);
  try {
    if (!fs.lstatSync(directory).isDirectory()) return;
  } catch {
    return;
  }
  if (hasActiveCollector(directory)) return;
  const lockPath = path.join(
    directory,
    `${COLLECTING_PREFIX}${process.pid}-${randomUUID()}`
  );
  let lockDescriptor: number;
  try {
    lockDescriptor = fs.openSync(lockPath, 'wx', 0o600);
  } catch {
    return;
  }
  try {
    try {
      fs.fsyncSync(lockDescriptor);
    } finally {
      fs.closeSync(lockDescriptor);
      lockDescriptor = -1;
    }
    syncDirectory(directory);
    const sessionEntries = fs.readdirSync(directory, { withFileTypes: true });
    for (const entry of sessionEntries) {
      const marker = path.join(directory, entry.name);
      if (
        marker === lockPath ||
        marker === ignoredSession ||
        entry.name === LEGACY_COLLECTING_FILE ||
        entry.name.startsWith(COLLECTING_PREFIX)
      ) {
        continue;
      }
      if (!entry.name.startsWith('session-') || !entry.name.endsWith('.json'))
        return;
      if (!entry.isFile()) return;
      const pid = sessionProcessId(directory, entry);
      if (pid === null) return;
      if (isProcessAlive(pid)) return;
      fs.unlinkSync(marker);
    }

    const statePath = getEditorStatePath(projectPath);
    if (!statePath || !fs.lstatSync(statePath).isFile()) return;
    const stateValue: unknown = JSON.parse(fs.readFileSync(statePath, 'utf8'));
    if (!isValidEditorState(stateValue)) return;
    const musicFolder = getMusicFolderPath(projectPath);
    if (!musicFolder || !fs.lstatSync(musicFolder).isDirectory()) return;
    const records = path.join(projectPath, RETENTION_FOLDER);
    if (!fs.lstatSync(records).isDirectory()) return;
    const retained = (stateValue.musicTracks ?? []).filter(
      track => track.source === 'music'
    );
    for (const entry of fs.readdirSync(records, { withFileTypes: true })) {
      if (!entry.isFile()) continue;
      const marker = path.join(records, entry.name);
      let candidate: CandidateRecord;
      try {
        candidate = JSON.parse(
          fs.readFileSync(marker, 'utf8')
        ) as CandidateRecord;
      } catch {
        continue;
      }
      if (
        typeof candidate.fileName !== 'string' ||
        !candidate.fileName ||
        path.basename(candidate.fileName) !== candidate.fileName ||
        entry.name !==
          `${createHash('sha256').update(candidate.fileName).digest('hex')}.json`
      ) {
        continue;
      }
      const source = path.join(musicFolder, candidate.fileName);
      try {
        if (!fs.lstatSync(source).isFile()) continue;
        if (
          retained.some(track =>
            sameMusicFile(source, path.join(musicFolder, track.fileName))
          )
        ) {
          continue;
        }
        fs.unlinkSync(source);
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== 'ENOENT') continue;
      }
      try {
        fs.unlinkSync(marker);
        syncDirectory(musicFolder);
        syncDirectory(records);
      } catch {}
    }
  } catch {
    return;
  } finally {
    if (lockDescriptor !== -1) {
      try {
        fs.closeSync(lockDescriptor);
      } catch {}
    }
    try {
      fs.unlinkSync(lockPath);
      syncDirectory(directory);
    } catch {}
  }
}
