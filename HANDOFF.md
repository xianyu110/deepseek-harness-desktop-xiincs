# 交接文档 — v1.1.0

本文档面向接手这个项目的下一个人（可能是未来的你自己）。记录这一轮做了什么、为什么这么做、
还有什么没做完以及卡在哪。任务级别的清单在 [TODO.md](TODO.md)，这里是叙事版本 + 决策依据。

## 起因

竞品调研（`anywhere-labs/deepseek-harness-desktop` 等同类 Electron 桌面壳项目）挖出一批同类
项目踩过的坑，加上自己实际跑起来手动排查，一共建了 25 项任务。本文档覆盖的是这一轮全部
工作，从 [b6161e2](../../commit/b6161e2) 到 [8c3d083](../../commit/8c3d083)。

## 现状

- **版本**：外壳版本 `1.0.0 → 1.1.0`（[8c3d083](../../commit/8c3d083)），已打 tag 并推送，
  `.github/workflows/release.yml` 会构建签名并创建**草稿** Release——按项目原有设计，草稿
  发布还需要人工点一下（自动更新影响所有已装用户，不该无人确认就上线），我没有替你点。
- **任务进度**：21/25 完成，4 项因为真实的外部限制没做（不是漏做，见下文"没做完的"）。
- **dsh 运行时版本**：`0.1.0-rc.6`，跑过 `npm run check:dsh-version` 确认与 npm 最新一致，
  这次没有改动运行时版本，只动了外壳。

## 这一轮做了什么

### 阻断级 bug（发布前必须修的那种）

- **右键"后退"卡死启动页**（[ba8ffef](../../commit/ba8ffef)）：WebView2 默认右键菜单没禁用，
  点"后退"会导航回本地启动页，且无法用"前进"恢复，是真死锁。这是最早发现的问题。
- **右键菜单修复的副作用**：上面那个修复一开始用的是
  `SetAreDefaultContextMenusEnabled(false)`，把整个默认右键菜单都关了——结果连"复制"都被
  一起干掉了，是用户实测发现并直接骂出来的（"你把右键菜单删了，用户怎么复制？"）。改成
  `ContextMenuRequested` 事件精确摘除 `back`/`forward`/`reload`/`inspectElement` 四个具名项
  （[334ad75](../../commit/334ad75)），复制/剪切/粘贴/全选都留着。**如果以后还要动右键菜单
  相关的代码，务必记住这个教训：WebView2 的"默认菜单"是复制粘贴和危险导航共用的同一个东西，
  别整个禁用。**
- **单实例重新聚焦失效**（[8ef4c5f](../../commit/8ef4c5f)）：窗口最小化/隐藏后二次启动本该
  拉回窗口，但 `show_main_window()` 在单实例回调所在的线程上调用会静默失败（之前
  `let _ = ...` 把错误吞了，什么都看不出来）。用 `run_on_main_thread` 修复，实测验证过
  修复前/修复后的行为差异。

### 打包环境专属地雷

- **PATH 丢失**（[a5ef27d](../../commit/a5ef27d)）：双击启动的 GUI 进程不像终端进程那样继承
  完整 PATH，agent 调用 pwsh/git 等系统命令会找不到。这个开发模式测不出来，必须在打包产物上
  测。已经从注册表重建 PATH。
- **CI 冒烟测试**（[07d8d2d](../../commit/07d8d2d)）：Windows 版打包产物之前只验证"编译通过"，
  没验证"真的能跑起来"，加了一步真实启动检查。

### 体验/安全加固

- 移除 Windows/Linux 上的经典 Win32 菜单条（macOS 保留，因为全局菜单栏是那边的平台惯例）
  （[b6161e2](../../commit/b6161e2)）。
- 缩放上限 + 生产环境锁 DevTools（[ace2857](../../commit/ace2857)）。
- 深色模式跟随系统（[0439f9f](../../commit/0439f9f)）：现有 CSS 已经是 token 化的，只是给
  `:root` 加一层 `prefers-color-scheme: dark` 覆盖，`--accent`/`--danger` 两个品牌色两套主题
  下保持不变（没有设计师输入的情况下不该自己发明配色）。

### 新功能

