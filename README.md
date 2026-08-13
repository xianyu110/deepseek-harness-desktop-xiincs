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
| `DSH_DESKTOP_RUNTIME_DIR` | Where the managed `@deepseek-ai/dsh` runtime is installed |
| `DSH_DESKTOP_DSH_VERSION` | npm version spec for the managed runtime (default `0.1.0-rc.6`) |
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
- [ ] Bundled runtime: ship `node.exe` + the `dsh` node_modules inside the installer, so the app
      runs on machines without Node.js (`scripts/fetch-node.mjs`, `scripts/prepare-runtime.mjs`)
- [ ] NSIS/MSI installers via `tauri build`
- [ ] Tray-resident mode (close to tray keeps the server running)
- [ ] Auto-update (`tauri-plugin-updater`)

## License

[MIT](./LICENSE)
