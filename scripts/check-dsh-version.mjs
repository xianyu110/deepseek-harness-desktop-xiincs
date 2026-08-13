// Checks the hardcoded @deepseek-ai/dsh default version against npm's latest
// published version, and cross-checks the two source-of-truth locations
// (server.rs, prepare-runtime.mjs) against each other. Run this by hand
// before cutting a release; upstream is still in developer preview and
// publishes new RCs without notice.
//
// Usage: node scripts/check-dsh-version.mjs
// Exits non-zero if the pinned defaults disagree with each other, or if a
// newer version is available on npm than what's pinned.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));

function extract(path, pattern) {
  const text = readFileSync(join(root, path), "utf8");
  const match = text.match(pattern);
  if (!match) {
    console.error(`Could not find a version default in ${path}`);
    process.exit(1);
  }
  return match[1];
}

const serverRsVersion = extract(
  "src-tauri/src/server.rs",
  /const DSH_VERSION_DEFAULT: &str = "([^"]+)"/,
);
const prepareRuntimeVersion = extract(
  "scripts/prepare-runtime.mjs",
  /const version = process\.env\.DSH_DESKTOP_DSH_VERSION \?\? "([^"]+)"/,
);

console.log(`server.rs pins:            ${serverRsVersion}`);
console.log(`prepare-runtime.mjs pins:  ${prepareRuntimeVersion}`);

let failed = false;

if (serverRsVersion !== prepareRuntimeVersion) {
  console.error(
    `\nMismatch: server.rs and prepare-runtime.mjs pin different default versions.\n` +
      `Both must agree — a packaged build uses prepare-runtime.mjs's default, but the\n` +
      `managed per-user runtime install (no bundled resources) uses server.rs's default.`,
  );
  failed = true;
}

console.log("\nChecking npm for newer published versions...");
let versions;
try {
  // `npm` resolves inconsistently as a direct execFileSync target across
  // Windows npm installs (plain PATH npm vs. nvm-managed shims); `shell: true`
  // sidesteps that the same way a user's own shell would. The command has no
  // interpolated input, so the shell-escaping caveat that comes with
  // `shell: true` doesn't apply here.
  const raw = execFileSync("npm view @deepseek-ai/dsh versions --json", {
    encoding: "utf8",
    shell: true,
  });
  versions = JSON.parse(raw);
} catch (err) {
  console.error(`Could not query npm registry: ${err.message}`);
  process.exit(1);
}

const latest = versions.at(-1);
console.log(`npm's latest published version: ${latest}`);
console.log(`All published versions: ${versions.join(", ")}`);

if (latest !== serverRsVersion) {
  console.error(
    `\nPinned default (${serverRsVersion}) is not npm's latest published version (${latest}).\n` +
      `Upstream is in developer preview and iterates fast — review the changelog before bumping,\n` +
      `then update DSH_VERSION_DEFAULT in src-tauri/src/server.rs and the default in\n` +
      `scripts/prepare-runtime.mjs (and the docs in README.md) together.`,
  );
  failed = true;
}

if (failed) {
  process.exit(1);
}
console.log("\nOK: pinned defaults agree and match npm's latest.");
