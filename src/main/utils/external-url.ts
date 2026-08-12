import { shell } from 'electron';

const ALLOWED_EXTERNAL_PROTOCOLS = new Set(['http:', 'https:']);

export function openExternalUrl(url: unknown): boolean {
  if (typeof url !== 'string') return false;

  try {
    const parsed = new URL(url);
    if (!ALLOWED_EXTERNAL_PROTOCOLS.has(parsed.protocol)) return false;

    void shell.openExternal(parsed.toString());
    return true;
  } catch {
    return false;
  }
}
