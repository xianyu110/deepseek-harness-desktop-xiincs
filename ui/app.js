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
  panel: document.getElementById("panel"),
  panelWorkspaceSelect: document.getElementById("panel-workspace-select"),
  panelTree: document.getElementById("panel-tree"),
  btnPanelRefresh: document.getElementById("btn-panel-refresh"),
  resizePanelContent: document.getElementById("resize-panel-content"),
  btnToolbarFiles: document.getElementById("btn-toolbar-files"),
  btnFilesCollapse: document.getElementById("btn-files-collapse"),
  cardFiles: document.getElementById("card-files"),
  cardFile: document.getElementById("card-file"),
  panelPreviewTitle: document.getElementById("panel-preview-title"),
  panelPreviewDirtyDot: document.getElementById("panel-preview-dirty-dot"),
  panelPreviewBody: document.getElementById("panel-preview-body"),
  btnPreviewClose: document.getElementById("btn-preview-close"),
  btnPreviewSave: document.getElementById("btn-preview-save"),
  btnPreviewRevert: document.getElementById("btn-preview-revert"),
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

// ── resizable dock width ────────────────────────────────────────────────
//
// One draggable, persisted split: the dock (#panel, now right-docked)
// against #content (the harness iframe). Applies as an inline style (see
// applyPanelWidth) rather than living in styles.css, since a CSS width
// can't be end-user-adjustable without JS setting it somewhere; the
// stylesheet keeps a single fallback default for the instant before this
// script runs. The two dock cards (Files/File) no longer share a
// drag-adjustable split between them — each sizes to its own content and
// scrolls independently, so there's nothing left to resize inside #panel.

const PANEL_WIDTH_KEY = "dsh-desktop-panel-width";
const DEFAULT_PANEL_WIDTH = 380;
// Never shrinks the dock below this — small enough to still show a few
// characters of a filename or a code line, too small to accidentally
// collapse it to nothing mid-drag.
const MIN_PANEL_WIDTH = 240;
// The dock may never claim more than this fraction of the window — #content
// (the harness) staying visibly present matters more than an oversized dock.
const MAX_PANEL_WIDTH_RATIO = 0.7;

function loadStoredWidth(key, fallback) {
  const n = parseInt(localStorage.getItem(key), 10);
  return Number.isFinite(n) && n > 0 ? n : fallback;
}

let panelWidth = loadStoredWidth(PANEL_WIDTH_KEY, DEFAULT_PANEL_WIDTH);

function applyPanelWidth() {
  els.panel.style.width = `${panelWidth}px`;
}

function initResizeHandle() {
  els.resizePanelContent.addEventListener("mousedown", (downEvent) => {
    downEvent.preventDefault();
    els.resizePanelContent.classList.add("dragging");
    document.body.classList.add("resizing");

    // #panel is now the right-docked element, so its width is measured
    // from the window's right edge inward, not directly from clientX (that
    // reasoning only held when #panel started flush against the left edge).
    const onMouseMove = (moveEvent) => {
      const candidate = window.innerWidth - moveEvent.clientX;
      const max = window.innerWidth * MAX_PANEL_WIDTH_RATIO;
      panelWidth = Math.max(MIN_PANEL_WIDTH, Math.min(max, candidate));
      applyPanelWidth();
    };
    const onMouseUp = () => {
      document.removeEventListener("mousemove", onMouseMove);
      document.removeEventListener("mouseup", onMouseUp);
      els.resizePanelContent.classList.remove("dragging");
      document.body.classList.remove("resizing");
      localStorage.setItem(PANEL_WIDTH_KEY, String(Math.round(panelWidth)));
    };
    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("mouseup", onMouseUp);
  });
}

// ── dock open/closed ────────────────────────────────────────────────────
//
// Closed on every launch (not persisted) — the toolbar's 文件 button is a
// deliberate opt-in each session, not a state to restore. Only the width
// (once opened) is remembered, via panelWidth/PANEL_WIDTH_KEY above.

