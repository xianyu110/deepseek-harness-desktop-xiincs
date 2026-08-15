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

use std::fs;
use std::path::Path;
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
