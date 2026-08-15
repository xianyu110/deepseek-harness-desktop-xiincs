//! Workspace file tree and git status — powers the native side panel that
//! sits beside the embedded harness `<iframe>` in `ui/`. Independent of dsh:
//! reads the workspace directory directly and shells out to the user's own
//! `git`, the same pattern `server.rs` already uses for `node`/`npm`.
//!
//! There is no dsh-side HTTP API for either of these (confirmed against the
//! upstream harness's own `/api` gateway, which exposes no filesystem or git
//! surface) — the upstream project's own design notes place an in-app file
//! preview "in the desktop shell's own design, not [the harness's]", so this
//! module owns both independently rather than proxying anything dsh-side.
//!
//! It does read one piece of dsh's own state directly off disk, though: see
//! `active_workspace_dir` below for why `DSH_DESKTOP_CWD` alone isn't enough
//! to know what workspace the panel should show.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

/// Directory names never shown in the tree, regardless of workspace — `.git`
/// is never useful to browse here, and `node_modules`/`target` are the two
/// dependency/build directories common enough across arbitrary user
/// workspaces to be worth a hardcoded skip rather than requiring the user to
/// collapse them by hand every time. Deliberately not gitignore-aware (real
/// pattern matching, negation, and nested `.gitignore` files are a
/// meaningfully bigger scope than this first slice) — a workspace with other
/// large ignored directories still renders them today.
const IGNORED_DIR_NAMES: &[&str] = &[".git", "node_modules", "target"];
/// Caps a pathological workspace (a huge monorepo, or one of the ignored
/// names above not applying) from blocking the UI thread on a multi-second
/// directory walk.
const MAX_ENTRIES: usize = 3000;
const MAX_DEPTH: usize = 12;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TreeEntry {
    pub name: String,
    /// Relative to the workspace root, `/`-separated regardless of platform
    /// (matches `GitEntry::path` below, so the client can join tree entries
    /// against git status by exact string equality).
    pub path: String,
    pub is_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<TreeEntry>>,
}

pub fn list_workspace_tree(root: &Path) -> Vec<TreeEntry> {
    let mut budget = MAX_ENTRIES;
    read_dir_entries(root, root, 0, &mut budget)
}

