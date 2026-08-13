// Downloads the official Node.js binary (node executable only) into
// src-tauri/resources/runtime/ so packaged builds can run without a system
// Node.js. This is part of the "bundled runtime" milestone.
//
// NOTE: the harness's own `engines.node` is `^22.19.0 || >=24.0.0` (Node 23 is
// intentionally excluded upstream — it's a non-LTS/EOL line, not a feature
// floor). We bundle Node 24.x LTS here for headroom above the current floor.
//
// The macOS/Linux branches below are untested — this project has shipped only
// a Windows (NSIS) build so far, and there's no Mac/Linux hardware or CI here
// to verify them against. They exist so this script matches server.rs's
// platform-generic shape; treat them as a starting point, not a verified path.
//
// Usage: node scripts/fetch-node.mjs [version]   (default: v24.9.0)

import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const outDir = join(root, "src-tauri", "resources", "runtime");
mkdirSync(outDir, { recursive: true });

const version = process.argv[2] ?? "v24.9.0";

const PLATFORM_TABLE = {
  win32: { plat: "win", ext: "zip" },
  darwin: { plat: "darwin", ext: "tar.gz" },
  linux: { plat: "linux", ext: "tar.gz" },
};
const ARCH_TABLE = { x64: "x64", arm64: "arm64" };

const platEntry = PLATFORM_TABLE[process.platform];
const arch = ARCH_TABLE[process.arch];
if (!platEntry || !arch) {
  console.error(`Unsupported platform/arch: ${process.platform}/${process.arch}`);
  process.exit(1);
}
const { plat, ext } = platEntry;
const isWindows = process.platform === "win32";

const base = `node-${version}-${plat}-${arch}`;
const archiveUrl = `https://nodejs.org/dist/${version}/${base}.${ext}`;
const archivePath = join(tmpdir(), `${base}.${ext}`);
const extractDir = join(tmpdir(), `dsh-node-${version}-${plat}-${arch}`);
const nodeBinName = isWindows ? "node.exe" : "node";
const outExe = join(outDir, nodeBinName);

if (existsSync(outExe)) {
  console.log(`${nodeBinName} already present: ${outExe}`);
  process.exit(0);
}

function powershell(script) {
  execFileSync("powershell", ["-NoProfile", "-NonInteractive", "-Command", script], {
    stdio: "inherit",
  });
}

function downloadOnce() {
  if (isWindows) {
    powershell(
      `$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -Uri '${archiveUrl}' -OutFile '${archivePath}'`,
    );
  } else {
    // curl ships on macOS by default and on most Linux distros.
    execFileSync("curl", ["-fsSL", "-o", archivePath, archiveUrl], { stdio: "inherit" });
  }
}

let ok = false;
for (let attempt = 1; attempt <= 5 && !ok; attempt++) {
  console.log(`Downloading ${archiveUrl} (attempt ${attempt}/5) …`);
  try {
    downloadOnce();
    ok = true;
  } catch (err) {
    console.error(`download failed: ${err.message}`);
    if (attempt < 5) await new Promise((r) => setTimeout(r, 5000));
  }
}
if (!ok) {
  console.error("Failed to download Node.js after 5 attempts");
  process.exit(1);
}

console.log(`Extracting ${nodeBinName} …`);
rmSync(extractDir, { recursive: true, force: true });
mkdirSync(extractDir, { recursive: true });
// The Windows zip lays out <base>/node.exe directly; the official
// macOS/Linux tar.gz nests the binary one level deeper, at <base>/bin/node.
let extractedBin;
if (isWindows) {
  powershell(`Expand-Archive -LiteralPath '${archivePath}' -DestinationPath '${extractDir}' -Force`);
  extractedBin = join(extractDir, base, "node.exe");
} else {
  execFileSync("tar", ["-xzf", archivePath, "-C", extractDir], { stdio: "inherit" });
  extractedBin = join(extractDir, base, "bin", "node");
}
copyFileSync(extractedBin, outExe);
if (!isWindows) {
  execFileSync("chmod", ["+x", outExe]);
}
rmSync(archivePath, { force: true });

// sanity check
execFileSync(outExe, ["--version"], { stdio: "inherit" });
console.log(`${nodeBinName} → ${outExe}`);
