// Installs the @deepseek-ai/dsh production dependency tree into
// src-tauri/resources/runtime so packaged builds can serve the harness
// without a network connection or a system npm. Part of the "bundled runtime"
// milestone.
//
// Usage: node scripts/prepare-runtime.mjs
// Env:   DSH_DESKTOP_DSH_VERSION  npm version spec (default 0.1.0-rc.6)

import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const runtimeDir = join(root, "src-tauri", "resources", "runtime");
const version = process.env.DSH_DESKTOP_DSH_VERSION ?? "0.1.0-rc.6";

console.log(`Installing @deepseek-ai/dsh@${version} → ${runtimeDir}`);
execFileSync(
  "npm",
  [
    "install",
    "--prefix",
    runtimeDir,
    `@deepseek-ai/dsh@${version}`,
    "--omit=dev",
    "--no-audit",
    "--no-fund",
    "--no-progress",
  ],
  { stdio: "inherit" },
);
console.log("Runtime ready. Add the runtime dir to tauri.conf.json bundle.resources to ship it.");