function setDockOpen(open) {
  els.panel.classList.toggle("hidden", !open);
  els.resizePanelContent.classList.toggle("hidden", !open);
  els.btnToolbarFiles.classList.toggle("active", open);
  if (open) refreshPanel();
}

function toggleDock() {
  setDockOpen(els.panel.classList.contains("hidden"));
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
  if (!entry.isDir && entry.path === currentPreviewPath) row.classList.add("tree-row-selected");

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
  } else if (!entry.isDir) {
    row.addEventListener("click", () => {
      if (currentPreviewPath === entry.path) {
        closePreview();
      } else {
        showPreview(entry.path);
      }
    });
  }
}

// ── file preview / edit (CodeMirror) ────────────────────────────────────
//
// Opening a file goes straight to an editable CodeMirror instance — no
// separate read-only "view" the user has to explicitly leave to edit. When
// the file has a git status worth comparing against (Modified/Deleted),
// @codemirror/merge's unifiedMergeView adds inline gutter decorations for
// the changed regions on top of that same editable pane (VS Code's own
// pattern: gutter markers are informational, not a second view to switch
// into), diffing client-side against the file's last-committed (HEAD)
// content — no more hand-rolled diff parsing on the Rust side.
//
// Two independent "did this change" baselines coexist deliberately:
//   1. git HEAD vs current content → the merge view's own gutter/inline
//      decorations. Unaffected by saving to disk (HEAD only moves on a
//      commit) — never needs refetching after a save.
//   2. last load-or-save vs the live, possibly-unsaved editor content →
//      this file's own dirty-dot/保存/还原 tracking, via currentSavedContent
//      below. Reset to "clean" on every successful save.
// "还原" only ever undoes (1) against baseline (2) — the current *editing
// session's* unsaved typing — never git's committed history. Conflating
// the two would make a UI button that reads as "undo my typing" silently
// capable of discarding a git-tracked change instead.

let currentPreviewPath = null;
let currentEditorView = null;
// The content as of the last successful load or save — see the baseline
// note above. `null` whenever no editor is mounted.
let currentSavedContent = null;
// Built once, lazily, and reused by every editor instance — colors are
// resolved via var(...) at paint time, so they already stay correct across
// a live prefers-color-scheme change without rebuilding this.
let cmBaseExtensions = null;

function buildCodeMirrorBaseExtensions() {
  if (cmBaseExtensions) return cmBaseExtensions;
  const CM = window.CM;
  const t = CM.tags;
  const highlightStyle = CM.HighlightStyle.define([
    { tag: t.comment, color: "var(--muted)", fontStyle: "italic" },
    { tag: [t.string, t.special(t.string)], color: "var(--success-text)" },
    { tag: [t.number, t.bool, t.null], color: "var(--danger)" },
    { tag: [t.keyword, t.controlKeyword, t.operatorKeyword, t.moduleKeyword], color: "var(--accent)" },
    { tag: [t.function(t.variableName), t.className, t.typeName], color: "var(--accent-tint-text)" },
    { tag: t.propertyName, color: "var(--text)" },
    { tag: t.punctuation, color: "var(--muted)" },
    { tag: t.tagName, color: "var(--accent)" },
    { tag: t.attributeName, color: "var(--accent-tint-text)" },
    { tag: t.invalid, color: "var(--danger)", textDecoration: "underline" },
  ]);

  const dark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const theme = CM.EditorView.theme(
    {
      "&": { height: "100%", fontSize: "11px", backgroundColor: "var(--card)", color: "var(--text)" },
      ".cm-content": {
        fontFamily: "'SF Mono','JetBrains Mono','Fira Code',Consolas,Menlo,monospace",
        caretColor: "var(--text)",
        padding: "8px 0",
      },
      ".cm-gutters": { backgroundColor: "var(--sidebar-bg)", color: "var(--muted)", border: "none" },
      ".cm-activeLine": { backgroundColor: "color-mix(in srgb, var(--accent) 6%, transparent)" },
      ".cm-activeLineGutter": { backgroundColor: "color-mix(in srgb, var(--accent) 10%, transparent)" },
      "&.cm-focused .cm-cursor": { borderLeftColor: "var(--text)" },
      ".cm-scroller": { overflow: "auto" },
      // @codemirror/merge's own decoration classes — overridden to this
      // project's tokens rather than its default hardcoded rgba() colors
      // (same "no new palette" rule already applied to every other color
      // in this file).
      ".cm-changedLine": { backgroundColor: "color-mix(in srgb, var(--accent) 10%, transparent)" },
      ".cm-changedLineGutter": { backgroundColor: "color-mix(in srgb, var(--accent) 18%, transparent)" },
      ".cm-changedText": { backgroundColor: "color-mix(in srgb, var(--accent) 22%, transparent)" },
      ".cm-deletedChunk": { backgroundColor: "color-mix(in srgb, var(--danger) 6%, transparent)" },
      ".cm-deletedLine": { backgroundColor: "color-mix(in srgb, var(--danger) 10%, transparent)" },
      ".cm-deletedLineGutter": { backgroundColor: "color-mix(in srgb, var(--danger) 18%, transparent)" },
      ".cm-deletedText": { background: "none", textDecoration: "line-through", color: "var(--danger)" },
      ".cm-insertedLine": { backgroundColor: "color-mix(in srgb, var(--success-text) 12%, transparent)" },
    },
    { dark },
  );

  cmBaseExtensions = [CM.syntaxHighlighting(highlightStyle), theme];
  return cmBaseExtensions;
}

