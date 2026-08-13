//! dsh server manager.
//!
//! Responsibilities:
//! - resolve the Node executable (explicit override → bundled runtime → PATH)
//! - resolve the `dsh` `lib/bin.js` (explicit override → bundled runtime →
//!   per-user managed runtime, installed on first use)
//! - probe `127.0.0.1:3080` and attach to an already-running harness instead
//!   of spawning a second instance (avoids concurrent writers on `~/.dsh`)
//! - spawn `node <bin> web --port …` and discover the real URL from the
//!   printed `dsh web: http://127.0.0.1:<port>` line
//! - navigate the main webview to the URL, watch the process, auto-restart
//!   once per 60s window on unexpected exit, and surface errors to the boot page
//! - clean up the whole process tree (`taskkill /T /F`) on stop

use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

pub const DEFAULT_PORT: u16 = 3080;

/// Default npm version spec for the managed `@deepseek-ai/dsh` runtime.
const DSH_VERSION_DEFAULT: &str = "0.1.0-rc.6";
/// Marker found verbatim in the harness index page (served uncompressed).
const INDEX_MARKER: &str = "DeepSeek Harness";
/// Max lines kept in the in-memory log ring buffer.
const LOG_CAP: usize = 400;
/// The URL line printed by the web profile (`dsh-web-app`, `printUrl: true`).
const URL_PREFIX: &str = "dsh web: http://127.0.0.1:";
/// Minimum gap between automatic restarts of a crashing server.
const AUTO_RESTART_MIN_GAP: Duration = Duration::from_secs(60);
/// How long to watch for a quick EADDRINUSE exit after spawning on 3080.
const EADDRINUSE_WATCH: Duration = Duration::from_secs(6);

/// Optional persistent log file, set once via [`init_log_file`] at startup.
static LOG_FILE: OnceLock<PathBuf> = OnceLock::new();

/// Point the persistent log at a file (called once at app setup).
pub fn init_log_file(path: PathBuf) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = LOG_FILE.set(path);
}

