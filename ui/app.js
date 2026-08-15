// Container page for dsh-desktop: hosts the harness UI in an <iframe> and
// renders the native file/git panel beside it. Talks to the Rust shell
// through Tauri IPC. The harness content inside the iframe never gets IPC
// access — window.__TAURI__ is injected into this top-level document only,
// and browsers don't propagate it into a cross-origin nested iframe.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const els = {
  starting: document.getElementById("state-starting"),
  error: document.getElementById("state-error"),
  harnessFrame: document.getElementById("harness-frame"),
  startingDetail: document.getElementById("starting-detail"),
  errorMessage: document.getElementById("error-message"),
  logBox: document.getElementById("log-box"),
  logBoxStarting: document.getElementById("log-box-starting"),
  btnLogsStarting: document.getElementById("btn-logs-starting"),
  btnRetry: document.getElementById("btn-retry"),
  btnRestart: document.getElementById("btn-restart"),
  btnLogs: document.getElementById("btn-logs"),
  btnOpenBrowser: document.getElementById("btn-open-browser"),
  footer: document.getElementById("footer"),
  updateBanner: document.getElementById("update-banner"),
  updateText: document.getElementById("update-text"),
  btnUpdateInstall: document.getElementById("btn-update-install"),
  btnUpdateDismiss: document.getElementById("btn-update-dismiss"),
  providerTip: document.getElementById("provider-tip"),
  btnProviderTipDismiss: document.getElementById("btn-provider-tip-dismiss"),
  panelWorkspaceSelect: document.getElementById("panel-workspace-select"),
  panelTree: document.getElementById("panel-tree"),
  btnPanelRefresh: document.getElementById("btn-panel-refresh"),
};

// Shown once (best-effort) during the first-ever boot wait, so new users
// discover the existing Settings → 模型 → 添加提供方 flow without us having
// to touch the harness page itself (it's iframed content with zero IPC
// access — see lib.rs).
const PROVIDER_TIP_DISMISSED_KEY = "dsh-desktop-provider-tip-dismissed";

function initProviderTip() {
  if (localStorage.getItem(PROVIDER_TIP_DISMISSED_KEY)) return;
  els.providerTip.classList.remove("hidden");
  els.btnProviderTipDismiss.addEventListener("click", () => {
    localStorage.setItem(PROVIDER_TIP_DISMISSED_KEY, "1");
    els.providerTip.classList.add("hidden");
  });
}

let logsVisible = false;
let logsStartingVisible = false;

function show(id) {
  for (const key of ["starting", "error"]) {
    els[key].classList.toggle("hidden", key !== id);
  }
  els.harnessFrame.classList.toggle("hidden", id !== "running");
}

async function loadLogsInto(box) {
  try {
    const lines = await invoke("get_log_tail", { n: 200 });
    box.textContent = lines.join("\n");
  } catch (err) {
    box.textContent = `无法读取日志: ${err}`;
  }
}

function toggleLogs() {
  logsVisible = !logsVisible;
  els.logBox.classList.toggle("hidden", !logsVisible);
  els.btnLogs.textContent = logsVisible ? "隐藏日志" : "查看日志";
  if (logsVisible) loadLogsInto(els.logBox);
}

function toggleLogsStarting() {
  logsStartingVisible = !logsStartingVisible;
  els.logBoxStarting.classList.toggle("hidden", !logsStartingVisible);
  els.btnLogsStarting.textContent = logsStartingVisible ? "隐藏日志" : "查看日志";
  if (logsStartingVisible) loadLogsInto(els.logBoxStarting);
}

function render(status) {
  switch (status.state) {
    case "running":
      show("running");
      els.harnessFrame.src = status.url;
      refreshPanel();
      break;
    case "starting":
    case "idle":
      show("starting");
      els.startingDetail.textContent = status.detail || "准备本地服务";
      break;
    case "stopped":
      show("error");
      els.harnessFrame.src = "about:blank";
      els.errorMessage.textContent =
        `服务已停止（exit ${status.code ?? "?"}）。` +
        (status.message ? `\n${status.message}` : "");
      break;
    case "error":
      show("error");
      els.harnessFrame.src = "about:blank";
      els.errorMessage.textContent = status.message || "未知错误";
      break;
    default:
      show("starting");
  }
}

