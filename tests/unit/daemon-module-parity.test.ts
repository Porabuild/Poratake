import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

function registeredModules(source: string, pattern: RegExp): string[] {
  return [...source.matchAll(pattern)]
    .map(match => match[1].replace(/Module$/, '').toLowerCase())
    .toSorted();
}

describe('native daemon module parity', () => {
  it('registers the same modules on macOS and Windows', () => {
    const root = process.cwd();
    const swift = fs.readFileSync(
      path.join(root, 'src/main/daemon/main.swift'),
      'utf-8'
    );
    const rust = fs.readFileSync(
      path.join(root, 'src/main/daemon-win/src/main.rs'),
      'utf-8'
    );

    const swiftModules = registeredModules(
      swift,
      /router\.register\((\w+Module)\(\)\)/g
    );
    const rustModules = registeredModules(
      rust,
      /router\.register\(Box::new\((\w+Module)(?:::new\(\))?\)\);/g
    );

    expect(swiftModules).toHaveLength(18);
    expect(rustModules).toHaveLength(18);
    expect(swiftModules).toEqual(rustModules);
  });
});
