// Downloads the official Node.js Windows x64 binary (node.exe only) into
// src-tauri/resources/runtime/ so packaged builds can run without a system
// Node.js. This is part of the "bundled runtime" milestone.
//
// Usage: node scripts/fetch-node.mjs [version]   (default: v22.14.0)

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, rmSync } from "node:fs";
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

console.log(`Downloading ${zipUrl} …`);
execFileSync("powershell", [
  "-NoProfile",
  "-Command",
  `Invoke-WebRequest -Uri '${zipUrl}' -OutFile '${zipPath}'`,
], { stdio: "inherit" });

console.log(`Extracting node.exe …`);
rmSync(extractDir, { recursive: true, force: true });
execFileSync("powershell", [
  "-NoProfile",
  "-Command",
  `Expand-Archive -LiteralPath '${zipPath}' -DestinationPath '${extractDir}' -Force`,
], { stdio: "inherit" });

const nodeExe = join(extractDir, base, "node.exe");
copyFileSync(nodeExe, join(outDir, "node.exe"));
rmSync(zipPath, { force: true });
console.log(`node.exe → ${join(outDir, "node.exe")}`);