- **Explorer 右键"用 DeepSeek Harness 打开"**（[e9c42a4](../../commit/e9c42a4)、
  [77cfe8f](../../commit/77cfe8f)、[1b8e124](../../commit/1b8e124)）：右键文件夹直接以该目录
  为工作区启动。注册表写在 `HKCU\Software\Classes`（不是 `HKLM`/`HKCR`），因为这个安装包默认
  是 per-user 模式、不会请求管理员权限。**这个是唯一一个真实构建了安装包、装上、查注册表、
  卸载、再查注册表确认清理干净的功能**——其余功能都只做到 `cargo test` 级别验证。已经启动的
  实例二次右键另一个文件夹会切换工作目录重启服务（不会丢数据，dsh 按项目路径持久化会话到
  `~/.dsh/sessions/`）。
- 全局快捷键 Ctrl+Alt+D、托盘状态 tooltip、开机自启动
  （[6813e90](../../commit/6813e90)、[95b6a1b](../../commit/95b6a1b)）：都是官方 Tauri 插件
  的薄封装，风险低。

### 拍板的决定（不是没做完，是不做）

- **不接入/预集成第三方插件生态**：把未经审查的代码引入一个本身有文件系统/shell 执行权限的
  agent 工具，是真实的安全面扩大；目前没有任何同类竞品真正安全落地过这件事。如果以后要重新
  评估，先设计插件审核机制，不要图省事直接"接个市场"。

## 没做完的（4 项，都有具体卡点）

| 任务 | 卡点 | 怎么解开 |
|---|---|---|
| mac/Linux 窗口交互核实 | 没有对应系统环境 | 需要真实 mac/Linux 机器测 |
| 右侧文件预览面板 + Git 图谱 | 要么是把整个应用从"单一 webview"重构成"多面板原生壳"的架构级工作量，要么需要 dsh 自己的 HTTP API 文档（未知，harness 是上游项目，没有源码可查） | 先搞清楚 dsh 有没有暴露文件树/git 相关的 HTTP 接口，或者接受这是独立的大项目单独排期 |
| Provider 配置快速导入 | 探查过真实的 `~/.dsh` 目录：`cordis.yml`/`cordis.patch.yml` 都是空的，实际配置是由 `package.json` 的 `dsh.profile.bundles` 驱动的插件组合系统，没有能直接安全读写的扁平文件；配置里大概率含 API key，不该在不了解具体 bundle 插件 patch 格式的情况下猜着写 | 需要具体某个 provider bundle 插件的 patch schema 文档 |
| 长任务完成系统通知 | harness 页面刻意零 Tauri IPC 权限（安全边界，见 `lib.rs` 顶部注释），壳层感知不到单个任务完成；探查过 `~/.dsh/sessions/*/session.jsonl.zstd`，是压缩流文件，实时会被持续写入，要安全监听得新引入 zstd 流式解压依赖 + 处理并发读写正确性 | 要么破坏现有 IPC 隔离边界，要么等 dsh 自己暴露状态接口，要么专门做一个 zstd 流式 tail 的小项目 |

## 这一轮踩的坑（给下一个人省时间）

- **本地开发环境的截图/GUI 自动化工具在这个环境里对某些窗口/弹出菜单不可靠**（会显示错误
  内容，比如截图变成别的应用界面）。反复出现，不是偶发。遇到类似问题别死磕同一个工具，换
  用日志文件、注册表查询、进程状态这类更直接的信号验证，通常更快也更可靠。
- **每次手动测试完 `dsh-desktop.exe` 记得 `taskkill /F /T`（带 `/T`）**，不然子进程
  （dsh 的 node 进程）会被孤立遗留，不会跟着父进程一起死。
- **注册表写入之后不要立刻查**，NSIS 安装流程里 hook 触发点在文件复制之后、流程末尾，`/S`
  静默安装看起来"命令已返回"不代表所有步骤都跑完了；查早了会误判成"没生效"。

## 怎么验证

```bash
cd src-tauri
cargo check --no-default-features   # 快速编译检查
cargo test --no-default-features    # 单元测试（7 个，server.rs 3 个 + lib.rs 4 个）
cargo build --no-default-features   # debug 二进制，target/debug/dsh-desktop.exe
```

打包安装程序验证走 `npm run build`（需要 `src-tauri/resources/runtime` 已存在）→
`target/release/bundle/nsis/*.exe /S` 静默安装 → `reg query` 确认注册表 → 卸载 → 再查一遍
确认清理干净。

## 下一步建议

按 [TODO.md](TODO.md) 里的优先级，剩下 4 项里 mac/Linux 那项只要有环境就能测；另外三项都需要
先做决策或者拿到缺失的信息，不建议在没有这些前提的情况下硬上。
