import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';
import { DAEMON_METHODS, type DaemonModule } from '@/types/daemon';

function registeredModules(source: string, pattern: RegExp): string[] {
  return [...source.matchAll(pattern)]
    .map(match => match[1].replace(/Module$/, '').toLowerCase())
    .toSorted();
}

function quotedValues(source: string): string[] {
  return [...source.matchAll(/"([^"]+)"/g)].map(match => match[1]);
}

function switchBody(source: string, start: number): string {
  let depth = 0;
  for (
    let index = source.indexOf('{', start);
    index < source.length;
    index += 1
  ) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}') depth -= 1;
    if (depth === 0) return source.slice(start, index);
  }
  return '';
}

function swiftMethods(source: string): string[] {
  const start = source.indexOf('switch method');
  expect(start).toBeGreaterThan(-1);
  const body = switchBody(source, start);
  expect(body).not.toBe('');

  return [...body.matchAll(/^\s*case\s+([^:\n]+):/gm)]
    .flatMap(match => quotedValues(match[1]))
    .toSorted();
}

function rustMethods(source: string): string[] {
  const start = source.indexOf('match request.method.as_str()');
  expect(start).toBeGreaterThan(-1);
  const dispatch = source.slice(start);
  const fallback = dispatch.match(
    /^([ \t]*)method => method_not_found\(method\),?$/m
  );
  expect(fallback).not.toBeNull();
  const end = fallback!.index!;
  const indentation = fallback![1].replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

  const arms = dispatch
    .slice(0, end)
    .matchAll(
      new RegExp(
        `^${indentation}(?:\\w+ @ \\()?((?:"[^"]+"(?:\\s*\\|\\s*"[^"]+")*))\\)?\\s*=>`,
        'gm'
      )
    );
  return [...arms].flatMap(match => quotedValues(match[1])).toSorted();
}

function swiftContracts(root: string, main: string): Record<string, string[]> {
  const contracts: Record<string, string[]> = {};
  const classes = [
    ...main.matchAll(/router\.register\((\w+Module)\(\)\)/g),
  ].map(match => match[1]);

  for (const className of classes) {
    const source = fs.readFileSync(
      path.join(root, 'src/main/daemon/Modules', `${className}.swift`),
      'utf-8'
    );
    const moduleName = source.match(/let name = "([^"]+)"/)?.[1];
    expect(moduleName).toBeDefined();
    contracts[moduleName!] = swiftMethods(source);
  }

  return contracts;
}

function rustContracts(root: string, main: string): Record<string, string[]> {
  const contracts: Record<string, string[]> = {};
  const sourceFiles = new Map(
    [...main.matchAll(/use modules::(\w+)::(\w+);/g)].map(match => [
      match[2],
      match[1],
    ])
  );
  const classes = [
    ...main.matchAll(
      /router\.register\(Box::new\((\w+Module)(?:::new\(\))?\)\);/g
    ),
  ].map(match => match[1]);

  for (const className of classes) {
    const sourceFile = sourceFiles.get(className);
    expect(sourceFile).toBeDefined();
    const source = fs.readFileSync(
      path.join(root, 'src/main/daemon-win/src/modules', `${sourceFile}.rs`),
      'utf-8'
    );
    const moduleName = source.match(
      /fn name\(&self\) -> &'static str \{\s*"([^"]+)"/
    )?.[1];
    expect(moduleName).toBeDefined();
    contracts[moduleName!] = rustMethods(source);
  }

  return contracts;
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

    const moduleCount = Object.keys(DAEMON_METHODS).length;
    expect(swiftModules).toHaveLength(moduleCount);
    expect(rustModules).toHaveLength(moduleCount);
    expect(swiftModules).toEqual(rustModules);
  });

  it('implements the exact shared method contract on both platforms', () => {
    const root = process.cwd();
    const swiftMain = fs.readFileSync(
      path.join(root, 'src/main/daemon/main.swift'),
      'utf-8'
    );
    const rustMain = fs.readFileSync(
      path.join(root, 'src/main/daemon-win/src/main.rs'),
      'utf-8'
    );
    const expected = Object.fromEntries(
      Object.entries(DAEMON_METHODS).map(([moduleName, methods]) => [
        moduleName,
        [...methods].toSorted(),
      ])
    ) as Record<DaemonModule, string[]>;

    expect(swiftContracts(root, swiftMain)).toEqual(expected);
    expect(rustContracts(root, rustMain)).toEqual(expected);
  });

  it('uses the shared title field for native window lists', () => {
    const root = process.cwd();
    const swift = fs.readFileSync(
      path.join(root, 'src/main/daemon/Modules/WindowSelectorModule.swift'),
      'utf-8'
    );
    const rust = fs.readFileSync(
      path.join(root, 'src/main/daemon-win/src/modules/window_selector.rs'),
      'utf-8'
    );

    expect(swift).toContain('"title": info.title');
    expect(rust).toContain('"title": target.title.as_str()');
  });
});
