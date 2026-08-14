// Builds the tauri-plugin-updater `latest.json` manifest by hand.
//
// Why this exists: tauri-apps/tauri-action has a long-standing, still-open
// bug where a space in `productName` ("DeepSeek Harness") breaks its own
// latest.json generation — it normalizes the uploaded asset name (spaces →
// dots) but doesn't apply the same normalization when matching that asset
// against the .sig file it just signed, so it logs "Signature not found for
// the updater JSON. Skipping upload..." and skips it entirely. Confirmed
// against the action's own issue tracker (tauri-apps/tauri-action#345, #843,
// #860 — same symptom, still unresolved as of this writing). The .sig files
// themselves are generated correctly; only the action's matching logic is
// broken. Renaming productName would fix the upstream bug but breaks the
// installed-directory continuity for existing installs (Program Files\<name>),
// so this script reads the correctly-signed artifacts directly instead.
//
// Usage: node scripts/build-latest-json.mjs <version> <releaseTag>
// Reads: src-tauri/target/release/bundle/nsis/*.exe.sig
// Writes: latest.json (repo root)

import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const [, , version, releaseTag] = process.argv;
if (!version || !releaseTag) {
  console.error("Usage: node scripts/build-latest-json.mjs <version> <releaseTag>");
  process.exit(1);
}

// Find the single *.exe.sig in the nsis bundle dir (there's exactly one —
// this project only builds one NSIS installer per release).
const nsisDir = join(root, "src-tauri", "target", "release", "bundle", "nsis");
const entries = existsSync(nsisDir) ? readdirSync(nsisDir) : [];
const sigName = entries.find((f) => f.endsWith(".exe.sig"));
if (!sigName) {
  console.error(`No .exe.sig found in ${nsisDir}. Entries: ${entries.join(", ") || "(none)"}`);
  process.exit(1);
}
const exeName = sigName.slice(0, -".sig".length);
const signature = readFileSync(join(nsisDir, sigName), "utf8").trim();

// GitHub normalizes uploaded asset filenames: spaces (and () [] {}) become
// dots. This must match tauri-action's own normalization exactly, or the
// URL below 404s once uploaded.
const uploadedName = exeName.replace(/[ ()[\]{}]/g, ".").replace(/\.\./g, ".");

const url = `https://github.com/xiincs/deepseek-harness-desktop/releases/download/${releaseTag}/${uploadedName}`;

const manifest = {
  version,
  notes: "",
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": { signature, url },
  },
};

writeFileSync(join(root, "latest.json"), JSON.stringify(manifest, null, 2));
console.log(`latest.json written: ${url}`);
