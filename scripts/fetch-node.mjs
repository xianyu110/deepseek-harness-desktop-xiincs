// Downloads the official Node.js Windows x64 binary (node.exe only) into
// src-tauri/resources/runtime/ so packaged builds can run without a system
// Node.js. This is part of the "bundled runtime" milestone.
//
// Usage: node scripts/fetch-node.mjs [version]   (default: v22.14.0)

import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const outDir = join(root, "src-tauri", "resources", "runtime");
mkdirSync(outDir, { recursive: true });

const version = process.argv[2] ?? "v22.14.0";
const base = `node-${version}-win-x64`;
const zipUrl = `https://nodejs.org/dist/${version}/${base}.zip`;
const zipPath = join(tmpdir(), `${base}.zip`);
const extractDir = join(tmpdir(), `dsh-node-${version}`);
const outExe = join(outDir, "node.exe");

if (existsSync(outExe)) {
  console.log(`node.exe already present: ${outExe}`);
  process.exit(0);
}

function powershell(script) {
  execFileSync("powershell", ["-NoProfile", "-NonInteractive", "-Command", script], {
    stdio: "inherit",
  });
}

let ok = false;
for (let attempt = 1; attempt <= 5 && !ok; attempt++) {
  console.log(`Downloading ${zipUrl} (attempt ${attempt}/5) …`);
  try {
    powershell(
      `$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -Uri '${zipUrl}' -OutFile '${zipPath}'`,
    );
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

console.log("Extracting node.exe …");
rmSync(extractDir, { recursive: true, force: true });
powershell(
  `Expand-Archive -LiteralPath '${zipPath}' -DestinationPath '${extractDir}' -Force`,
);
copyFileSync(join(extractDir, base, "node.exe"), outExe);
rmSync(zipPath, { force: true });

// sanity check
execFileSync(outExe, ["--version"], { stdio: "inherit" });
console.log(`node.exe → ${outExe}`);
