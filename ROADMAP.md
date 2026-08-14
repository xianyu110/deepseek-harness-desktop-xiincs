# 迭代规划：从"能用"到"爆款"

基于对 4 个竞品仓库（均为社区第三方壳，无一官方——见下方说明）的 issue/PR 分析，
和对本项目当前代码（`src-tauri/src/*.rs`、`tauri.conf.json`）的实测，得出的优先级排序。

> **关于"官方"的澄清**：`dataelement/dsh-desktop` 由 GitHub 组织 `dataelement`（DataElem，
> dataelem.com）所有，与发布 harness 本体的 `deepseek-ai` 组织完全无关。该仓库自己的 README
> 也明确写着 "DSH Desktop is an independent community desktop wrapper... not affiliated with
> or endorsed by DeepSeek"。所以五个项目（含本项目）在"官方认证"这一点上是平等的——没有谁
> 是基准线，都是同一层级的社区实现，只是 dataelement 这家做得早、功能全、star 数领先。

结论先行：本项目在**工程可靠性**上已经领先所有竞品，但在**功能广度**上落后。爆款的路径不是
补齐所有功能，而是保住可靠性优势的同时，只吃最高杠杆的几个功能缺口——尤其是第三方模型
接入和 macOS，这两项直接决定潜在用户池的大小。

---

## 市场背景：为什么这个空当存在（查了官方 Discussions 后确认）

官方仓库 `deepseek-ai/deepseek-harness` **关闭了 issue 功能**，只保留 GitHub Discussions 作反馈
渠道。搜索 "desktop" 能翻到 **15+ 条**独立发起的讨论帖，其中比较有代表性的：