async function refresh() {
  try {
    const status = await invoke("get_status");
    render(status);
  } catch (err) {
    show("error");
    els.errorMessage.textContent = `无法获取状态: ${err}`;
  }
}

// ── file/git panel ───────────────────────────────────────────────────────

const GIT_STATUS_CLASS = {
  modified: "git-modified",
  added: "git-added",
  deleted: "git-deleted",
  untracked: "git-untracked",
};

// "" is the sentinel for "auto-follow" in the <select> — never a real
// filesystem path, so it can't collide with an actual workspace's value.
const AUTO_OPTION_VALUE = "";
const LOCKED_WORKSPACE_KEY = "dsh-desktop-locked-workspace";

// null = auto-follow (get_active_workspace's live, best-effort inference —
// see its Rust-side doc comment for why that's a real ceiling, not a
// shortcut); a string is the user's own pick from the panel's picker,
// passed straight through as get_workspace_tree/get_git_status's
// overridePath. Persisted so an explicit choice survives a restart, the
// same pattern PROVIDER_TIP_DISMISSED_KEY already uses for a one-time
// user decision.
let lockedWorkspace = localStorage.getItem(LOCKED_WORKSPACE_KEY) || null;

function renderTreeNode(entry, gitMap, container) {
  const row = document.createElement("div");
  row.className = "tree-row" + (entry.isDir ? " tree-dir" : " tree-file");
  const status = gitMap.get(entry.path);
  if (status && GIT_STATUS_CLASS[status]) row.classList.add(GIT_STATUS_CLASS[status]);

  const label = document.createElement("span");
  label.className = "tree-label";
  label.textContent = (entry.isDir ? "📁 " : "📄 ") + entry.name;
  row.appendChild(label);
  container.appendChild(row);

  if (entry.isDir && entry.children) {
    const childWrap = document.createElement("div");
    childWrap.className = "tree-children";
    container.appendChild(childWrap);
    row.addEventListener("click", () => childWrap.classList.toggle("collapsed"));
    for (const child of entry.children) {
      renderTreeNode(child, gitMap, childWrap);
    }
  }
}

// Rebuilds the picker's <option>s from the known-workspaces list plus the
// auto-follow sentinel (whose label carries the live-resolved name, when
// there is one, so auto mode stays informative without a second element).
function renderWorkspaceOptions(knownWorkspaces, autoLabel) {
  const select = els.panelWorkspaceSelect;
  select.replaceChildren();

  const autoOption = document.createElement("option");
  autoOption.value = AUTO_OPTION_VALUE;
  autoOption.textContent = autoLabel ? `自动跟随（${autoLabel}）` : "自动跟随当前会话";
  select.appendChild(autoOption);

  let lockedValueFound = lockedWorkspace === null;
  for (const ws of knownWorkspaces) {
    const option = document.createElement("option");
    option.value = ws.path;
    option.textContent = ws.title;
    option.title = ws.path;
    select.appendChild(option);
    if (ws.path === lockedWorkspace) lockedValueFound = true;
  }
  // The locked path was picked from a list that has since changed (e.g. the
  // workspace was removed/archived in the harness) — keep showing it rather
  // than silently falling back, since the directory on disk hasn't gone
  // anywhere; only the picker's own option list is stale.
  if (!lockedValueFound) {
    const staleOption = document.createElement("option");
    staleOption.value = lockedWorkspace;
    staleOption.textContent = lockedWorkspace;
    select.appendChild(staleOption);
  }

  select.value = lockedWorkspace ?? AUTO_OPTION_VALUE;
}