// Extension → CodeMirror language extension, for the fixed package set
// bundled in vendor/codemirror. An unmapped extension (or none) just opens
// unhighlighted plain text rather than failing.
function languageExtensionForPath(path) {
  const CM = window.CM;
  if (!CM) return null;
  const L = CM.languages;
  const ext = path.slice(path.lastIndexOf(".") + 1).toLowerCase();
  switch (ext) {
    case "js": case "mjs": case "cjs": return L.javascript();
    case "jsx": return L.javascript({ jsx: true });
    case "ts": return L.javascript({ typescript: true });
    case "tsx": return L.javascript({ jsx: true, typescript: true });
    case "rs": return L.rust();
    case "py": return L.python();
    case "json": return L.json();
    case "css": return L.css();
    case "html": case "htm": return L.html();
    case "md": return L.markdown();
    case "yaml": case "yml": return L.yaml();
    case "sql": return L.sql();
    case "java": return L.java();
    case "go": return L.go();
    case "c": case "h": case "cpp": case "cc": case "hpp": case "cxx": return L.cpp();
    default: return null;
  }
}

function isDirty() {
  return currentEditorView !== null && currentSavedContent !== null && currentEditorView.state.doc.toString() !== currentSavedContent;
}

function setDirty(dirty) {
  els.panelPreviewDirtyDot.classList.toggle("hidden", !dirty);
  els.btnPreviewSave.classList.toggle("hidden", !dirty);
  els.btnPreviewRevert.classList.toggle("hidden", !dirty);
}

// True if it's safe to proceed with whatever's about to replace or close
// the current preview: nothing open, nothing unsaved, or the user
// explicitly confirmed discarding it. Never mutates state itself — the
// caller that gets `true` back is the one actually tearing down or
// replacing the editor.
function confirmDiscardIfNeeded() {
  if (!isDirty()) return true;
  return confirm("有未保存的改动，确定要放弃吗？");
}

function destroyEditor() {
  if (currentEditorView) {
    currentEditorView.destroy();
    currentEditorView = null;
  }
  currentSavedContent = null;
  setDirty(false);
}

function contentTextOrNull(fileContent) {
  return fileContent && fileContent.kind === "text" ? fileContent.content : null;
}