fn append_log_file(line: &str) {
    if let Some(path) = LOG_FILE.get() {
        use std::io::Write as _;
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{line}");
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ServerStatus {
    Idle,
    Starting { detail: String },
    Running { url: String },
    Stopped { code: Option<i32>, message: Option<String> },
    Error { message: String },
}

pub struct DshServer {
    pub status: ServerStatus,
    pub logs: VecDeque<String>,
    pub boot_url: Option<String>,
    pid: Option<u32>,
    install_pid: Option<u32>,
    requested_stop: bool,
    last_auto_restart: Option<Instant>,
    node: Option<String>,
    bin: Option<String>,
}

impl Default for DshServer {
    fn default() -> Self {
        Self {
            status: ServerStatus::Idle,
            logs: VecDeque::new(),
            boot_url: None,
            pid: None,
            install_pid: None,
            requested_stop: false,
            last_auto_restart: None,
            node: None,
            bin: None,
        }
    }
}

pub type Shared = Arc<Mutex<DshServer>>;

// ── status / logs ────────────────────────────────────────────────────────────

fn set_status(app: &AppHandle, server: &Shared, status: ServerStatus) {
    {
        let mut s = server.lock().unwrap();
        s.status = status.clone();
    }
    let _ = app.emit("server-status", status);
}

fn push_log(server: &Shared, line: String) {
    let mut s = server.lock().unwrap();
    s.logs.push_back(line.clone());
    while s.logs.len() > LOG_CAP {
        s.logs.pop_front();
    }
    drop(s);
    append_log_file(&line);
}

pub fn log_tail(server: &Shared, n: usize) -> Vec<String> {
    let s = server.lock().unwrap();
    let n = n.min(s.logs.len());
    s.logs.iter().skip(s.logs.len() - n).cloned().collect()
}

pub fn running_url(server: &Shared) -> Option<String> {
    match &server.lock().unwrap().status {
        ServerStatus::Running { url } => Some(url.clone()),
        _ => None,
    }
}

// ── paths & environment ──────────────────────────────────────────────────────

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Tauri's `PathResolver` returns verbatim (`\\?\C:\...`) paths on Windows.
/// Node.js chokes on those for module resolution, so convert to the plain
/// `C:\...` form before passing anything to a child process.
fn plain_win_path(p: &Path) -> String {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s.into_owned()
    }
}

fn resolve_home(app: &AppHandle) -> PathBuf {
    app.path().home_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// The harness data root: `$DSH_HOME` (with `~/` expansion) or `~/.dsh`.
pub fn dsh_home_dir(app: &AppHandle) -> PathBuf {
    let home = resolve_home(app);
    if let Some(h) = env_nonempty("DSH_HOME") {
        if h == "~" {
            return home;
        }
        if let Some(rest) = h.strip_prefix("~/").or_else(|| h.strip_prefix("~\\")) {
            return home.join(rest);
        }
        return PathBuf::from(h);
    }
    home.join(".dsh")
}

/// Where the managed `@deepseek-ai/dsh` runtime lives (per-user, cache dir).
fn runtime_dir(app: &AppHandle) -> PathBuf {
    if let Some(dir) = env_nonempty("DSH_DESKTOP_RUNTIME_DIR") {
        return PathBuf::from(dir);
    }
    let cache = app
        .path()
        .app_cache_dir()
        .unwrap_or_else(|_| resolve_home(app));
    cache.join("runtime")
}

fn runtime_bin_path(rd: &Path) -> PathBuf {
    rd.join("node_modules")
        .join("@deepseek-ai")
        .join("dsh")
        .join("lib")
        .join("bin.js")
}

// ── resolution ───────────────────────────────────────────────────────────────

fn resolve_node(app: &AppHandle) -> Result<String, String> {
    if let Some(node) = env_nonempty("DSH_DESKTOP_NODE") {
        if Path::new(&node).exists() {
            return Ok(plain_win_path(Path::new(&node)));
        }
        return Err(format!("DSH_DESKTOP_NODE 指向的文件不存在: {node}"));
    }
    // bundled runtime shipped with packaged builds
    if let Ok(res) = app.path().resource_dir() {
        let bundled = res.join("runtime").join("node.exe");
        if bundled.exists() {
            return Ok(plain_win_path(&bundled));
        }
    }
    // system node from PATH
    match Command::new("node").arg("--version").output() {
        Ok(out) if out.status.success() => Ok("node".to_string()),
        _ => Err(
            "未检测到 Node.js：请安装 Node.js（https://nodejs.org），\
             或在环境变量 DSH_DESKTOP_NODE 中指定 node.exe 的路径。"
                .to_string(),
        ),
    }
}

fn resolve_bin(app: &AppHandle, server: &Shared) -> Result<String, String> {
    if let Some(bin) = env_nonempty("DSH_DESKTOP_DSH_BIN") {
        let p = PathBuf::from(&bin);
        if p.exists() {
            push_log(server, format!("使用 DSH_DESKTOP_DSH_BIN: {bin}"));
            return Ok(plain_win_path(&p));
        }
        return Err(format!("DSH_DESKTOP_DSH_BIN 指向的文件不存在: {bin}"));
    }
    // bundled runtime shipped with packaged builds
    if let Ok(res) = app.path().resource_dir() {
        let bundled = runtime_bin_path(&res.join("runtime"));
        if bundled.exists() {
            return Ok(plain_win_path(&bundled));
        }
    }
    let rd = runtime_dir(app);
    let bin = runtime_bin_path(&rd);
    if !bin.exists() {
        install_runtime(app, server, &rd)?;
    }
    if !bin.exists() {
        return Err(format!("dsh 运行时安装失败（{}），请查看日志。", rd.display()));
    }
    Ok(plain_win_path(&bin))
}

fn install_runtime(app: &AppHandle, server: &Shared, rd: &Path) -> Result<(), String> {
    let version = env_nonempty("DSH_DESKTOP_DSH_VERSION").unwrap_or_else(|| DSH_VERSION_DEFAULT.to_string());
    let target = format!("@deepseek-ai/dsh@{version}");
    fs::create_dir_all(rd).map_err(|e| format!("无法创建运行时目录 {}: {e}", rd.display()))?;
    push_log(server, format!("首次使用：安装 dsh 运行时 ({target}) → {}", rd.display()));
    set_status(
        app,
        server,
        ServerStatus::Starting {
            detail: format!("安装 dsh 运行时 ({version})…"),
        },
    );
    let mut output = Command::new("cmd")
        .args([
            "/C",
            "npm",
            "install",
            "--prefix",
            &plain_win_path(rd),
            &target,
            "--omit=dev",
            "--no-audit",
            "--no-fund",
            "--no-progress",
            "--prefer-offline",
            "--fetch-retries=5",
            "--fetch-retry-mintimeout=2000",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法运行 npm install: {e}"))?;

    // Stream npm output into the log ring buffer so first-run installs
    // (hundreds of MB) are visible instead of silent.
    let install_pid = output.id();
    {
        let mut s = server.lock().unwrap();
        s.install_pid = Some(install_pid);
    }
    let stdout = output.stdout.take().expect("stdout pipe");
    let stderr = output.stderr.take().expect("stderr pipe");
    let s1 = server.clone();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            push_log(&s1, line);
        }
    });
    let s2 = server.clone();
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            push_log(&s2, line);
        }
    });
    // Poll wait so `stop()` (which taskkills `install_pid`) is honoured even
    // while the install is running.
    let status = loop {
        match output.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(300)),
            Err(e) => return Err(format!("npm install 进程错误: {e}")),
        }
    };
    {
        let mut s = server.lock().unwrap();
        s.install_pid = None;
    }
    if !status.success() {
        return Err(format!("npm install 失败（exit {:?}），请查看日志。", status.code()));
    }
    Ok(())
}

