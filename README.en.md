# deepseek-harness-desktop

[中文](README.md) | English

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Latest release](https://img.shields.io/github/v/release/xiincs/deepseek-harness-desktop)](https://github.com/xiincs/deepseek-harness-desktop/releases/latest)
[![Platform: Windows](https://img.shields.io/badge/platform-Windows-0078D6)](#)
[![Platform: macOS](https://img.shields.io/badge/platform-macOS-000000)](#)
[![Platform: Linux](https://img.shields.io/badge/platform-Linux-FCC624)](#)
[![Built with Tauri 2](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB)](https://v2.tauri.app/)

A desktop app for [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness), built with
[Tauri 2](https://v2.tauri.app/). The Windows installer is signed and wired into auto-update;
macOS (`.dmg`) and Linux (`.deb`) builds are also published with every release, but are
**unsigned and unnotarized** (no Apple Developer account) — macOS needs a one-time manual
allow in System Settings → Privacy & Security, Linux installs normally via `dpkg -i` or your
package manager, and neither participates in auto-update — you'll need to download new
versions manually.

It wraps the harness's own web server (`dsh web`) in a native window: the app spawns a local
`dsh` server process, waits for it to be ready, and points an embedded WebView2 at it — the same
UI you get by opening `http://127.0.0.1:3080` in a browser, with the same data under `~/.dsh`.

## Screenshots

<p align="center">
  <img src="docs/screenshots/app-boot.png" width="480" alt="Boot page while dsh starts">
  &nbsp;&nbsp;
  <img src="docs/screenshots/app-running.png" width="480" alt="DeepSeek Harness running inside the desktop app">
</p>

## Features

- One-click start: launches the `dsh` web server and loads the harness UI automatically.
- Shared data: sessions, storage and configuration live under `~/.dsh` (`$DSH_HOME`), exactly like
  the browser version.
- Smart ports: if a `dsh` server is already listening on `127.0.0.1:3080` the app attaches to it
  instead of starting a second instance; if the port is taken by something else it falls back to an
  OS-assigned port.
- Native menu & tray: left-click the tray icon to show the window, right-click for the menu (open
  the UI in your default browser, restart the server, reveal the data directory, quit).
- Tray-resident: closing the window hides it instead of stopping the server; only "退出"
  (quit) from the menu/tray actually exits.
- No console flashes: every spawned child process (the `dsh` server, `npm install` on first run)
  runs with its console window suppressed — only the app window itself is visible.
- Crash recovery: an unexpected server exit is restarted once automatically, then surfaced on a
  retry screen with logs.
- Auto-update: the boot page checks for a newer release on startup and offers to install it.
- Minimal attack surface: the harness page runs as a plain remote page in the webview and is given
  **no** Tauri IPC access.

## Prerequisites (development)

- [Rust](https://rustup.rs/) (MSVC toolchain) — for the Tauri shell
- [Node.js](https://nodejs.org/) >= 22 — required by `dsh` itself (the app locates it on `PATH`)
- [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/) (preinstalled on
  Windows 11 / most Windows 10)

## Development

```bash
npm install          # installs @tauri-apps/cli
npm run tauri dev    # builds the Rust shell and opens the app window
```

On first launch the app installs the `@deepseek-ai/dsh` npm package into a per-user runtime
directory (`%LOCALAPPDATA%\dev.dsh.desktop\runtime`) and starts it. The install is cached by npm,
so it is fast and offline after the first run.

### Environment overrides

| Variable | Purpose |
|---|---|
| `DSH_DESKTOP_NODE` | Absolute path to `node.exe` to use instead of the one on `PATH` |
| `DSH_DESKTOP_DSH_BIN` | Absolute path to a `dsh` `lib/bin.js` (e.g. a local checkout) |
| `DSH_DESKTOP_RUNTIME_DIR` | Where the managed `@deepseek-ai/dsh` runtime is installed (default: app cache dir); point it at an existing `node_modules` root to skip the first-run npm install |
| `DSH_DESKTOP_DSH_VERSION` | npm version spec for the managed runtime (default `0.1.0-rc.6`) |
| `DSH_DESKTOP_PORT` | Default bind port override (default `3080`); handy for running several instances |
| `DSH_DESKTOP_CWD` | Working directory for the `dsh` server process (default: user home) |
| `DSH_HOME` | Passed through to the server; harness data root (default `~/.dsh`) |

## Architecture

```
┌─ Tauri app (Rust, WebView2) ─────────────────────────────┐
│ local boot page (loading / error / retry)                │
│   └─ navigates to → http://127.0.0.1:<port> (the UI)     │
│ server manager (src-tauri/src/server.rs)                 │
│   locate node → install/verify dsh runtime → probe 3080  │
│   → spawn `node dsh web --port …` → parse stdout URL     │
│   → navigate → watch process → taskkill tree on exit     │
│ native menu & tray (src-tauri/src/menu.rs)               │
└─────────────────────────┬────────────────────────────────┘
                          │ spawn
                 ┌────────▼────────┐
                 │  dsh web server │  data → ~/.dsh (DSH_HOME)
                 └─────────────────┘
```

The harness page is loaded from `http://127.0.0.1:<port>` and is intentionally **not** granted
Tauri IPC access (`dangerousRemoteDomainIpcAccess` is never enabled), so the web UI cannot reach
the shell — every shell action goes through the native menu/tray or the local boot page.

## Roadmap

- [x] Scaffold, server manager, menu/tray, crash recovery
- [x] Persistent log file (`%LOCALAPPDATA%\dev.dsh.desktop\logs\desktop.log`) + live logs on the boot page
- [x] Bundled runtime: `npm run bundle` ships `node.exe` (Node 24 — the harness's own
      `engines.node` is `^22.19.0 || >=24.0.0`; Node 23 is intentionally excluded upstream as an
      EOL/non-LTS line) + the `dsh` node_modules inside the NSIS installer, so the app runs on
      machines without Node.js
- [x] Tray-resident mode: closing the window hides it and leaves the server running; a one-time
      notification explains this on first close each run. Only the menu/tray "退出" action stops
      the server and exits
- [x] Auto-update (`tauri-plugin-updater`): the boot page checks on startup and shows a
      dismissible banner; `.github/workflows/release.yml` builds, signs and drafts a GitHub
      Release on every `v*` tag push (a human still publishes it — auto-update amplifies the
      blast radius of a bad release, so nothing goes live unattended). See
      [Two version axes](#two-version-axes) below for how this interacts with the bundled `dsh`
      runtime version.
- [x] macOS / Linux packaging: `server.rs`'s Unix branches, `fetch-node.mjs`'s darwin/linux
      download paths, and `prepare-runtime.mjs`'s runtime install are all verified on GitHub
      Actions `macos-latest`/`ubuntu-latest` runners (see
      [.github/workflows/ci.yml](.github/workflows/ci.yml)); `tauri build --bundles dmg`/`--bundles
      deb` produce real installable artifacts and are wired into
      [release.yml](.github/workflows/release.yml). What's still missing is code signing and
      notarization — needs an Apple Developer account this project doesn't have, so these two
      platforms' builds ship unsigned (see the note above)

## Building the installer

```bash
npm install
npm run bundle        # fetch:node → prepare:runtime → tauri build
# or step by step:
npm run fetch:node    # downloads node.exe → src-tauri/resources/runtime
DSH_RUNTIME_SOURCE=<node_modules root> npm run prepare:runtime  # copy a local runtime instead of npm install
npm run build         # → src-tauri/target/release/bundle/nsis/DeepSeek Harness_0.3.0_x64-setup.exe
```

Before cutting a release, run `npm run check:dsh-version` — upstream is in developer preview and
publishes new RCs without notice; this checks the pinned `@deepseek-ai/dsh` default (duplicated in
`src-tauri/src/server.rs` and `scripts/prepare-runtime.mjs`, they must agree) against npm's latest.
The release workflow runs this same check and fails the build on a mismatch.

### Two version axes

This app has two independent version numbers that must not be conflated:

- **Shell version** (`tauri.conf.json`'s `version`, e.g. `0.3.0`) — the desktop wrapper itself.
  `tauri-plugin-updater` only updates this.
- **Runtime version** (`DSH_VERSION_DEFAULT` in `server.rs` / the default in
  `prepare-runtime.mjs`, e.g. `0.1.0-rc.6`) — the pinned `@deepseek-ai/dsh` release bundled
  inside the installer or installed on first use.

**For a bundled-runtime install (the default, `npm run bundle`)** these travel together
automatically: the NSIS installer's payload includes `resources/runtime/`, so a shell
auto-update reinstalls the runtime pinned at build time along with it — there's no separate
runtime-update mechanism to build as long as `DSH_VERSION_DEFAULT` is bumped (and
`check:dsh-version` passes) before cutting each shell release.

**For the managed (non-bundled) runtime path** — used when there's no `resources/runtime/`
(e.g. an unpackaged dev build, or `DSH_DESKTOP_RUNTIME_DIR` pointed elsewhere) — the runtime is
installed once via `npm install` on first use ([server.rs](src-tauri/src/server.rs)'s
`install_runtime`) and **never re-checked afterward**. A user on this path who wants a newer
`dsh` has to clear `DSH_DESKTOP_RUNTIME_DIR` (or set `DSH_DESKTOP_DSH_VERSION` to a newer spec)
and let it reinstall. This is a known, narrow gap — not worth a bespoke updater for a path that's
mainly used in development.

## License

[MIT](./LICENSE)
