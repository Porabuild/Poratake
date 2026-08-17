import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

import {
  macAssetNames,
  publishedAssetNames,
  windowsAssetNames,
} from '../../scripts/published-release-assets.mjs';

const roots: string[] = [];

afterEach(() => {
  roots
    .splice(0)
    .forEach(root => rmSync(root, { recursive: true, force: true }));
});

describe('published release assets', () => {
  it('requires two Mac packages and Windows x64 plus arm64', () => {
    const version = '1.0.0';
    expect(macAssetNames(version)).toEqual([
      'Poratake-1.0.0-universal.dmg',
      'Poratake-1.0.0-universal-mac.zip',
      'latest-mac.yml',
      'Poratake-1.0.0-universal-mac.zip.blockmap',
    ]);
    expect(windowsAssetNames(version)).toEqual([
      'Poratake-1.0.0-win-x64.exe',
      'Poratake-1.0.0-win-x64.exe.blockmap',
      'Poratake-1.0.0-win-arm64.exe',
      'Poratake-1.0.0-win-arm64.exe.blockmap',
      'latest.yml',
    ]);
    expect(publishedAssetNames(version)).toContain(
      'Poratake-1.0.0-win-arm64.exe'
    );
    expect(publishedAssetNames(version)).not.toEqual(
      publishedAssetNames(version).filter(name => !name.includes('win-arm64'))
    );
  });

  it('prints the same expected list the publish job compares against', () => {
    const printed = spawnSync(
      process.execPath,
      [
        path.join(process.cwd(), 'scripts', 'published-release-assets.mjs'),
        'expected',
        '1.0.0',
      ],
      { encoding: 'utf8' }
    );
    expect(printed.status).toBe(0);
    expect(printed.stdout.trim().split('\n')).toEqual(
      publishedAssetNames('1.0.0')
    );
  });

  it('merges Windows x64 and arm64 installers into one updater feed', async () => {
    const root = mkdtempSync(path.join(tmpdir(), 'poratake-win-yml-'));
    roots.push(root);
    const x64Path = path.join(root, 'Poratake-1.0.0-win-x64.exe');
    const arm64Path = path.join(root, 'Poratake-1.0.0-win-arm64.exe');
    const outputPath = path.join(root, 'latest.yml');
    writeFileSync(x64Path, 'x64-installer');
    writeFileSync(arm64Path, 'arm64-installer-bytes');

    const merged = spawnSync(
      process.execPath,
      [
        path.join(process.cwd(), 'scripts', 'merge-windows-latest-yml.mjs'),
        '1.0.0',
        x64Path,
        arm64Path,
        outputPath,
      ],
      { encoding: 'utf8' }
    );
    expect(merged.status).toBe(0);
    const yaml = readFileSync(outputPath, 'utf8');
    expect(yaml).toContain('Poratake-1.0.0-win-x64.exe');
    expect(yaml).toContain('Poratake-1.0.0-win-arm64.exe');
    expect(yaml).toContain('version: ');
  });
});