- [#510](https://github.com/deepseek-ai/deepseek-harness/discussions/510)
  "Feature request: Desktop GUI client (Tauri/Electron) alongside the browser Web UI"——描述的
  验收标准（原生窗口、托盘常驻、通知、关窗不杀会话）几乎就是本项目现在已经做到的功能清单。
- [#222](https://github.com/deepseek-ai/deepseek-harness/discussions/222) "desktop 啥时候端上来"、
  [#1133](https://github.com/deepseek-ai/deepseek-harness/discussions/1133)
  "后续推进app的进度有吗?"——需求持续被提出，跨越了较长时间。
- 至少 6 个不同作者各自发帖展示了自己独立做的桌面壳（[#239](https://github.com/deepseek-ai/deepseek-harness/discussions/239)、
  [#434](https://github.com/deepseek-ai/deepseek-harness/discussions/434)、[#537](https://github.com/deepseek-ai/deepseek-harness/discussions/537)、
  [#872](https://github.com/deepseek-ai/deepseek-harness/discussions/872)、[#904](https://github.com/deepseek-ai/deepseek-harness/discussions/904)、
  [#1086](https://github.com/deepseek-ai/deepseek-harness/discussions/1086) 等），彼此之间没有协作痕迹，
  是同一时间窗口内独立收敛到同一个想法——**这五个仓库（含本项目）只是其中被发现、被本次分析
  覆盖到的一小部分，实际"竞品"数量可能更多**。

查了这些帖子下面的回复：**没有任何 `deepseek-ai` 官方账号表态过路线图或认领这个方向**。
这意味着两件事同时成立：

1. **需求是真实、持续、被反复验证过的**——不是臆测的市场，官方 Discussions 就是现成的用户
   声音来源，可以直接引用真实用户的验收标准（如 #510）校准功能优先级。
2. **最大的单一风险是官方随时可能亲自下场**——一旦 `deepseek-ai` 官方发布自己的桌面客户端，
   现在所有社区实现（包括本项目）的生存空间会被大幅压缩。这是判断"值得投入多少"时必须考虑
   的边界条件：优先做那些即使官方入场也依然有价值的差异化（工程可靠性、安全模型），少做
   容易被官方一次性原生吸收的东西（比如单纯的"引导用户加供应商"这类，官方大概率会自己在
   某个 RC 里补上，见下方 P0-1 里"已确认 rc.6 是当前最新发布版，HEAD 已经在改这块逻辑"的
   记录）。

---

## 重大发现：harness 自己定义了一套"桌面壳"官方集成契约

这条改变了本文档后面好几处判断，单独拎出来。

在 ChisaAlter 仓库 vendor 进来的官方 harness 源码里读到
`packages/client/ui-settings-plugin-inventory/src/client/desktop-shell.ts`——这是**上游
harness 自己的源码**，不是任何 fork 加的东西。文件顶部注释写着 "Desktop shell bridge used by
the marketplace Settings tab. Absent outside the desktop app"，内容是一个完整的 TypeScript
接口 `DesktopShell`：`listMarketplace` / `installPlugin` / `uninstallPlugin` /
`saveConfig`（写 GitHub Token）/ `onPluginProgress` 等方法，运行时通过读取
`window.shell` 探测这个桥是否存在（`desktopShell()` 函数）。

也就是说：**harness 官方在设计插件市场这个 UI 时，就预留了"桌面壳可以注入 `window.shell`
对象来接管市场功能"这个正式扩展点**，不是哪个社区项目反向工程出来的偏方。已经用 ChisaAlter
自己的 `src/preload/index.js` 核实过，它通过 Electron 的 `contextBridge.exposeInMainWorld
('shell', {...})` 完整实现了这个契约（`listMarketplace`/`installPlugin` 等方法名和官方接口
逐一对应），并且不止插件市场——还扩展出了 `getState`/`onTheme`/`windowAction` 等更多方法，
说明这套桥接机制本身可以承载相当广的桌面壳↔网页双向通信，不只是插件安装这一项。

**这对本项目"harness 页面零 IPC"的安全模型意味着什么**：不是说这个安全立场错了——注入
`window.shell` 等于把桌面壳的本地文件系统/进程控制能力暴露给一个远程加载的网页，是本项目
现在明确拒绝的攻击面。但需要更正一个判断：这不是"harness 架构逼着桌面壳必须做深度 IPC 才能
用插件市场"，而是"harness 官方提供了一个可选的集成点，本项目选择不用它，用零 IPC 换更小的
攻击面"。这是一个**主动的安全取舍，不是唯一可行架构**，文档后面凡是提到"harness architecture
决定了必须走 A 方案"的地方，都应该改成"这是本项目为了安全性主动放弃的官方集成点"这个更准确
的表述。也意味着如果将来真的要做插件市场（P1-5），"照官方契约实现 `window.shell`"是一条
被官方明确支持、有文档可依的路径，不是自己摸索的野路子——只是仍然要承担"允许远程页面驱动
本地安装脚本"的风险，这个风险本身没有变。

---

## 竞品清单

| 仓库 | Star | 技术栈 | 平台 | 一句话定位 |
|---|---|---|---|---|
| [dataelement/dsh-desktop](https://github.com/dataelement/dsh-desktop)（非官方，DataElem 出品） | 101 | Electron+TS | macOS 签名公证，Win 未验证 | 第三方模型供应商接入 UI |
| **本项目**（xiincs） | — | **Tauri 2 (Rust)** | Windows only | 极简攻击面、体积小 |
| [myYangyunfan/dsh_desktop](https://github.com/myYangyunfan/dsh_desktop) | 58 | Electron+TS+PowerShell | Windows only | 功能最激进（计费小部件、文件 diff 还原） |
| [steven-kid/deepseek-harness-desktop](https://github.com/steven-kid/deepseek-harness-desktop) | 79 | Electron+JS | macOS+Win+Linux 全平台 | 全平台覆盖 + 独立官网 |
| [ChisaAlter/Deepseek-Harness-Desktop](https://github.com/ChisaAlter/Deepseek-Harness-Desktop) | 24 | Electron+JS | Windows only | 主题个性化 + 插件市场 |

---

## 本项目当前状态核查（读代码得出，非猜测）

已验证**没有**竞品踩过的坑：

- **超时常量单一来源**：[server.rs:49](src-tauri/src/server.rs:49) 只有一个 `READY_TIMEOUT`（45s），
  不存在 myYangyunfan [#3](https://github.com/myYangyunfan/dsh_desktop/issues/3) 那种
  `bootTimer`(60s) 和 `waitUntilUp`(120s) 两处不一致导致首启必然误报超时的问题。
- **打包依赖不缺失**：运行时通过 `npm install --prefix <dir> @deepseek-ai/dsh@<version>`
  整体安装（[server.rs:360-372](src-tauri/src/server.rs:360)），不是把 node_modules 树打进
  electron-builder 再靠 `files` 白名单裁剪，不会重现 dataelement/dsh-desktop
  [#9](https://github.com/dataelement/dsh-desktop/issues/9)/[#10](https://github.com/dataelement/dsh-desktop/pull/10)
  那种运行时动态 import 的 19 个包被裁掉、`ERR_MODULE_NOT_FOUND` 崩溃的问题。
- **启动等待页 + 托盘常驻已具备**：这是 steven-kid
  [#2](https://github.com/steven-kid/deepseek-harness-desktop/issues/2)/[#3](https://github.com/steven-kid/deepseek-harness-desktop/issues/3)
  用户主动要的两个功能，本项目 [lib.rs:217-238](src-tauri/src/lib.rs:217) 早就有。

已确认**存在**的差距（代码层面直接可查）：

- [tauri.conf.json:28](src-tauri/tauri.conf.json:28) `bundle.targets` 只有 `["nsis"]`，
  `resources/runtime` 也只有 Windows 版 node.exe——macOS/Linux 打包完全未配置。
- 本项目没有任何模型供应商选择 UI 定制——因为架构上 harness 页面是纯远程页面、零 IPC
  （[lib.rs:1-8](src-tauri/src/lib.rs:1) 的安全模型注释），桌面壳本身插不上手，这是设计选择
  而非疏漏，见下方"设计权衡"。

  **重要澄清（已读 dataelement 的实际 patch 文件核实）**：stock `@deepseek-ai/dsh` 本来就有
  通用的 Settings → Models 多供应商机制（`ProviderEditor`/`namespaces` 这套底层管线原本就
  支持任意 provider——已用 `npm pack @deepseek-ai/dsh-client-ui-settings-models@0.1.0-rc.6`
  下载实际发布包核实，"添加提供方" 按钮和通用 `ProviderEditor` 组件本来就在），本项目现在
  打开 harness 自带 Settings 页也能手动加供应商，**不是完全空白**。dataelement patch 的实际
  内容（`patches/@deepseek-ai+dsh-client-ui-settings-models+0.1.0-rc.6.patch`，21.7KB）是把
  stock harness **首次启动引导弹窗**里硬编码"只提供 DeepSeek 官方一个选项"
  （原文案 "Configure the official DeepSeek provider to start building"，同样在下载的 rc.6
  实际包里核实存在）改造成了一个可搜索的多供应商网格选择器（`ProviderPicker`，9 家供应商，
  双语文案），即"把已存在的能力从设置深处搬到首次启动的第一屏，并做了视觉打磨"，而不是
  从零实现了多供应商支持。差距是真实的（首次启动体验差一截），但比"完全没有这个功能"要小。

  **版本时效性提醒**：`npm view @deepseek-ai/dsh versions` 核实过，`0.1.0-rc.6` 是**当前
  npm 上发布的最新版本**（截至查证时）。但官方仓库 HEAD 分支的
  `apps/web/tests/onboarding-usable-provider.e2e.ts` 显示，主干代码已经把这个首屏引导逻辑
  改成"配置任意一个可用供应商即可结束引导"的通用判断（`anyUsable` 逻辑），不再是 rc.6 这种
  DeepSeek 专属硬编码。也就是说**上游已经在修这个问题，只是还没发布新 RC**。如果后续升级
  `DSH_VERSION_DEFAULT`（[server.rs:36](src-tauri/src/server.rs:36)）到比 rc.6 新的版本，
  第一步应该是重新核实这个差距是否已经自然消失——如果消失了，P0-1 直接可以从路线图划掉，
  不需要做任何桌面壳侧的改动。

---

## P0：直接影响能否留住用户（2-4 周）

### 1. 首次启动的多供应商引导体验 ✅ 已完成
**现状**：dataelement/dsh-desktop 和 myYangyunfan 都做了首启多供应商选择弹窗（9 家一键接入）；
本项目首启直接进 harness 原生流程，原生流程默认只引导配置 DeepSeek 官方 provider（其他供应商
要用户自己找到 设置→模型 手动加，功能都在，但**发现路径长**）。

**已实现（Path A，纯前端，零 Rust 改动）**：[ui/index.html](ui/index.html)/[ui/app.js](ui/app.js)/
[ui/styles.css](ui/styles.css) 加了一条一次性的 `#provider-tip` 提示 banner，首次启动等待期间
出现，用 `localStorage` 记"已提示过"；文案指向已有的"设置 → 模型 → 添加提供方"入口，不碰
harness 内部状态，不新增 Tauri command。已在真实 Tauri webview（非纯前端 mockup）里截图确认
渲染正确，并验证过它在状态从"启动中"切到"启动失败"时依然正确留在原地、不随状态机重置——
说明它的生命周期确实独立于 `render(status)`。dismiss 按钮本身没能在真机上点击验证到（当时
环境里有端口竞争导致的重启抖动），但和同文件里已验证在用的 `btnUpdateDismiss` 是完全同构的
写法，风险很低。
**为什么优先级高**：这是新用户第一屏就会碰到的体验差，直接影响"只用第三方模型的用户"愿不愿意
留下来试第二步——即使功能上都能达到同样终点，首屏引导的缺失会在多数用户还没发现"其实设置里
能加"之前就把人推走。

**关键判断**：这不是"从零实现供应商支持"，本质是"把 harness 已有的通用多供应商能力从设置
深处搬到首屏，加一层更好的引导 UI"。两条可行路径：
- **路径 A（轻，推荐）**：桌面壳自己的启动页/首屏加一步"引导用户去 设置→模型 配置供应商"
  的提示或深链（比如启动页加个"先配置模型供应商"的按钮，点击后 `navigate` 到 harness 的
  设置路由），不碰 harness 内部状态，维持零 IPC 边界，成本低。
- **路径 B（重）**：照抄 dataelement 的做法，用 `patch-package` 直接改
  `@deepseek-ai/dsh-client-ui-settings-models` 的 JS，把 patch 应用在运行时装好的
  node_modules 里（见 [server.rs](src-tauri/src/server.rs) 的 `runtime_dir`）。可行，
  但要跟着上游 `0.1.0-rc.x` 频繁重新生成 patch，且需要在打包/CI 里补一道 patch-apply 步骤，
  维护成本持续存在。

路径 A 风险小、符合现有架构；路径 B 能做到和 dataelement 版视觉对齐但架构包袱重。
**建议先做 A**，观察 harness 自身是否会在后续 RC 版本里把这个引导做进 stock 流程
（上游本来就在快速迭代，首屏只推 DeepSeek 官方这一点也可能是上游自己会补的）。

### 2. macOS 打包与验证（部分推进：CI 上的 cargo check 已在三平台绿灯）
**新增**：[.github/workflows/ci.yml](.github/workflows/ci.yml) 加了 windows/macos/ubuntu 三平台的
`cargo check` job，推送后实测跑过——第一次跑三个平台全部失败（`tauri-build` 的 build script 硬性
要求 `resources/runtime` 路径存在，这个目录只有跑过 `fetch:node`/`prepare:runtime` 才有内容，
CI 里没跑那两步），修成先建一个空占位目录满足存在性检查后，三平台 `cargo check` 全部通过
（[run](https://github.com/xiincs/deepseek-harness-desktop/actions/runs/31793561917)）。这是
`server.rs` 里 `#[cfg(unix)]` 分支第一次被真正类型检查——之前"从未跑过"字面意义上是"从未编译过"
（cfg 不匹配的代码根本不会被纳入编译单元，不是编译过没验证，是压根没编译）。

**新增（第二轮）**：加了 `fetch-node` CI job，在 macOS/Linux 上真实跑了一次
`node scripts/fetch-node.mjs`（不是短路跳过——日志里能看到真实的
`https://nodejs.org/dist/v24.9.0/node-v24.9.0-darwin-arm64.tar.gz` 下载 URL、解压过程、两次独立
`--version` 校验都输出 `v24.9.0`）。darwin 和 linux 分支都通过
（[run](https://github.com/xiincs/deepseek-harness-desktop/actions/runs/31794303728)）——这是这两个
分支写下来之后第一次被真正执行过。

**新增（第三轮）**：加了 `prepare-runtime` CI job，在 macOS/Linux 上真实跑了
`node scripts/prepare-runtime.mjs`（`npm install --prefix ... @deepseek-ai/dsh@0.1.0-rc.6`），
再用刚 fetch 到的真实 node 二进制跑 `bin.js --help` 冒烟测试。日志里能看到真实的
"added 529 packages in 2m" 和真实的 `Usage: dsh [options] [command] [args...]` 输出，两个平台
都通过（[run](https://github.com/xiincs/deepseek-harness-desktop/actions/runs/31794639808)）。

**结论**：`fetch-node.mjs` + `prepare-runtime.mjs` 这条组装完整 `resources/runtime` 的链路，
在 macOS 和 Linux 上都有真实证据证明能跑通——这是这个项目历史上第一次验证过"能不能在这两个平台
上产出一份可用的打包运行时"这个最核心的技术未知数，不再是纯粹的"理论上跨平台"。

**新增（第四轮，完整链路验证）**：加了 `bundle-smoke-test` job，用 `--bundles <fmt>` 这个
per-invocation CLI 参数（不是改 `tauri.conf.json` 的共享 `targets` 列表）在 macOS 上真跑
`tauri build --bundles dmg`、Linux 上真跑 `tauri build --bundles deb`。第一次跑暴露了一个真实
但良性的问题：`tauri.conf.json` 配了 updater pubkey，deb 打包成功后会额外尝试产出签名的 updater
产物，没给 `TAURI_SIGNING_PRIVATE_KEY`（这个 job 本来就不该有这个 secret）导致这一步报错、把整个
命令的退出码带成非零——但此时 `.deb` 本身已经真实落盘了（日志："Finished 1 bundle at: ...
DeepSeek Harness_0.3.0_amd64.deb"）。加了 `continue-on-error: true` 让专门的产物存在性检查
成为真正的判定标准后，两个平台都绿灯：
[最终 run](https://github.com/xiincs/deepseek-harness-desktop/actions/runs/31796177602)
里能看到真实产出的 `DeepSeek Harness_0.3.0_aarch64.dmg`（102MB）和
`DeepSeek Harness_0.3.0_amd64.deb`。

**结论**：`fetch-node.mjs` → `prepare-runtime.mjs` → `tauri build --bundles` 这条完整链路，
在 macOS 和 Linux 上都有端到端的真实证据——不只是"运行时能装起来"，是"能产出真实的、体积正常的
安装包文件"。这是这个项目历史上第一次证明"macOS/Linux 打包在技术上是可行的"，剩下的不再是未知数，
是纯粹的资源/流程缺口。

**新增（第五轮，已接入发布流程）**：跟用户确认过"未签名发布 macOS/Linux"这个产品取舍（推荐选项）
后，[release.yml](.github/workflows/release.yml) 加了 `macos-release`/`linux-release` 两个
`needs: release` 的新 job，在 Windows job 建好 draft release 之后用 `gh release upload` 把
`.dmg`/`.deb` 附加上去——不碰 Windows job 的任何逻辑，不碰 `tauri.conf.json` 的共享
`targets`，不写入 `latest.json`/不参与自动更新（那套机制要验证真实签名，macOS 这边没有
Apple Developer 账号产不出）。**这一步还没有被真实的 `v*` tag 触发验证过**——`release.yml`
只在打 tag 时跑，push 到 main 不会触发，需要下次正式发版时才会第一次真正跑起来。

**仍未做（真正的硬约束，不是没验证）**：
  值得单独更谨慎地对待，不该用前四轮"加一个独立 job"那种节奏推进。
- 代码签名与公证：需要 Apple Developer 账号，没有，且这不是能靠 CI 绕过的技术问题，是纯粹的
  资源/credential 缺口。

**现状（旧）**：[tauri.conf.json](src-tauri/tauri.conf.json) 和 `resources/runtime` 都是
Windows-only，`server.rs` 里非 Windows 分支存在但从未跑过（见文件头注释 "non-Windows branches
below are untested"）——这条注释现在已经不完全准确，见上方"新增"，但打包/签名部分依然成立。
**为什么优先级高**：steven-kid（79 star）、dataelement（101 star）都已经在 macOS 上跑通，
本项目路线图里这一项拖得越久，越可能被"全平台"的定位抢走用户心智。
**已知平台坑**（来自 steven-kid [#1](https://github.com/steven-kid/deepseek-harness-desktop/issues/1)，
已读实际修复代码 `src/mac-titlebar.js` + `src/window-options.js` 核实细节）：`hiddenInset`
标题栏本身**没有**在修复后被换掉——`window-options.js` 里 `titleBarStyle: isMac ? 'hiddenInset'
: 'default'` 修复后依然是 hiddenInset。真正的修复是 `mac-titlebar.js` 通过
`webContents.insertCSS()` 往**远程 harness 页面**注入一段 CSS，用 `padding-top:
env(titlebar-area-height, 38px)` 让页面内容整体下移，给 OS 让出一条可拖动的空白带——harness
页面本身完全不用改，桌面壳单方面就能修。这个技巧比"退回原生标题栏"更精确，而且**直接适用
于本项目**：如果之后想在 macOS/Windows 上做无边框窗口，同样可以在 Rust 侧用 WebView2/WKWebView
的注入 CSS 能力（Tauri 有 `Window::eval` 或 webview 的 `add_init_script`）往加载好的 harness
远程页面注入类似的 `padding-top` 规则，不需要碰 harness 源码、也不违反"harness 页面零 IPC"
的边界（注入 CSS 不等于授予 IPC——纯视觉层面）。
**建议**：先在真实 Mac 硬件（或 CI runner）上把 `npm run tauri dev` 跑通，把
`kill_process_tree`/`npm_install_command` 的 Unix 分支实测一遍，再谈打包签名公证；如果决定做
无边框窗口，直接复用上面这个"注入 CSS 让出拖动带"的思路，不用重新踩一遍 steven-kid 踩过的坑。

### 3. Windows 首启体验的压力测试（新增：真实生产事故已修复两项）
**从真实使用中发现并已修复**（不是压测出来的，是安装版实际崩溃两次后直接定位）：
- **进程生命周期竞态**：重启时旧进程的退出监听线程可能在新进程 spawn 之后才醒来，无条件清空
  `s.pid` 会误伤新进程、触发多余的自动重启——[server.rs](src-tauri/src/server.rs) 的 exit
  watcher 和 stdout URL 探测都加了"我还是不是当前进程"的判断。
- **单实例缺失**：桌面图标多次点击会开多个进程、多个托盘图标——已接入
  `tauri-plugin-single-instance`。
- **`~/.dsh/profiles/node_modules/@deepseek-ai/*` 残留自愈**：dsh 自己的 `ensureSymlink`
  bootstrap 在该目录下发现非符号链接的残留目录时会直接崩溃退出，两次真实事故各自卡在不同的包
  上。现在 `start_inner()` 会识别这个特定错误、安全校验（路径必须在预期前缀下、必须是空目录、
  不是符号链接）后自动清理并重试，上限 5 次。三条真实回归测试（含两条原始报错文本）在
  `server.rs` 的 `mod tests` 里。

**未做（仍需真实沙箱）**：Defender 拖慢首次 npm install 的计时验证——`READY_TIMEOUT`（45s）是否
够用尚无证据，不会凭空调整。

原始条目：
**现状**：`READY_TIMEOUT` 是 45 秒，比 myYangyunfan 修复前的 60 秒还短。myYangyunfan
[#4](https://github.com/myYangyunfan/dsh_desktop/issues/4) 实测过：便携版冷启动在 Defender
实时扫描下解压 132MB/2.4万文件能拖到数分钟。
**行动**：本项目当前是安装版（NSIS），运行时通过 npm install 装到 per-user 缓存目录，理论上
不会重现"每次启动全量解压"的问题（装一次之后复用），但**没有实测过 Defender 拖慢首次 npm
install 的场景**。建议在全新 Windows 沙箱里跑一次首次安装计时，确认 45 秒对最坏情况是否够用，
必要时把这个数字提到 90-120 秒——但要成为唯一的超时常量，不要重蹈 myYangyunfan 的覆辙。

---

## P1：差异化但非致命（1-2 月）

### 4. 全平台覆盖（Linux）
steven-kid 已做到 Win+Mac+Linux 全覆盖（AppImage + deb）。Linux 用户体量比 macOS 小，
放在 macOS 之后。Tauri 本身对 Linux 打包支持成熟，一旦 macOS 分支验证通过，Linux 增量成本低。

### 5. 插件市场入口
ChisaAlter 做了（GitHub `dsh-plugin` topic 检索 + 一键安装，通过官方
`packages/client/ui-settings-plugin-inventory` 定义的 `DesktopShell.installPlugin` 等契约
实现，见上方"重大发现"一节），但同期 myYangyunfan
[#5](https://github.com/myYangyunfan/dsh_desktop/issues/5) 报告插件市场里的插件把旧版核心包
装进用户目录、覆盖新版导致服务名对不上而卡死——**插件市场是真实需求，官方也预留了正式集成点
（不是要不要"合规"的问题），但接入这个契约本身就意味着要把"安装脚本可以碰本机文件系统"这个
能力交给一个远程页面驱动，这是本项目当前主动放弃的攻击面，不是被架构逼出来的取舍**。
**建议**：如果做，两条路径都可行——(a) 做"引导用户去 harness 自带插件管理"的向导（不接入
`window.shell`，维持零 IPC，但用户体验不如原生市场直接）；(b) 照官方 `DesktopShell` 契约
实现一个真正的 `window.shell` 桥（有据可依，不是野路子，但要接受"允许远程页面触发本地
安装脚本"这个真实风险，且要像 ChisaAlter 一样对安装脚本的信任边界做好提示）。先做 (a)，
把 (b) 留给用户量涨到插件市场变成高频请求之后再评估。

### 6. 个性化（主题/背景图）
ChisaAlter 独有功能，24 star 里唯一一条 issue（[#3](https://github.com/ChisaAlter/Deepseek-Harness-Desktop/issues/3)）
反而是要 MCP/Skills 支持，不是要更多主题——说明个性化本身不是这个用户群的强需求。
**建议**：观察，不抢跑。

---

## P2：重量级功能，先看数据再决定（按需）

### 7. 会话文件级 diff / 一键还原、计费余额小部件
myYangyunfan 的差异化功能，实现成本高（需要解析会话日志、渲染 diff UI），且这类功能理论上
应该由 harness 官方实现（更贴近数据源头），桌面壳做一遍容易和上游后续原生功能重复造轮子。
**建议**：不主动做，除非收到大量用户请求。

---

## 明确不做的事（保护现有优势）

1. **不引入 Electron 或拉 harness 源码进仓库**：dataelement 和 ChisaAlter 的 patch-package/subtree
   方案能做更深的定制，但代价是要跟着上游 `0.1.0-rc.x` 频繁 rebase，且 Electron 默认攻击面比
   `dangerousRemoteDomainIpcAccess: false` 的 Tauri 壳大。这是本项目相对其他四个社区实现
   最大的工程优势，不应该为了功能对齐而放弃。
2. **不做"两个独立计时器"这类容易失配的设计**：任何新增的等待/超时逻辑，必须复用或扩展
   [server.rs](src-tauri/src/server.rs) 里已有的单一常量模式，不要在别处（比如未来的更新检查、
   插件安装）另起一套超时数字。
3. **不为了抢跑功能而牺牲"harness 页面零 IPC"的安全边界**——这是本项目在
   [lib.rs](src-tauri/src/lib.rs) 顶部就写明的核心设计。需要澄清一点：harness 官方其实定义了
   一套正式的 `window.shell` 桌面集成契约（`DesktopShell` 接口，见上方"重大发现"一节），
   接入它是官方支持的路径，不是违反架构；本项目不接入它是**主动的安全取舍**，不是"唯一可行
   方案"。选择继续不接入的理由依然成立——远程加载的页面一旦能驱动本地安装脚本/文件系统，
   零 IPC 这条防线就名存实亡——但要清楚这是权衡后的决定，不是被迫的限制，未来如果用户需求
   压过安全顾虑，`DesktopShell` 契约本身是随时可以照官方文档实现的候选项，不需要重新发明。

---

## 执行顺序建议

```
第 1-2 周   Windows 首启压力测试（P0-3）+ macOS 本地跑通 tauri dev（P0-2 起步）
第 3-4 周   第三方模型供应商接入路径 A 原型（P0-1）
第 5-8 周   macOS 打包签名公证走通 + CI 覆盖（P0-2 完成）
第 9-12 周  Linux 打包（P1-4）+ 视用户反馈决定插件市场/个性化（P1-5/6）
```

每一步做完后，回到这份文档更新"已验证"章节——保持"代码事实 vs 计划"两者不脱节，
是本项目相对社区 fork 最大的可信度来源。
