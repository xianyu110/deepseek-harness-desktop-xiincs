# Release — DeepSeek Harness Desktop v0.1.0

> Tag: `v0.1.0`  ·  Repo: `deepseek-harness-desktop`

---

## What's new

**DeepSeek Harness Desktop** is a native Windows desktop app for
[DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness), built with
[Tauri 2](https://v2.tauri.app/). It hosts the harness's own web server
(`dsh web`) inside a desktop window — no browser tab needed.

### Highlights

- **One-click start** — launches the `dsh` server and loads the harness UI automatically.
- **Shared data** — sessions, storage and settings live in `~/.dsh` (`$DSH_HOME`),
  identical to the browser version.
- **Self-contained installer** — bundles Node.js 24 + the `dsh` runtime, so it runs on
  machines that don't have Node.js installed.
- **Smart port handling** — attaches to an already-running harness on `127.0.0.1:3080`
  instead of starting a second instance (no racing on `~/.dsh`); falls back to an
  OS-assigned port when 3080 is taken by something else.
- **Native menu & tray** — open the UI in your default browser, restart the server,
  reveal the data directory, quit.
- **Crash recovery** — one automatic restart per 60s window, then a clear error screen
  with retry and logs.
- **Small attack surface** — the harness page runs as a plain remote page and is granted
  **no** Tauri IPC access; all shell actions go through the native menu/tray.
- **Observable** — live logs on the boot page, persistent log file
  (`%LOCALAPPDATA%\dev.dsh.desktop\logs\desktop.log`).

## Downloads

| Asset | Size | SHA256 |
|---|---|---|
| `DeepSeek Harness_0.1.0_x64-setup.exe` | 53.9 MB | `E95E7447CE080EDB12D088A1A717BBD3DDF365633202E76CBE4D988484A2B62A` |

> Windows 10/11 (x64). Requires the WebView2 runtime — preinstalled on Windows 11,
> auto-prompted by the installer on Windows 10.

## First run

1. Run the installer and launch **DeepSeek Harness**.
2. The app starts the local server and opens the harness UI.
3. Your existing browser sessions under `~/.dsh` appear immediately — same data, same UI.

No Node.js needed: the installer ships its own Node 24 runtime.

## Power-user environment variables

| Variable | Purpose |
|---|---|
| `DSH_DESKTOP_NODE` | Absolute path to `node.exe` (overrides the bundled one) |
| `DSH_DESKTOP_DSH_BIN` | Absolute path to a `dsh` `lib/bin.js` (e.g. a local checkout) |
| `DSH_DESKTOP_RUNTIME_DIR` | Where the managed `@deepseek-ai/dsh` runtime lives |
| `DSH_DESKTOP_DSH_VERSION` | npm version spec for the managed runtime (default `0.1.0-rc.6`) |
| `DSH_DESKTOP_PORT` | Default bind port override (default `3080`) |
| `DSH_DESKTOP_CWD` | Working directory for the `dsh` server process (default: user home) |
| `DSH_HOME` | Passed through to the server; harness data root (default `~/.dsh`) |

## Known limitations

- Windows-only in this release (Tauri is cross-platform; macOS/Linux packaging is on the roadmap).
- Closing the window stops the server — tray-resident mode is not implemented yet.
- No auto-update yet.

## What's next

- [ ] Tray-resident mode (close to tray keeps the server running)
- [ ] Auto-update (`tauri-plugin-updater`)
- [ ] macOS / Linux builds
- [ ] Per-session workspace picker polish

## Acknowledgements

- [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness) — the agent harness this app wraps
- [Tauri](https://tauri.app/) — the desktop shell
- MIT license

---

## 中文版

**DeepSeek Harness 桌面版 v0.1.0**：基于 Tauri 2 的 Windows 原生桌面应用，把 DeepSeek Harness 的
Web 服务（`dsh web`）装进桌面窗口。

### 主要特性

- **一键启动**：自动拉起 `dsh` 服务并加载界面，无需浏览器标签页
- **数据共享**：会话/存储/配置沿用 `~/.dsh`（`$DSH_HOME`），与浏览器版完全一致
- **免装 Node**：安装包内置 Node.js 24 与 dsh 运行时，裸机可跑
- **端口智能**：3080 已有 harness 在跑时直接挂接（避免双实例争写数据）；被其他程序占用时自动换端口
- **原生菜单/托盘**：在浏览器打开、重启服务、打开数据目录、退出
- **崩溃恢复**：60 秒窗口内自动重启一次，失败后给出错误页 + 重试 + 日志
- **安全面小**：harness 页面是纯远程页面，不开放任何 Tauri IPC
- **可观测**：启动页实时日志 + 持久化日志文件（`%LOCALAPPDATA%\dev.dsh.desktop\logs\desktop.log`）

### 下载

| 安装包 | 大小 | SHA256 |
|---|---|---|
| `DeepSeek Harness_0.1.0_x64-setup.exe` | 53.9 MB | `E95E7447CE080EDB12D088A1A717BBD3DDF365633202E76CBE4D988484A2B62A` |

### 已知限制

- 当前仅支持 Windows（Tauri 本身跨平台，macOS/Linux 打包在路线图中）
- 关闭窗口会停止服务，托盘常驻模式尚未实现
- 暂无自动更新

### 致谢

- [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness)
- [Tauri](https://tauri.app/)
- MIT License
