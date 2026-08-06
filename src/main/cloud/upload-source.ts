import fs from 'fs';
import { Readable } from 'stream';

export type UploadSource =
  | { kind: 'buffer'; buffer: Buffer }
  | { kind: 'file'; path: string; size: number };

export interface ResolvedUploadBody {
  body: ReadableStream<Uint8Array>;
  size: number;
}

export function bufferSource(buffer: Buffer): UploadSource {
  return { kind: 'buffer', buffer };
}

export async function fileSource(path: string): Promise<UploadSource> {
  const { size } = await fs.promises.stat(path);
  return { kind: 'file', path, size };
}

export function resolveUploadBody(source: UploadSource): ResolvedUploadBody {
  if (source.kind === 'buffer') {
    const stream = Readable.toWeb(
      Readable.from(source.buffer)
    ) as ReadableStream<Uint8Array>;
    return { body: stream, size: source.buffer.length };
  }

  const stream = Readable.toWeb(
    fs.createReadStream(source.path)
  ) as ReadableStream<Uint8Array>;
  return { body: stream, size: source.size };
}
