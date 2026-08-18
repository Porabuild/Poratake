import { transformFileAsync } from '@babel/core';
import fs from 'node:fs';
import { loadConfigFromFile } from 'vite';

if (!process.versions.bun) {
  throw new Error('React Compiler verification must run under Bun');
}

const loaded = await loadConfigFromFile(
  { command: 'build', mode: 'production' },
  'vite.config.mts',
  undefined,
  undefined,
  undefined,
  'native'
);
if (!loaded) {
  throw new Error('Vite config did not load with the native loader');
}

const config = fs.readFileSync('vite.config.mts', 'utf8');
if (!config.includes('babel({ presets: [reactCompilerPreset()] })')) {
  throw new Error('Vite is not configured with React Compiler');
}

const files = [];
for await (const file of new Bun.Glob('src/renderer/**/*.{ts,tsx}').scan('.')) {
  files.push(file);
}

let optimized = 0;
for (const file of files) {
  const parserPlugins = file.endsWith('.tsx')
    ? ['jsx', 'typescript']
    : ['typescript'];
  const result = await transformFileAsync(file, {
    plugins: ['babel-plugin-react-compiler'],
    parserOpts: { plugins: parserPlugins },
  });
  if (result?.code?.includes('react/compiler-runtime')) {
    optimized++;
  }
}

if (optimized === 0) {
  throw new Error('React Compiler did not optimize any renderer modules');
}

console.log(
  `React Compiler optimized ${optimized}/${files.length} renderer modules.`
);
