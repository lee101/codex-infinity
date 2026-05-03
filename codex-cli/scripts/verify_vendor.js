#!/usr/bin/env node
// Verify vendor binaries exist for at least one platform
import { existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const vendorRoot = join(__dirname, '..', 'vendor');

const platformGroups = {
  'linux-x64': ['x86_64-unknown-linux-gnu', 'x86_64-unknown-linux-musl'],
  'windows-x64': ['x86_64-pc-windows-msvc', 'x86_64-pc-windows-gnu'],
  'darwin-x64': ['x86_64-apple-darwin'],
  'darwin-arm64': ['aarch64-apple-darwin'],
  'linux-arm64': ['aarch64-unknown-linux-gnu', 'aarch64-unknown-linux-musl'],
};

const requiredGroups = process.env.CODEX_INFINITY_REQUIRED_GROUPS || 'any';
let foundAny = false;
const foundGroups = new Set();

for (const [group, triples] of Object.entries(platformGroups)) {
  for (const triple of triples) {
    const binaryName = triple.includes('windows') ? 'codex.exe' : 'codex';
    const binaryPath = join(vendorRoot, triple, 'codex', binaryName);
    if (existsSync(binaryPath)) {
      console.log(`Found binary for ${triple}`);
      foundAny = true;
      foundGroups.add(group);
      break;
    }
  }
}

if (requiredGroups === 'any') {
  if (!foundAny) {
    console.error('No vendor binaries found');
    process.exit(1);
  }
} else {
  const missingGroups = requiredGroups
    .split(',')
    .map((group) => group.trim())
    .filter(Boolean)
    .filter((group) => !foundGroups.has(group));

  if (missingGroups.length > 0) {
    console.error(`Missing vendor binaries for: ${missingGroups.join(', ')}`);
    process.exit(1);
  }
}

if (!foundAny) {
  console.error('No vendor binaries found');
  process.exit(1);
}

console.log('Vendor verification passed');
