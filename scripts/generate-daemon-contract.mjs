import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const contractPath = path.join(root, 'src/types/daemon-contract.json');
const contract = JSON.parse(fs.readFileSync(contractPath, 'utf8'));
if (contract.version !== 1) {
  throw new Error(`Unsupported daemon contract version: ${contract.version}`);
}
const modules = Object.entries(contract.modules);
const capabilitiesPath = path.join(
  root,
  'src/types/platform-capabilities.json'
);
const capabilities = JSON.parse(fs.readFileSync(capabilitiesPath, 'utf8'));
if (capabilities.version !== 1) {
  throw new Error(
    `Unsupported platform capabilities version: ${capabilities.version}`
  );
}
const featureIds = new Set(capabilities.features);
if (featureIds.size !== capabilities.features.length) {
  throw new Error('Duplicate feature in platform capabilities');
}
const capabilityTargets = [
  'macos',
  'windows',
  'linuxX11',
  'linuxWayland',
  'headless',
];
const actualCapabilityTargets = Object.keys(capabilities.targets);
if (
  actualCapabilityTargets.length !== capabilityTargets.length ||
  capabilityTargets.some(target => !actualCapabilityTargets.includes(target))
) {
  throw new Error('Platform capability targets do not match the contract');
}
for (const [target, features] of Object.entries(capabilities.targets)) {
  if (new Set(features).size !== features.length) {
    throw new Error(`Duplicate capability for ${target}`);
  }
  for (const feature of features) {
    if (!featureIds.has(feature)) {
      throw new Error(`Unknown capability ${feature} for ${target}`);
    }
  }
}

const typeOverrides = new Map([
  ['ocr', 'Ocr'],
  ['qrcode', 'QrCode'],
]);
const swiftTypeOverrides = new Map([
  ['ocr', 'OCR'],
  ['qrcode', 'QRCode'],
]);

function words(value) {
  return value
    .replaceAll('-', ' ')
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replace(/([A-Z]+)([A-Z][a-z])/g, '$1 $2')
    .split(/\s+/)
    .filter(Boolean);
}

function pascal(value) {
  return words(value)
    .map(word => word[0].toUpperCase() + word.slice(1).toLowerCase())
    .join('');
}

function camel(value) {
  const result = pascal(value);
  return result[0].toLowerCase() + result.slice(1);
}

function rustType(module) {
  return `${typeOverrides.get(module) ?? pascal(module)}Method`;
}

function swiftType(module) {
  return swiftTypeOverrides.get(module) ?? pascal(module);
}

function moduleConstant(module) {
  return `${module.replaceAll('-', '_').toUpperCase()}_MODULE`;
}

function featureType(feature) {
  return typeOverrides.get(feature) ?? pascal(feature);
}

function typescriptCapabilities() {
  const lines = ['export const FEATURE_IDS = ['];
  for (const feature of capabilities.features) {
    lines.push(`  '${feature}',`);
  }
  lines.push(
    '] as const;',
    '',
    'export type FeatureId = (typeof FEATURE_IDS)[number];',
    '',
    'export const PLATFORM_CAPABILITIES = {'
  );
  for (const [target, features] of Object.entries(capabilities.targets)) {
    const inline = `  ${target}: [${features.map(feature => `'${feature}'`).join(', ')}],`;
    if (inline.length <= 80) {
      lines.push(inline);
      continue;
    }
    lines.push(`  ${target}: [`);
    for (const feature of features) {
      lines.push(`    '${feature}',`);
    }
    lines.push('  ],');
  }
  lines.push(
    '} as const satisfies Record<string, readonly FeatureId[]>;',
    '',
    'export type CapabilityTarget = keyof typeof PLATFORM_CAPABILITIES;',
    ''
  );
  return lines.join('\n');
}

function rustCapabilities() {
  const lines = [
    '#[cfg_attr(target_os = "linux", allow(dead_code))]',
    '#[derive(Clone, Copy, PartialEq, Eq, Debug)]',
    'pub enum Feature {',
  ];
  for (const feature of capabilities.features) {
    lines.push(`    ${featureType(feature)},`);
  }
  lines.push('}', '');
  for (const [target, features] of Object.entries(capabilities.targets)) {
    const platform =
      target.startsWith('linux') || target === 'headless' ? 'linux' : target;
    lines.push(`#[cfg(target_os = "${platform}")]`);
    const constant = target
      .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
      .toUpperCase();
    if (features.length === 0) {
      lines.push(`pub const ${constant}_FEATURES: &[Feature] = &[];`, '');
      continue;
    }
    lines.push(`pub const ${constant}_FEATURES: &[Feature] = &[`);
    for (const feature of features) {
      lines.push(`    Feature::${featureType(feature)},`);
    }
    lines.push('];', '');
  }
  return lines.join('\n');
}

