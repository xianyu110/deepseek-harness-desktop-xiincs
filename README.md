# dsh-desktop

A desktop app for [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness), built with [Tauri 2](https://v2.tauri.app/).

It wraps the harness's own web server (`dsh web`) in a native window: the app spawns a local
`dsh` server process, waits for it to be ready, and points an embedded WebView2 at it — the same
UI you get by opening `http://127.0.0.1:3080` in a browser, with the same data under `~/.dsh`.

## Features

- One-click start: launches the `dsh` web server and loads the harness UI automatically.
- Shared data: sessions, storage and configuration live under `~/.dsh` (`$DSH_HOME`), exactly like
  the browser version.
- Smart ports: if a `dsh` server is already listening on `127.0.0.1:3080` the app attaches to it
  instead of starting a second instance; if the port is taken by something else it falls back to an
  OS-assigned port.
- Native menu & tray: open the UI in your default browser, restart the server, reveal the data
  directory, quit.
- Crash recovery: an unexpected server exit is restarted once automatically, then surfaced on a
  retry screen with logs.
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
- [ ] Tray-resident mode (close to tray keeps the server running)
- [ ] Auto-update (`tauri-plugin-updater`)

## Building the installer

```bash
npm install
npm run bundle        # fetch:node → prepare:runtime → tauri build
# or step by step:
npm run fetch:node    # downloads node.exe → src-tauri/resources/runtime
DSH_RUNTIME_SOURCE=<node_modules root> npm run prepare:runtime  # copy a local runtime instead of npm install
npm run build         # → src-tauri/target/release/bundle/nsis/DeepSeek Harness_0.1.0_x64-setup.exe
```

Before cutting a release, run `npm run check:dsh-version` — upstream is in developer preview and
publishes new RCs without notice; this checks the pinned `@deepseek-ai/dsh` default (duplicated in
`src-tauri/src/server.rs` and `scripts/prepare-runtime.mjs`, they must agree) against npm's latest.

## License

[MIT](./LICENSE)