async function refreshPanel() {
  const knownWorkspacesPromise = invoke("get_known_workspaces").catch(() => []);

  // Skipped entirely once the user has a manual pick locked in — there's
  // nothing left to infer. Otherwise re-resolved every refresh, not just
  // once at startup: the harness's own in-page workspace switcher, entirely
  // inside the iframe with no signal reaching this shell directly, can
  // change independently of anything else this shell observes.
  let autoLabel = null;
  if (lockedWorkspace === null) {
    try {
      autoLabel = await invoke("get_active_workspace");
    } catch {
      /* falls through with autoLabel null; the option keeps its static text */
    }
  }
  renderWorkspaceOptions(await knownWorkspacesPromise, autoLabel);

  try {
    const treeArgs = { overridePath: lockedWorkspace };
    const [tree, gitEntries] = await Promise.all([
      invoke("get_workspace_tree", treeArgs),
      invoke("get_git_status", treeArgs),
    ]);
    const gitMap = new Map(gitEntries.map((e) => [e.path, e.status]));
    els.panelTree.replaceChildren();
    if (tree.length === 0) {
      const empty = document.createElement("p");
      empty.className = "muted panel-empty";
      empty.textContent = "空工作区";
      els.panelTree.appendChild(empty);
      return;
    }
    for (const entry of tree) {
      renderTreeNode(entry, gitMap, els.panelTree);
    }
  } catch (err) {
    els.panelTree.textContent = `无法加载文件树: ${err}`;
  }
}

// Cheap enough (one directory walk + one `git status`) to poll rather than
// stand up a real filesystem watcher for this first slice — see the plan
// note on deferring that complexity. Cleared on nothing; the container page
// itself is never torn down, so this interval simply runs for the app's
// whole lifetime.
const PANEL_POLL_MS = 6000;

// ── init ─────────────────────────────────────────────────────────────────

async function init() {
  try {
    const info = await invoke("get_info");
    const bits = [];
    if (info.dshVersion) bits.push(`dsh ${info.dshVersion}`);
    if (info.nodePath) bits.push(`Node ${info.nodePath}`);
    if (info.dshHome) bits.push(`数据目录 ${info.dshHome}`);
    els.footer.textContent = bits.join(" · ");
  } catch {
    /* footer is cosmetic */
  }

  listen("server-status", (event) => render(event.payload));
  els.btnRetry.addEventListener("click", () => {
    els.btnRetry.disabled = true;
    invoke("start_server")
      .catch((err) => {
        els.errorMessage.textContent = `启动失败: ${err}`;
      })
      .finally(() => {
        els.btnRetry.disabled = false;
      });
  });
  els.btnRestart.addEventListener("click", () => {
    els.btnRestart.disabled = true;
    invoke("restart_server")
      .catch((err) => {
        els.errorMessage.textContent = `重启失败: ${err}`;
      })
      .finally(() => {
        els.btnRestart.disabled = false;
      });
  });
  els.btnLogs.addEventListener("click", toggleLogs);
  els.btnLogsStarting.addEventListener("click", toggleLogsStarting);
  els.btnOpenBrowser.addEventListener("click", () => invoke("open_in_browser"));
  els.btnPanelRefresh.addEventListener("click", refreshPanel);
  els.panelWorkspaceSelect.addEventListener("change", () => {
    const value = els.panelWorkspaceSelect.value;
    lockedWorkspace = value === AUTO_OPTION_VALUE ? null : value;
    if (lockedWorkspace === null) {
      localStorage.removeItem(LOCKED_WORKSPACE_KEY);
    } else {
      localStorage.setItem(LOCKED_WORKSPACE_KEY, lockedWorkspace);
    }
    refreshPanel();
  });

  els.btnUpdateDismiss.addEventListener("click", () => {
    els.updateBanner.classList.add("hidden");
  });
  els.btnUpdateInstall.addEventListener("click", () => {
    els.btnUpdateInstall.disabled = true;
    els.btnUpdateInstall.textContent = "正在更新…";
    els.btnUpdateDismiss.disabled = true;
    // On success this relaunches the app (the window disappears); a caught
    // error means the update didn't apply, so restore the button for retry.
    invoke("install_update").catch((err) => {
      els.btnUpdateInstall.disabled = false;
      els.btnUpdateInstall.textContent = "立即更新";
      els.btnUpdateDismiss.disabled = false;
      els.updateText.textContent = `更新失败: ${err}`;
    });
  });
  checkForUpdate();
  initProviderTip();

  setInterval(refreshPanel, PANEL_POLL_MS);
  await refresh();
  await refreshPanel();
}

async function checkForUpdate() {
  try {
    const update = await invoke("check_for_update");
    if (!update) return;
    els.updateText.textContent = `发现新版本 ${update.version}`;
    els.updateBanner.classList.remove("hidden");
  } catch {
    /* update check is best-effort; silent failure keeps the boot page usable offline */
  }
}

init();