// `preview` is a Rust EditablePreview: { current: FileContent | null,
// original: FileContent | null }. `current` is null only for a git-Deleted
// file (nothing left on disk); `original` is null unless there's a HEAD
// version worth diffing against (Modified/Deleted).
function mountEditor(path, preview) {
  const CM = window.CM;
  destroyEditor();
  els.panelPreviewBody.replaceChildren();

  if (preview.current === null) {
    const originalText = contentTextOrNull(preview.original);
    if (originalText === null) {
      els.panelPreviewBody.textContent = "无法读取此文件的历史内容";
      return;
    }
    const extensions = [CM.basicSetup, ...buildCodeMirrorBaseExtensions(), CM.EditorState.readOnly.of(true)];
    const lang = languageExtensionForPath(path);
    if (lang) extensions.push(lang);
    currentEditorView = new CM.EditorView({ doc: originalText, extensions, parent: els.panelPreviewBody });
    currentSavedContent = originalText;
    return;
  }

  const current = preview.current;
  if (current.kind === "binary") {
    els.panelPreviewBody.textContent = "二进制文件，无法预览";
    return;
  }
  if (current.kind === "tooLarge") {
    els.panelPreviewBody.textContent = `文件过大（${(current.bytes / 1024 / 1024).toFixed(1)} MB），未加载预览`;
    return;
  }
  if (current.kind === "error") {
    els.panelPreviewBody.textContent = current.message;
    return;
  }

  const originalText = contentTextOrNull(preview.original);
  const lang = languageExtensionForPath(path);
  const extensions = [
    CM.basicSetup,
    ...buildCodeMirrorBaseExtensions(),
    CM.keymap.of([
      CM.indentWithTab,
      { key: "Mod-s", preventDefault: true, run: () => (saveCurrentEdit(), true) },
    ]),
    CM.EditorView.updateListener.of((update) => {
      if (update.docChanged) setDirty(isDirty());
    }),
  ];
  if (lang) extensions.push(lang);
  // gutter markers only, no per-chunk accept/reject buttons — this is an
  // ordinary editable file with an informational "differs from HEAD"
  // signal, not a merge-conflict resolution UI.
  if (originalText !== null) extensions.push(CM.unifiedMergeView({ original: originalText, mergeControls: false }));

  currentEditorView = new CM.EditorView({ doc: current.content, extensions, parent: els.panelPreviewBody });
  currentSavedContent = current.content;
  setDirty(false);
}

async function showPreview(path) {
  if (!confirmDiscardIfNeeded()) return;
  currentPreviewPath = path;
  els.cardFile.classList.remove("hidden");
  els.panelPreviewTitle.textContent = path;
  els.panelPreviewTitle.title = path;
  destroyEditor();
  els.panelPreviewBody.replaceChildren();
  const loading = document.createElement("p");
  loading.className = "muted panel-empty";
  loading.textContent = "加载中…";
  els.panelPreviewBody.appendChild(loading);

  try {
    const preview = await invoke("get_editable_preview", { path, overridePath: lockedWorkspace });
    // A slower load may resolve after the user already clicked a different
    // file (or closed the preview) — never let a stale response overwrite
    // whatever's actually being shown now.
    if (currentPreviewPath !== path) return;
    mountEditor(path, preview);
  } catch (err) {
    if (currentPreviewPath !== path) return;
    els.panelPreviewBody.textContent = `预览加载失败: ${err}`;
  }
}

// Re-fetches the open preview on the same cadence as the tree/git-status
// poll (called from refreshPanel) — the agent is very plausibly editing the
// exact file being previewed. Skipped entirely while there's an unsaved
// local edit: a background poll must never clobber that, and re-mounting a
// fresh editor on every tick would also reset cursor/scroll/undo history
// for no reason once the file has settled.
async function refreshCurrentPreview() {
  if (currentPreviewPath === null || isDirty()) return;
  const path = currentPreviewPath;
  try {
    const preview = await invoke("get_editable_preview", { path, overridePath: lockedWorkspace });
    if (currentPreviewPath !== path) return;
    mountEditor(path, preview);
  } catch {
    /* leave whatever's already showing rather than blanking it over a transient poll failure */
  }
}

function closePreview() {
  if (!confirmDiscardIfNeeded()) return;
  currentPreviewPath = null;
  destroyEditor();
  els.cardFile.classList.add("hidden");
  els.panelPreviewBody.replaceChildren();
}

