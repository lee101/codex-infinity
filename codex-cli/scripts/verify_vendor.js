#!/usr/bin/env node
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const packageRoot = path.resolve(__dirname, "..");
const vendorRoot = path.join(packageRoot, "vendor");

const requiredGroups = [
  {
    label: "linux-x64",
    triples: ["x86_64-unknown-linux-musl", "x86_64-unknown-linux-gnu"],
  },
  {
    label: "linux-arm64",
    triples: ["aarch64-unknown-linux-musl", "aarch64-unknown-linux-gnu"],
  },
  { label: "darwin-x64", triples: ["x86_64-apple-darwin"] },
  { label: "darwin-arm64", triples: ["aarch64-apple-darwin"] },
  { label: "win32-x64", triples: ["x86_64-pc-windows-msvc"] },
  { label: "win32-arm64", triples: ["aarch64-pc-windows-msvc"] },
];

function binaryPathFor(triple) {
  const binaryName = triple.includes("windows") ? "codex.exe" : "codex";
  return path.join(vendorRoot, triple, "codex", binaryName);
}

const requiredOverride = process.env.CODEX_INFINITY_REQUIRED_GROUPS;
const requestedLabels = requiredOverride
  ? requiredOverride
      .split(",")
      .map((label) => label.trim())
      .filter(Boolean)
  : null;

const requireAny = requestedLabels?.length === 1 && requestedLabels[0] === "any";
const groupsToCheck = requestedLabels
  ? requiredGroups.filter((group) => requestedLabels.includes(group.label))
  : requiredGroups;

const hasAnyBinary = requiredGroups.some((group) =>
  group.triples.some((triple) => existsSync(binaryPathFor(triple))),
);

if (requireAny) {
  if (hasAnyBinary) {
    process.exit(0);
  }
  // eslint-disable-next-line no-console
  console.error("Missing native binaries for any supported target.");
  process.exit(1);
}

if (requestedLabels && groupsToCheck.length === 0) {
  const validLabels = requiredGroups.map((group) => group.label).join(", ");
  // eslint-disable-next-line no-console
  console.error(
    `CODEX_INFINITY_REQUIRED_GROUPS did not match known targets. Valid values: ${validLabels} or "any".`,
  );
  process.exit(1);
}

const missingGroups = groupsToCheck.filter(
  (group) => !group.triples.some((triple) => existsSync(binaryPathFor(triple))),
);

if (missingGroups.length === 0) {
  process.exit(0);
}

const missingLabels = missingGroups.map((group) => group.label).join(", ");
// eslint-disable-next-line no-console
console.error(`Missing native binaries for: ${missingLabels}`);
// eslint-disable-next-line no-console
console.error(`Expected codex binaries under: ${vendorRoot}`);
// eslint-disable-next-line no-console
console.error("Populate vendor/ before publishing:");
// eslint-disable-next-line no-console
console.error("  python3 scripts/install_native_deps.py --component codex --component rg");
// eslint-disable-next-line no-console
console.error("Note: this requires the GitHub CLI (gh) and network access.");
// eslint-disable-next-line no-console
console.error(
  "To publish a subset, set CODEX_INFINITY_REQUIRED_GROUPS=linux-x64 (or comma-separated labels).",
);
process.exit(1);