// ── probing & spawning ───────────────────────────────────────────────────────

/// Cheap dependency-free HTTP probe: does `http://host:port/` serve the
/// harness index page?
fn probe_dsh(url: &str) -> bool {
    let rest = url.strip_prefix("http://").unwrap_or(url);
    let (host, port) = match rest.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().unwrap_or(80)),
        None => (rest, 80),
    };
    let Ok(mut addrs) = (host, port).to_socket_addrs() else {
        return false;
    };
    let Some(sockaddr) = addrs.next() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&sockaddr, Duration::from_millis(1200)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(1500)));
    let req = format!(
        "GET / HTTP/1.1\r\nHost: {host}:{port}\r\nAccept-Encoding: identity\r\nConnection: close\r\nUser-Agent: dsh-desktop\r\n\r\n"
    );
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.len() > 131072 {
                    break;
                }
            }
        }
    }
    String::from_utf8_lossy(&buf).contains(INDEX_MARKER)
}

fn set_running(app: &AppHandle, server: &Shared, url: &str) {
    println!("[dsh-desktop] server running at {url}");
    {
        let mut s = server.lock().unwrap();
        s.status = ServerStatus::Running {
            url: url.to_string(),
        };
    }
    let _ = app.emit(
        "server-status",
        ServerStatus::Running {
            url: url.to_string(),
        },
    );
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.navigate(url.parse().expect("valid http url"));
    }
}

pub fn navigate_boot(app: &AppHandle, server: &Shared) {
    let boot = server.lock().unwrap().boot_url.clone();
    if let Some(url) = boot {
        if let Some(win) = app.get_webview_window("main") {
            let _ = win.navigate(url.parse().expect("valid boot url"));
        }
    }
}

/// Spawn the server process and start its reader/exit-watcher threads.
fn spawn(app: &AppHandle, server: &Shared, node: &str, bin: &str, port: u16) -> Result<(), String> {
    let cwd = env_nonempty("DSH_DESKTOP_CWD")
        .map(PathBuf::from)
        .unwrap_or_else(|| resolve_home(app));
    let cwd_plain = plain_win_path(&cwd);
    fs::create_dir_all(&cwd).map_err(|e| format!("无法创建工作目录 {}: {e}", cwd.display()))?;

    push_log(
        server,
        format!("启动: {node} {bin} web --port {port}  (cwd: {cwd_plain})"),
    );

    let mut cmd = Command::new(node);
    cmd.arg(bin).arg("web").arg("--port").arg(port.to_string());
    cmd.current_dir(&cwd_plain);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("启动 dsh 失败: {e}"))?;

    let pid = child.id();
    println!("[dsh-desktop] spawned dsh pid {pid} on port {port}");
    {
        let mut s = server.lock().unwrap();
        s.pid = Some(pid);
        s.requested_stop = false;
        s.node = Some(node.to_string());
        s.bin = Some(bin.to_string());
    }
    set_status(
        app,
        server,
        ServerStatus::Starting {
            detail: format!("服务启动中 (端口 {port})…"),
        },
    );

    let stdout = child.stdout.take().expect("stdout pipe");
    let stderr = child.stderr.take().expect("stderr pipe");

    // stdout reader: capture logs and discover the printed URL line
    let app2 = app.clone();
    let srv2 = server.clone();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            push_log(&srv2, line.clone());
            if let Some(idx) = line.find(URL_PREFIX) {
                let rest = &line[idx + URL_PREFIX.len()..];
                let port: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !port.is_empty() {
                    let url = format!("http://127.0.0.1:{port}");
                    set_running(&app2, &srv2, &url);
                }
            }
        }
    });

    // stderr reader
    let srv3 = server.clone();
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            push_log(&srv3, line);
        }
    });

    // exit watcher: auto-restart once per 60s window, then surface the error
    let app3 = app.clone();
    let srv4 = server.clone();
    thread::spawn(move || {
        let exit = child.wait();
        let code = exit.ok().and_then(|st| st.code());
        let requested = {
            let mut s = srv4.lock().unwrap();
            s.pid = None;
            s.requested_stop
        };
        if requested {
            set_status(&app3, &srv4, ServerStatus::Stopped { code, message: None });
            return;
        }
        push_log(&srv4, format!("dsh 服务退出 (exit {code:?})"));

        let allow_restart = {
            let s = srv4.lock().unwrap();
            match s.last_auto_restart {
                Some(t) => t.elapsed() >= AUTO_RESTART_MIN_GAP,
                None => true,
            }
        };
        if allow_restart {
            {
                let mut s = srv4.lock().unwrap();
                s.last_auto_restart = Some(Instant::now());
            }
            set_status(
                &app3,
                &srv4,
                ServerStatus::Error {
                    message: format!("dsh 服务异常退出 (exit {code:?})，1 秒后自动重启…"),
                },
            );
            thread::sleep(Duration::from_secs(1));
            let _ = start(&app3, &srv4);
        } else {
            let status = ServerStatus::Error {
                message: format!("dsh 服务异常退出 (exit {code:?})，自动重启已停止，请手动重试。"),
            };
            {
                let mut s = srv4.lock().unwrap();
                s.status = status.clone();
            }
            let _ = app3.emit("server-status", status);
            navigate_boot(&app3, &srv4);
        }
    });

    Ok(())
}

