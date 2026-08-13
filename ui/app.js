// Local boot page for dsh-desktop: loading / error / retry states.
// Talks to the Rust shell through Tauri IPC. The harness page itself
// (http://127.0.0.1:<port>) never gets IPC access.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const els = {
  starting: document.getElementById("state-starting"),
  running: document.getElementById("state-running"),
  error: document.getElementById("state-error"),
  startingDetail: document.getElementById("starting-detail"),
  runningUrl: document.getElementById("running-url"),
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
};

let logsVisible = false;
let logsStartingVisible = false;

function show(id) {
  for (const key of ["starting", "running", "error"]) {
    els[key].classList.toggle("hidden", key !== id);
  }
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
      els.runningUrl.textContent = status.url;
      break;
    case "starting":
    case "idle":
      show("starting");
      els.startingDetail.textContent = status.detail || "准备本地服务";
      break;
    case "stopped":
      show("error");
      els.errorMessage.textContent =
        `服务已停止（exit ${status.code ?? "?"}）。` +
        (status.message ? `\n${status.message}` : "");
      break;
    case "error":
      show("error");
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

  await refresh();
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