function typescript() {
  const lines = ['export const DAEMON_METHODS = {'];
  for (const [module, definition] of modules) {
    const methods = definition.shared;
    const key = /^[a-z]+$/.test(module) ? module : `'${module}'`;
    const inline = `  ${key}: [${methods.map(method => `'${method}'`).join(', ')}],`;
    if (inline.length <= 80) {
      lines.push(inline);
      continue;
    }
    lines.push(`  ${key}: [`);
    for (const method of methods) {
      lines.push(`    '${method}',`);
    }
    lines.push('  ],');
  }
  lines.push('} as const;', '');
  return lines.join('\n');
}

function rust() {
  const lines = [];
  for (const [module] of modules) {
    lines.push(`pub const ${moduleConstant(module)}: &str = "${module}";`);
  }
  lines.push('');
  for (const platform of ['macos', 'windows', 'linux']) {
    const names = modules
      .filter(([, definition]) => definition.platforms.includes(platform))
      .map(([module]) => moduleConstant(module));
    lines.push(`pub const ${platform.toUpperCase()}_MODULES: &[&str] = &[`);
    for (const name of names) {
      lines.push(`    ${name},`);
    }
    lines.push('];', '');
  }
  lines.push(
    'macro_rules! daemon_methods {',
    '    ($name:ident, { $($variant:ident => $id:literal),+ $(,)? }) => {',
    '        #[derive(Clone, Copy, Debug, PartialEq, Eq)]',
    '        pub enum $name {',
    '            $($variant),+',
    '        }',
    '',
    '        impl $name {',
    '            pub const ALL: [Self; [$(stringify!($variant)),+].len()] = [$(Self::$variant),+];',
    '',
    "            pub const fn id(self) -> &'static str {",
    '                match self {',
    '                    $(Self::$variant => $id),+',
    '                }',
    '            }',
    '',
    '            pub fn parse(method: &str) -> Option<Self> {',
    '                Self::ALL.into_iter().find(|item| item.id() == method)',
    '            }',
    '        }',
    '    };',
    '}',
    ''
  );
  for (const [module, definition] of modules) {
    const methods = definition.shared;
    lines.push(`daemon_methods!(${rustType(module)}, {`);
    for (const method of methods) {
      lines.push(`    ${pascal(method)} => "${method}",`);
    }
    lines.push('});');
    for (const platform of ['macos', 'windows', 'linux']) {
      const platformMethods = definition[platform] ?? [];
      if (platformMethods.length === 0) {
        continue;
      }
      lines.push(
        `daemon_methods!(${rustType(module).replace(/Method$/, `${pascal(platform)}Method`)}, {`
      );
      for (const method of platformMethods) {
        lines.push(`    ${pascal(method)} => "${method}",`);
      }
      lines.push('});');
    }
  }
  lines.push('');
  return lines.join('\n');
}

function swift() {
  const lines = ['import Foundation', '', 'enum DaemonContract {'];
  for (const [module, definition] of modules.filter(([, value]) =>
    value.platforms.includes('macos')
  )) {
    const type = swiftType(module);
    lines.push(`    enum ${type} {`, `        static let module = "${module}"`);
    lines.push('', '        enum Method: String {');
    for (const method of definition.shared) {
      lines.push(`            case ${camel(method)} = "${method}"`);
    }
    lines.push('        }');
    lines.push('    }', '');
  }
  lines.push('    static let macOSModules: Set<String> = [');
  for (const [module] of modules.filter(([, value]) =>
    value.platforms.includes('macos')
  )) {
    lines.push(`        ${swiftType(module)}.module,`);
  }
  lines.push('    ]', '}');
  return `${lines.join('\n')}\n`;
}

const outputs = new Map([
  [path.join(root, 'src/types/daemon-methods.generated.ts'), typescript()],
  [
    path.join(root, 'src/types/capabilities.generated.ts'),
    typescriptCapabilities(),
  ],
  [path.join(root, 'src/main/daemon-common/src/generated_methods.rs'), rust()],
  [
    path.join(root, 'src/main/app-gpui/src/system/capabilities.generated.rs'),
    rustCapabilities(),
  ],
  [
    path.join(root, 'src/main/daemon/Core/DaemonContract.generated.swift'),
    swift(),
  ],
]);

if (process.argv.includes('--check')) {
  const stale = [...outputs].filter(
    ([file, expected]) =>
      !fs.existsSync(file) || fs.readFileSync(file, 'utf8') !== expected
  );
  if (stale.length > 0) {
    for (const [file] of stale) {
      console.error(path.relative(root, file));
    }
    process.exit(1);
  }
  process.exit(0);
}

for (const [file, content] of outputs) {
  fs.writeFileSync(file, content);
}