function revertCurrentEdit() {
  if (!currentEditorView || currentSavedContent === null) return;
  if (!confirm("放弃当前改动，还原为上次保存的内容？")) return;
  currentEditorView.dispatch({
    changes: { from: 0, to: currentEditorView.state.doc.length, insert: currentSavedContent },
  });
  setDirty(false);
}

async function saveCurrentEdit() {
  if (!currentEditorView || currentPreviewPath === null) return;
  const path = currentPreviewPath;
  const content = currentEditorView.state.doc.toString();
  els.btnPreviewSave.disabled = true;
  try {
    await invoke("save_file_content", { path, content, overridePath: lockedWorkspace });
    currentSavedContent = content;
    setDirty(false);
    // The tree's git-status coloring should reflect a just-saved change
    // immediately, not after up to PANEL_POLL_MS — deliberately
    // refreshTreeAndGitStatus(), not the full refreshPanel(): HEAD hasn't
    // moved (a disk save isn't a commit), so the merge view's own gutter
    // decorations don't need refetching, and re-mounting the editor here
    // would reset cursor/scroll right after the user's own save action.
    refreshTreeAndGitStatus();
  } catch (err) {
    // Left exactly as the user typed it on failure — nothing is discarded
    // on a failed write.
    alert(`保存失败: ${err}`);
  } finally {
    els.btnPreviewSave.disabled = false;
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

// Split out from refreshPanel so saveCurrentEdit can refresh the tree's
// git-status coloring right after a save without also re-mounting the
// editor it just saved (see the comment on saveCurrentEdit).
async function refreshTreeAndGitStatus() {
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
    } else {
      for (const entry of tree) {
        renderTreeNode(entry, gitMap, els.panelTree);
      }
    }
  } catch (err) {
    els.panelTree.textContent = `无法加载文件树: ${err}`;
  }
}

async function refreshPanel() {
  await refreshTreeAndGitStatus();
  await refreshCurrentPreview();
}

// Cheap enough (one directory walk + one `git status`) to poll rather than
// stand up a real filesystem watcher for this first slice — see the plan
// note on deferring that complexity. Cleared on nothing; the container page
// itself is never torn down, so this interval simply runs for the app's
// whole lifetime.
const PANEL_POLL_MS = 6000;

// ── init ─────────────────────────────────────────────────────────────────

async function init() {
  applyPanelWidth();
  initResizeHandle();

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
  els.btnToolbarFiles.addEventListener("click", toggleDock);
  // Collapses the Files card's tree/picker body without closing the whole
  // dock — independent from #card-file's own close button, per the "each
  // card scrolls/collapses on its own" design.
  els.btnFilesCollapse.addEventListener("click", () => {
    const collapsed = els.cardFiles.classList.toggle("card-collapsed");
    els.btnFilesCollapse.textContent = collapsed ? "⌄" : "⌃";
  });
  els.btnPanelRefresh.addEventListener("click", refreshPanel);
  els.panelWorkspaceSelect.addEventListener("change", () => {
    const value = els.panelWorkspaceSelect.value;
    if (!confirmDiscardIfNeeded()) {
      // The <select>'s own DOM value already changed on click, ahead of
      // this handler — revert it to match the choice actually still in
      // effect, or the control would show a selection the app never adopted.
      els.panelWorkspaceSelect.value = lockedWorkspace ?? AUTO_OPTION_VALUE;
      return;
    }
    lockedWorkspace = value === AUTO_OPTION_VALUE ? null : value;
    if (lockedWorkspace === null) {
      localStorage.removeItem(LOCKED_WORKSPACE_KEY);
    } else {
      localStorage.setItem(LOCKED_WORKSPACE_KEY, lockedWorkspace);
    }
    // An open preview's path is relative to whichever workspace was active
    // when it was opened — re-resolving it against the new one could silently
    // show an unrelated (or nonexistent) file of the same relative path.
    closePreview();
    refreshPanel();
  });
  els.btnPreviewClose.addEventListener("click", closePreview);
  els.btnPreviewSave.addEventListener("click", saveCurrentEdit);
  els.btnPreviewRevert.addEventListener("click", revertCurrentEdit);

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
