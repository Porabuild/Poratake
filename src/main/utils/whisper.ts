import path from 'path';
import fs, { createWriteStream } from 'fs';
import https from 'https';
import http from 'http';
import { pipeline } from 'stream/promises';
import {
  getConfigDir,
  getNativeBinaryPath,
  ensureDirectoryExists,
} from './paths';
import type { WhisperModel } from '@/types/subtitle';

const HUGGINGFACE_MODEL_BASE =
  'https://huggingface.co/ggerganov/whisper.cpp/resolve/main';
const MAX_DOWNLOAD_REDIRECTS = 5;

const MODEL_FILES: Record<WhisperModel, string> = {
  base: 'ggml-base.bin',
  small: 'ggml-small.bin',
  medium: 'ggml-medium.bin',
};

export function getWhisperDir(): string {
  return path.join(getConfigDir(), 'whisper');
}

export function getWhisperCliPath(): string {
  return getNativeBinaryPath('whisper');
}

export function getWhisperModelPath(model: WhisperModel): string {
  return path.join(getWhisperDir(), MODEL_FILES[model]);
}

export function isWhisperBinaryAvailable(): boolean {
  const cliPath = getWhisperCliPath();
  return fs.existsSync(cliPath);
}

export function isWhisperModelAvailable(model: WhisperModel): boolean {
  const modelPath = getWhisperModelPath(model);
  return fs.existsSync(modelPath);
}

export function isWhisperReady(model: WhisperModel): boolean {
  return isWhisperBinaryAvailable() && isWhisperModelAvailable(model);
}

async function downloadFile(
  url: string,
  destPath: string,
  onProgress?: (percent: number) => void,
  redirectsRemaining = MAX_DOWNLOAD_REDIRECTS
): Promise<void> {
  return new Promise((resolve, reject) => {
    const makeRequest = (requestUrl: string) => {
      const protocol = requestUrl.startsWith('https') ? https : http;

      protocol
        .get(
          requestUrl,
          { headers: { 'User-Agent': 'Poratake' } },
          response => {
            if (
              response.statusCode &&
              response.statusCode >= 300 &&
              response.statusCode < 400 &&
              response.headers.location
            ) {
              response.resume();
              if (redirectsRemaining === 0) {
                reject(new Error('Too many redirects while downloading'));
                return;
              }

              let redirectUrl: string;
              try {
                redirectUrl = new URL(
                  response.headers.location,
                  requestUrl
                ).toString();
              } catch (error) {
                reject(error);
                return;
              }

              void downloadFile(
                redirectUrl,
                destPath,
                onProgress,
                redirectsRemaining - 1
              ).then(resolve, reject);
              return;
            }

            if (response.statusCode !== 200) {
              response.resume();
              reject(
                new Error(`Failed to download: HTTP ${response.statusCode}`)
              );
              return;
            }

            const totalSize = parseInt(
              response.headers['content-length'] || '0',
              10
            );
            let downloadedSize = 0;

            const fileStream = createWriteStream(destPath);

            response.on('data', chunk => {
              downloadedSize += chunk.length;
              if (totalSize > 0 && onProgress) {
                const percent = Math.round((downloadedSize / totalSize) * 100);
                onProgress(percent);
              }
            });

            pipeline(response, fileStream)
              .then(() => resolve())
              .catch(reject);
          }
        )
        .on('error', reject);
    };

    makeRequest(url);
  });
}

export async function downloadWhisperModel(
  model: WhisperModel,
  onProgress?: (percent: number) => void
): Promise<void> {
  const whisperDir = getWhisperDir();
  ensureDirectoryExists(whisperDir);

  const modelFile = MODEL_FILES[model];
  const modelUrl = `${HUGGINGFACE_MODEL_BASE}/${modelFile}`;
  const modelPath = getWhisperModelPath(model);
  const partialPath = `${modelPath}.download`;

  if (fs.existsSync(partialPath)) {
    fs.unlinkSync(partialPath);
  }

  try {
    await downloadFile(modelUrl, partialPath, onProgress);
    fs.renameSync(partialPath, modelPath);
  } catch (error) {
    if (fs.existsSync(partialPath)) {
      fs.unlinkSync(partialPath);
    }
    throw error;
  }
}

export async function ensureWhisperReady(
  model: WhisperModel,
  onProgress?: (info: {
    item: 'binary' | WhisperModel;
    percent: number;
  }) => void
): Promise<void> {
  if (!isWhisperBinaryAvailable()) {
    throw new Error('Whisper binary not found. Please reinstall the app.');
  }

  if (!isWhisperModelAvailable(model)) {
    await downloadWhisperModel(model, percent => {
      onProgress?.({ item: model, percent });
    });
  }
}

export function getAvailableModels(): WhisperModel[] {
  const models: WhisperModel[] = [];
  if (isWhisperModelAvailable('base')) models.push('base');
  if (isWhisperModelAvailable('small')) models.push('small');
  if (isWhisperModelAvailable('medium')) models.push('medium');
  return models;
}