fn read_dir_entries(root: &Path, dir: &Path, depth: usize, budget: &mut usize) -> Vec<TreeEntry> {
    if depth > MAX_DEPTH || *budget == 0 {
        return Vec::new();
    }
    let Ok(read_dir) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<_> = read_dir.filter_map(Result::ok).collect();
    // Directories first, then alphabetical within each group — stable,
    // predictable order for a tree UI (matches how most file browsers sort).
    entries.sort_by(|a, b| {
        let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_is_dir.cmp(&a_is_dir).then_with(|| a.file_name().cmp(&b.file_name()))
    });

    let mut out = Vec::new();
    for entry in entries {
        if *budget == 0 {
            break;
        }
        let Ok(file_type) = entry.file_type() else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = file_type.is_dir();
        if is_dir && IGNORED_DIR_NAMES.contains(&name.as_str()) {
            continue;
        }
        let full_path = entry.path();
        let Ok(rel) = full_path.strip_prefix(root) else { continue };
        let path = rel.to_string_lossy().replace('\\', "/");
        *budget -= 1;

        let children = is_dir.then(|| read_dir_entries(root, &full_path, depth + 1, budget));
        out.push(TreeEntry { name, path, is_dir, children });
    }
    out
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum GitStatus {
    Modified,
    Added,
    Deleted,
    Untracked,
}

#[derive(Serialize, Clone)]
pub struct GitEntry {
    pub path: String,
    pub status: GitStatus,
}

// ── active workspace ─────────────────────────────────────────────────────
//
// `DSH_DESKTOP_CWD` (`server::workspace_dir`) is only the directory the
// shell happened to be launched with — a Windows Explorer "open with" arg,
// or an env var. The harness has its own, separate, in-page "workspace"
// concept (Settings → Choose workspace) that a user can register many of
// and switch between freely, entirely inside the iframe, with no signal
// reaching this shell (that's the whole point of the zero-IPC boundary).
// Observed directly: those two can disagree — the shell panel kept showing
// its launch directory while the harness UI had a completely different
// workspace open.
//
// There is no live "the user is looking at session X right now" signal to
// chase, on either side of that boundary — confirmed by direct network
// inspection of a running instance (`read_network_requests` in the Browser
// tab, not a guess): the harness loads every session's and workspace's full
// metadata once at boot, then switching between *existing* sessions or
// expanding a workspace group in the sidebar fires zero further HTTP
// requests — it's pure client-side state. Corroborated from two more
// angles: no client-side URL routing (`pushState`/`useNavigate`/router
// exports all zero-hit across `packages/client` in
// deepseek-ai/deepseek-harness) and no existing iframe↔parent postMessage
// channel (the only "postMessage" hits in that repo are Web Worker
// messaging and a native dialog IPC helper, unrelated to this). So "most
// recently sent a message" (`session.list`'s `updatedAt`, below) is the
// ceiling of precision available without either breaking the iframe's
// origin isolation or the harness itself shipping a new integration hook —
// not a shortcut taken here.
//
// Two sources, tried in order:
//
// 1. `POST {harness url}/api/session.list` — the canonical, always-current
//    source, confirmed live: dsh's own internal unary RPC-over-HTTP wire
//    format (`rpc_call` below), cross-checked against
//    `packages/host/apiproxy/src/{fetch/handler.ts,api/sessions.schema.ts}`
//    in deepseek-ai/deepseek-harness. Only reachable once the server is up.
// 2. `<dsh home>/storages/workspace.json` — a plain JSON file dsh persists
//    itself, covering the window before the server is reachable. Its
//    `global.workspaceIds[0]` was verified by direct observation to match
//    the on-screen active workspace at one point in time, but — unlike
//    `session.list` — was later observed to lag ordinary session activity
//    (its `updatedAt` sat minutes behind `session_projcache.json`'s during
//    continued use), so it's kept only as the fallback for when there's no
//    live server to ask, not trusted as the primary signal.
//
// Both are undocumented, internal-shaped payloads in a product whose own
// README states "developer preview... THERE WILL BE COMPATIBILITY-BREAKING
// CHANGES" — not a stable contract either way. Every step is `Option`-
// chained so any mismatch (server unreachable, missing file, mid-write
// partial read, renamed field, restructured schema) falls through cleanly,
// and every caller falls back to `workspace_dir` — this must never be a
// hard requirement.

/// Extracts the most-recently-updated session's `cwd` from a `session.list`
/// response's `items` array. Sessions without a `cwd` (e.g. subagent-origin
/// ones, per the schema's `cwd?: string`) are skipped rather than winning a
/// comparison against an absent timestamp.
fn latest_session_cwd(items: &[serde_json::Value]) -> Option<PathBuf> {
    items
        .iter()
        .filter_map(|item| {
            let updated_at = item.get("updatedAt")?.as_i64()?;
            let cwd = item.get("cwd")?.as_str()?;
            Some((updated_at, cwd))
        })
        .max_by_key(|(updated_at, _)| *updated_at)
        .map(|(_, cwd)| PathBuf::from(cwd))
}

/// dsh's internal unary RPC-over-HTTP wire format: POST a
/// `{type: "client-request", rpcId, method, payload: {}}` envelope to
/// `{base_url}/api/<method>`, expect back `{type: "server-response", rpcId,
/// result: {ok: true, value} | {ok: false, error}}`. `rpcId` doesn't need
/// real uniqueness here: exactly one request is ever in flight on this
/// connection, nothing correlates concurrent replies against it.
fn rpc_call(base_url: &str, method: &str) -> Option<serde_json::Value> {
    let url = format!("{base_url}/api/{method}");
    let body = serde_json::json!({
        "type": "client-request",
        "rpcId": "dsh-desktop-panel",
        "method": method,
        "payload": {},
    });
    let response: serde_json::Value = ureq::post(&url)
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(3)))
        .build()
        .header("Content-Type", "application/json")
        .send_json(&body)
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    let result = response.get("result")?;
    if result.get("ok")?.as_bool()? {
        result.get("value").cloned()
    } else {
        None
    }
}

