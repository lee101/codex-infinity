#!/usr/bin/env node
// Unified entry point for Codex Infinite CLI.

import { spawn } from "node:child_process";
import { existsSync } from "fs";
import path from "path";
import { fileURLToPath } from "url";

// __dirname equivalent in ESM
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const { platform, arch } = process;

function prefersGnu() {
  if (platform !== "linux") {
    return false;
  }
  try {
    const report = process.report?.getReport?.();
    return Boolean(report?.header?.glibcVersionRuntime);
  } catch {
    return false;
  }
}

let targetTriples = [];
switch (platform) {
  case "linux":
  case "android": {
    const preferGnu = prefersGnu();
    switch (arch) {
      case "x64":
        targetTriples = preferGnu
          ? ["x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl"]
          : ["x86_64-unknown-linux-musl", "x86_64-unknown-linux-gnu"];
        break;
      case "arm64":
        targetTriples = preferGnu
          ? ["aarch64-unknown-linux-gnu", "aarch64-unknown-linux-musl"]
          : ["aarch64-unknown-linux-musl", "aarch64-unknown-linux-gnu"];
        break;
      default:
        break;
    }
    break;
  }
  case "darwin":
    switch (arch) {
      case "x64":
        targetTriples = ["x86_64-apple-darwin"];
        break;
      case "arm64":
        targetTriples = ["aarch64-apple-darwin"];
        break;
      default:
        break;
    }
    break;
  case "win32":
    switch (arch) {
      case "x64":
        targetTriples = ["x86_64-pc-windows-msvc"];
        break;
      case "arm64":
        targetTriples = ["aarch64-pc-windows-msvc"];
        break;
      default:
        break;
    }
    break;
  default:
    break;
}

if (targetTriples.length === 0) {
  throw new Error(`Unsupported platform: ${platform} (${arch})`);
}

const vendorRoot = path.join(__dirname, "..", "vendor");
const binaryName = process.platform === "win32" ? "codex.exe" : "codex";

// Find the first available binary
let binaryPath = null;
let archRoot = null;
let selectedTriple = null;
for (const triple of targetTriples) {
  archRoot = path.join(vendorRoot, triple);
  const candidatePath = path.join(archRoot, "codex", binaryName);
  if (existsSync(candidatePath)) {
    binaryPath = candidatePath;
    selectedTriple = triple;
    break;
  }
}

if (!binaryPath) {
  const tried = targetTriples.join(", ");
  throw new Error(
    `No binary found for platform: ${platform} (${arch}). Tried: ${tried}. ` +
      "This usually means the npm package was published without native binaries. " +
      "Reinstall from a fixed release or ask the publisher to populate vendor/ before publishing.",
  );
}

// If we're on Linux and do not appear to be running glibc, avoid attempting to
// execute a GNU (glibc-linked) binary since it will likely fail with a vague
// ENOENT/dynamic-linker error.
if (
  (platform === "linux" || platform === "android") &&
  !prefersGnu() &&
  selectedTriple?.includes("-gnu")
) {
  throw new Error(
    "This release does not include a musl build of Codex. " +
      "Your system does not appear to be running glibc, so the bundled GNU binary " +
      "will not run. Use a glibc-based Linux distribution or install a musl-compatible build.",
  );
}

// Use an asynchronous spawn instead of spawnSync so that Node is able to
// respond to signals (e.g. Ctrl-C / SIGINT) while the native binary is
// executing. This allows us to forward those signals to the child process
// and guarantees that when either the child terminates or the parent
// receives a fatal signal, both processes exit in a predictable manner.

function getUpdatedPath(newDirs) {
  const pathSep = process.platform === "win32" ? ";" : ":";
  const existingPath = process.env.PATH || "";
  const updatedPath = [
    ...newDirs,
    ...existingPath.split(pathSep).filter(Boolean),
  ].join(pathSep);
  return updatedPath;
}

/**
 * Use heuristics to detect the package manager that was used to install Codex
 * in order to give the user a hint about how to update it.
 */
function detectPackageManager() {
  const userAgent = process.env.npm_config_user_agent || "";
  if (/\bbun\//.test(userAgent)) {
    return "bun";
  }

  const execPath = process.env.npm_execpath || "";
  if (execPath.includes("bun")) {
    return "bun";
  }

  if (
    __dirname.includes(".bun/install/global") ||
    __dirname.includes(".bun\\install\\global")
  ) {
    return "bun";
  }

  return userAgent ? "npm" : null;
}

const additionalDirs = [];
const pathDir = path.join(archRoot, "path");
if (existsSync(pathDir)) {
  additionalDirs.push(pathDir);
}
const updatedPath = getUpdatedPath(additionalDirs);

const env = { ...process.env, PATH: updatedPath };
const packageManagerEnvVar =
  detectPackageManager() === "bun"
    ? "CODEX_INFINITE_MANAGED_BY_BUN"
    : "CODEX_INFINITE_MANAGED_BY_NPM";
env[packageManagerEnvVar] = "1";

const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  env,
});

child.on("error", (err) => {
  // Typically triggered when the binary is missing or not executable.
  // Re-throwing here will terminate the parent with a non-zero exit code
  // while still printing a helpful stack trace.
  // eslint-disable-next-line no-console
  console.error(err);
  process.exit(1);
});

// Forward common termination signals to the child so that it shuts down
// gracefully. In the handler we temporarily disable the default behavior of
// exiting immediately; once the child has been signaled we simply wait for
// its exit event which will in turn terminate the parent (see below).
const forwardSignal = (signal) => {
  if (child.killed) {
    return;
  }
  try {
    child.kill(signal);
  } catch {
    /* ignore */
  }
};

["SIGINT", "SIGTERM", "SIGHUP"].forEach((sig) => {
  process.on(sig, () => forwardSignal(sig));
});

// When the child exits, mirror its termination reason in the parent so that
// shell scripts and other tooling observe the correct exit status.
// Wrap the lifetime of the child process in a Promise so that we can await
// its termination in a structured way. The Promise resolves with an object
// describing how the child exited: either via exit code or due to a signal.
const childResult = await new Promise((resolve) => {
  child.on("exit", (code, signal) => {
    if (signal) {
      resolve({ type: "signal", signal });
    } else {
      resolve({ type: "code", exitCode: code ?? 1 });
    }
  });
});

if (childResult.type === "signal") {
  // Re-emit the same signal so that the parent terminates with the expected
  // semantics (this also sets the correct exit code of 128 + n).
  process.kill(process.pid, childResult.signal);
} else {
  process.exit(childResult.exitCode);
}