// ── public API ───────────────────────────────────────────────────────────────

/// Stop the server: mark requested, then kill the whole process tree.
pub fn stop(server: &Shared) {
    let (pid, install_pid) = {
        let mut s = server.lock().unwrap();
        s.requested_stop = true;
        (s.pid.take(), s.install_pid.take())
    };
    if let Some(pid) = pid {
        println!("[dsh-desktop] stopping dsh pid {pid} (process tree)");
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status();
    }
    if let Some(pid) = install_pid {
        println!("[dsh-desktop] stopping runtime install pid {pid}");
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status();
    }
}

/// The default bind port; `DSH_DESKTOP_PORT` overrides it (useful for
/// running several desktop instances side by side for testing).
pub fn default_port() -> u16 {
    env_nonempty("DSH_DESKTOP_PORT")
        .and_then(|p| p.parse::<u16>().ok())
        .filter(|p| *p > 0)
        .unwrap_or(DEFAULT_PORT)
}

/// Start (or attach to) the harness server. Never blocks for long: spawning
/// and URL discovery happen on worker threads started inside. Failures are
/// surfaced as `ServerStatus::Error` so the boot page always shows a reason.
pub fn start(app: &AppHandle, server: &Shared) -> Result<(), String> {
    {
        let s = server.lock().unwrap();
        if matches!(s.status, ServerStatus::Running { .. } | ServerStatus::Starting { .. }) {
            return Ok(());
        }
    }

    let result = start_inner(app, server);
    if let Err(msg) = &result {
        push_log(server, format!("启动失败: {msg}"));
        set_status(app, server, ServerStatus::Error { message: msg.clone() });
    }
    result
}

fn start_inner(app: &AppHandle, server: &Shared) -> Result<(), String> {
    // Attach to an already-running harness on the default port instead of
    // spawning a second instance (two servers would race on ~/.dsh). This
    // happens before resolving node/runtime so attach mode needs nothing.
    let port = default_port();
    let default_url = format!("http://127.0.0.1:{port}");
    if probe_dsh(&default_url) {
        push_log(server, format!("检测到已在运行的 dsh 服务，直接使用 {default_url}"));
        set_running(app, server, &default_url);
        return Ok(());
    }

    let node = resolve_node(app)?;
    let bin = resolve_bin(app, server)?;
    push_log(server, format!("node: {node}"));
    push_log(server, format!("bin:  {bin}"));

    spawn(app, server, &node, &bin, port)?;

    // If the port is taken by a non-dsh process, the child exits quickly with
    // EADDRINUSE; fall back to an OS-assigned port (the URL line is parsed
    // from stdout by the reader thread).
    let deadline = Instant::now() + EADDRINUSE_WATCH;
    loop {
        let (running, exited, eaddr) = {
            let s = server.lock().unwrap();
            (
                matches!(s.status, ServerStatus::Running { .. }),
                s.pid.is_none(),
                s.logs.iter().any(|l| l.contains("EADDRINUSE")),
            )
        };
        if running {
            return Ok(());
        }
        if exited && eaddr {
            push_log(server, "端口 3080 被其他程序占用，改用系统分配端口…".to_string());
            spawn(app, server, &node, &bin, 0)?;
            return Ok(());
        }
        if Instant::now() > deadline {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(200));
    }
}

/// Info for the boot page footer.
pub fn info(app: &AppHandle, server: &Shared) -> serde_json::Value {
    let (node, bin) = {
        let s = server.lock().unwrap();
        (s.node.clone(), s.bin.clone())
    };
    let dsh_version = env_nonempty("DSH_DESKTOP_DSH_VERSION").unwrap_or_else(|| DSH_VERSION_DEFAULT.to_string());
    serde_json::json!({
        "dshVersion": dsh_version,
        "nodePath": node.unwrap_or_else(|| "未检测到".to_string()),
        "binPath": bin.unwrap_or_default(),
        "dshHome": dsh_home_dir(app).to_string_lossy(),
        "runtimeDir": runtime_dir(app).to_string_lossy(),
    })
}
