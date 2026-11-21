import { readFileSync, writeFileSync } from 'node:fs';

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  process.stderr.write('Usage: set-package-version.mjs <x.y.z>\n');
  process.exit(1);
}

const packageJsonPath = 'package.json';
const cargoTomlPath = 'src/main/app-gpui/Cargo.toml';
const cargoLockPath = 'src/main/Cargo.lock';

const pkg = JSON.parse(readFileSync(packageJsonPath, 'utf8'));
pkg.version = version;
writeFileSync(packageJsonPath, `${JSON.stringify(pkg, null, 2)}\n`);

const cargoToml = readFileSync(cargoTomlPath, 'utf8');
const nextCargoToml = cargoToml.replace(
  /^version = "[^"]*"/m,
  `version = "${version}"`
);
if (nextCargoToml === cargoToml) {
  process.stderr.write(`Could not update the version in ${cargoTomlPath}\n`);
  process.exit(1);
}
writeFileSync(cargoTomlPath, nextCargoToml);

const cargoLock = readFileSync(cargoLockPath, 'utf8');
const nextCargoLock = cargoLock.replace(
  /(\[\[package\]\]\nname = "poratake-gpui"\nversion = ")[^"]*(")/,
  `$1${version}$2`
);
if (nextCargoLock === cargoLock) {
  process.stderr.write(`Could not update the version in ${cargoLockPath}\n`);
  process.exit(1);
}
writeFileSync(cargoLockPath, nextCargoLock);