/// Core file-parsing logic, independent of `AppHandle` so it's directly
/// testable against a synthetic (or real, captured) `workspace.json` — see
/// `active_workspace_dir` for the entry point that supplies dsh's actual
/// storage directory.
fn parse_active_workspace(storages_dir: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(storages_dir.join("workspace.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let first_id = json.get("global")?.get("workspaceIds")?.as_array()?.first()?.as_str()?;
    let path = json.get("tables")?.get("workspaces")?.get(first_id)?.get("path")?.as_str()?;
    Some(PathBuf::from(path))
}

/// The harness's own currently-active workspace, when resolvable — see the
/// module-level note above.
pub fn active_workspace_dir(app: &tauri::AppHandle, server: &crate::server::Shared) -> Option<PathBuf> {
    if let Some(url) = crate::server::running_url(server) {
        if let Some(items) = rpc_call(&url, "session.list").and_then(|v| v.get("items")?.as_array().cloned()) {
            if let Some(cwd) = latest_session_cwd(&items) {
                return Some(cwd);
            }
        }
    }
    parse_active_workspace(&crate::server::dsh_home_dir(app).join("storages"))
}

/// What the panel should actually show: the harness's active workspace when
/// resolvable, else the directory this shell was launched with/spawns the
/// server in.
pub fn effective_workspace_dir(app: &tauri::AppHandle, server: &crate::server::Shared) -> PathBuf {
    active_workspace_dir(app, server).unwrap_or_else(|| crate::server::workspace_dir(app))
}

// ── known workspaces (manual picker) ─────────────────────────────────────
//
// The auto-inference above has a real, principled ceiling (see the note
// above it): nothing observes a pure "switched which existing session I'm
// looking at" action, only "sent a message in it". For a user who wants the
// panel pinned to a specific project regardless of that, the client
// (`ui/app.js`) offers a manual picker populated from this list and passes
// its choice back to `get_workspace_tree`/`get_git_status` as an explicit
// override — see those commands in `lib.rs`. This function only supplies
// the *options*; which one is currently selected is UI state the client
// owns and persists itself, not something Rust tracks.

#[derive(Serialize, Clone)]
pub struct WorkspaceOption {
    pub path: String,
    pub title: String,
}

fn parse_workspace_option(item: &serde_json::Value) -> Option<WorkspaceOption> {
    let path = item.get("path")?.as_str()?.to_string();
    let title = item.get("title")?.as_str()?.to_string();
    Some(WorkspaceOption { path, title })
}

/// Every workspace dsh currently knows about, `session.list`-API first
/// (`workspace.list`'s `items` are already in the server's own order — the
/// live source `active_workspace_dir` also prefers), falling back to the
/// persisted file when the server isn't reachable.
pub fn known_workspaces(app: &tauri::AppHandle, server: &crate::server::Shared) -> Vec<WorkspaceOption> {
    if let Some(url) = crate::server::running_url(server) {
        if let Some(items) = rpc_call(&url, "workspace.list").and_then(|v| v.get("items")?.as_array().cloned()) {
            let options: Vec<WorkspaceOption> = items.iter().filter_map(parse_workspace_option).collect();
            if !options.is_empty() {
                return options;
            }
        }
    }
    file_workspace_options(&crate::server::dsh_home_dir(app).join("storages"))
}

/// Core file-parsing logic, independent of `AppHandle` — mirrors
/// `parse_active_workspace`'s fallback but returns every entry (in
/// `workspaceIds` order) rather than just the first.
fn file_workspace_options(storages_dir: &Path) -> Vec<WorkspaceOption> {
    let Ok(text) = fs::read_to_string(storages_dir.join("workspace.json")) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let Some(ids) = json.get("global").and_then(|g| g.get("workspaceIds")).and_then(|w| w.as_array()) else {
        return Vec::new();
    };
    let Some(workspaces) = json.get("tables").and_then(|t| t.get("workspaces")) else {
        return Vec::new();
    };
    ids.iter()
        .filter_map(|id| id.as_str())
        .filter_map(|id| workspaces.get(id))
        .filter_map(parse_workspace_option)
        .collect()
}

/// Runs `git status` in `cwd` and returns per-path status. Returns an empty
/// list — not an error — when `cwd` isn't a git repository, or `git` isn't
/// on PATH: the file tree above must stay usable either way, git status
/// coloring is a bonus overlay, not a precondition.
///
/// `--no-renames`: porcelain `-z` rename records put the *new* path in one
/// NUL-separated field and the *old* path in the next, and getting that
/// order backwards would silently mislabel a renamed file under its old
/// name. Rather than trust an unverified recollection of that field order,
/// renames are reported as a plain delete + add pair instead — the accurate
/// simplification given this slice's own testing already exercises the
/// modify/add/delete/untracked cases and no more.
pub fn git_status(cwd: &Path) -> Vec<GitEntry> {
    let mut cmd = Command::new("git");
    cmd.args(["status", "--porcelain=v1", "--untracked-files=all", "--no-renames", "-z"]);
    cmd.current_dir(crate::server::plain_win_path(cwd));
    crate::server::hide_console(&mut cmd);
    let Ok(output) = cmd.output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.split('\0').filter(|record| record.len() > 3).filter_map(parse_porcelain_record).collect()
}

fn parse_porcelain_record(record: &str) -> Option<GitEntry> {
    let mut chars = record.chars();
    let x = chars.next()?;
    let y = chars.next()?;
    let path = record.get(3..)?.replace('\\', "/");
    let status = if x == '?' && y == '?' {
        GitStatus::Untracked
    } else if x == 'A' {
        GitStatus::Added
    } else if x == 'D' || y == 'D' {
        GitStatus::Deleted
    } else {
        GitStatus::Modified
    };
    Some(GitEntry { path, status })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git").current_dir(dir).args(args).status().expect("git must be on PATH to run this test");
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    }

    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("dsh-desktop-panel-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // Trimmed from a real, captured `POST /api/session.list` response body
    // (read live off a running instance's Network tab; irrelevant
    // `projections`/`agentPreset`/etc fields dropped since this parser only
    // reads `updatedAt`/`cwd`) — three sessions across two workspaces, the
    // middle one most recently updated despite not being listed last.
    fn real_session_list_items() -> Vec<serde_json::Value> {
        serde_json::from_str(
            r#"[
              {"sessionId": "session-e5e407e6", "updatedAt": 1786774994999, "cwd": "E:\\Project202608\\ExampleProject001"},
              {"sessionId": "session-0f2b0cdf", "updatedAt": 1786775697190, "cwd": "E:\\Project202608\\ExampleProject001"},
              {"sessionId": "session-2390bcfc", "updatedAt": 1786733386399, "cwd": "E:\\Project202608\\HappyTime"}
            ]"#,
        )
        .unwrap()
    }

    #[test]
    fn latest_session_cwd_picks_the_max_updated_at_not_list_order() {
        let items = real_session_list_items();
        assert_eq!(
            latest_session_cwd(&items),
            Some(PathBuf::from(r"E:\Project202608\ExampleProject001"))
        );
    }

    #[test]
    fn latest_session_cwd_skips_entries_without_a_cwd() {
        let items: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
              {"sessionId": "session-subagent", "updatedAt": 9999999999999},
              {"sessionId": "session-real", "updatedAt": 1, "cwd": "E:\\only\\real\\one"}
            ]"#,
        )
        .unwrap();
        assert_eq!(latest_session_cwd(&items), Some(PathBuf::from(r"E:\only\real\one")));
    }

    #[test]
    fn latest_session_cwd_of_empty_list_is_none() {
        assert_eq!(latest_session_cwd(&[]), None);
    }

    // Trimmed from a real, captured `~/.dsh/storages/workspace.json` (seven
    // registered workspaces; `sessionIds`/timestamps/session-scoped data
    // dropped since this parser never reads them) — real field names and
    // real nesting, not a hand-typed guess at the shape.
    const REAL_WORKSPACE_JSON: &str = r#"{
      "unit": { "name": "workspace", "version": 2 },
      "global": {
        "initialized": true,
        "workspaceIds": [
          "bf8d8e2c-1049-4f40-bac4-f6ffe614aa59",
          "13d72047-972b-42cf-98e3-1c3605728319",
          "1c98bf74-32e0-4ad5-b0c0-66906a4778a4"
        ],
        "archivedSessionIds": []
      },
      "tables": {
        "workspaces": {
          "1c98bf74-32e0-4ad5-b0c0-66906a4778a4": {
            "path": "E:\\Project202608\\dsh-desktop",
            "title": "dsh-desktop"
          },
          "13d72047-972b-42cf-98e3-1c3605728319": {
            "path": "E:\\Project202608\\HappyTime",
            "title": "HappyTime"
          },
          "bf8d8e2c-1049-4f40-bac4-f6ffe614aa59": {
            "path": "E:\\Project202608\\ExampleProject001",
            "title": "ExampleProject001"
          }
        }
      }
    }"#;

    #[test]
    fn parses_the_first_workspace_ids_entry_from_a_real_captured_file() {
        let dir = scratch_dir("workspace-json");
        fs::write(dir.join("workspace.json"), REAL_WORKSPACE_JSON).unwrap();

        assert_eq!(
            parse_active_workspace(&dir),
            Some(PathBuf::from(r"E:\Project202608\ExampleProject001"))
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_workspace_options_returns_all_entries_in_workspace_ids_order() {
        let dir = scratch_dir("workspace-json-options");
        fs::write(dir.join("workspace.json"), REAL_WORKSPACE_JSON).unwrap();

        let options = file_workspace_options(&dir);
        let titles: Vec<_> = options.iter().map(|o| o.title.as_str()).collect();
        // Matches workspaceIds order in REAL_WORKSPACE_JSON, not tables.workspaces'
        // (arbitrary, UUID-keyed) object-key order.
        assert_eq!(titles, vec!["ExampleProject001", "HappyTime", "dsh-desktop"]);
        assert_eq!(options[0].path, r"E:\Project202608\ExampleProject001");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_workspace_options_on_missing_file_is_empty() {
        let dir = scratch_dir("workspace-json-options-missing");
        assert!(file_workspace_options(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_workspace_json_returns_none_not_an_error() {
        let dir = scratch_dir("workspace-json-missing");
        assert_eq!(parse_active_workspace(&dir), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_workspace_json_returns_none_not_a_panic() {
        let dir = scratch_dir("workspace-json-malformed");
        fs::write(dir.join("workspace.json"), "{ this is not valid json").unwrap();
        assert_eq!(parse_active_workspace(&dir), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_workspace_ids_returns_none() {
        let dir = scratch_dir("workspace-json-empty");
        fs::write(
            dir.join("workspace.json"),
            r#"{"global": {"workspaceIds": []}, "tables": {"workspaces": {}}}"#,
        )
        .unwrap();
        assert_eq!(parse_active_workspace(&dir), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn git_status_reports_modified_deleted_untracked_against_a_real_commit() {
        let dir = scratch_dir("status");
        git(&dir, &["init", "-q"]);
        git(&dir, &["config", "user.email", "test@example.com"]);
        git(&dir, &["config", "user.name", "test"]);

        fs::write(dir.join("keep.txt"), "a").unwrap();
        fs::write(dir.join("gone.txt"), "b").unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "init"]);

        fs::write(dir.join("keep.txt"), "a-changed").unwrap();
        fs::remove_file(dir.join("gone.txt")).unwrap();
        fs::write(dir.join("new.txt"), "c").unwrap();

        let entries = git_status(&dir);
        let find = |p: &str| entries.iter().find(|e| e.path == p).map(|e| e.status);

        assert_eq!(find("keep.txt"), Some(GitStatus::Modified));
        assert_eq!(find("gone.txt"), Some(GitStatus::Deleted));
        assert_eq!(find("new.txt"), Some(GitStatus::Untracked));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn git_status_on_a_non_git_directory_is_empty_not_an_error() {
        let dir = scratch_dir("nogit");
        assert!(git_status(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tree_skips_ignored_dir_names_and_sorts_directories_before_files() {
        let dir = scratch_dir("tree");
        fs::create_dir_all(dir.join("node_modules")).unwrap();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("a.txt"), "").unwrap();
        fs::write(dir.join("src").join("main.rs"), "").unwrap();

        let tree = list_workspace_tree(&dir);
        let names: Vec<_> = tree.iter().map(|e| e.name.as_str()).collect();

        assert!(!names.contains(&"node_modules"));
        assert_eq!(names, vec!["src", "a.txt"]);
        assert_eq!(tree[0].path, "src");
        assert_eq!(tree[0].children.as_ref().unwrap()[0].path, "src/main.rs");

        let _ = fs::remove_dir_all(&dir);
    }
}
