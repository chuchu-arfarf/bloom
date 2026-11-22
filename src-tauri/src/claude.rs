//! Claude panel: talks to the Claude Code program the user already has
//! installed, in its streaming JSON mode. Claude Code signs in with the user's
//! own Claude plan; Bloom never sees credentials.
//!
//! Two conversations live here for the whole run: "chat" (web search only, no
//! file access) and "code" (works in a project folder and asks before editing
//! files or running commands). The panel window can be closed and reopened
//! without losing them.
use crate::claude_protocol::{self as protocol, Event, ToolCall, TurnError, UsageInfo};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashSet, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder};

const WINDOW_LABEL: &str = "claude";
const STATE_EVENT: &str = "claude-state";
const USAGE_EVENT: &str = "claude-usage";
const PANEL_SESSION: &str = "bloom-panel";
const USAGE_INTERVAL: Duration = Duration::from_secs(5 * 60);
const USAGE_TIMEOUT: Duration = Duration::from_secs(45);
const CHAT_PROMPT: &str =
    "You are answering inside Bloom, a small chat panel on the user's Windows \
desktop. Keep answers concise; light Markdown is fine.";

#[derive(Clone, Copy, Serialize, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Idle,
    Working,
    NeedsApproval,
    Error,
}

/// One entry in a conversation. Text from Claude or the user is shown as-is;
/// everything else is worded by the UI's translations.
#[derive(Clone, Serialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Item {
    User {
        text: String,
    },
    Assistant {
        text: String,
    },
    Tool {
        call: ToolCall,
    },
    Decision {
        allowed: bool,
        call: ToolCall,
    },
    /// `text` is Claude Code's own message; `code` is a known reason the UI translates.
    Error {
        text: Option<String>,
        code: Option<String>,
    },
}

#[derive(Clone, Serialize, Debug)]
pub struct Approval {
    request_id: String,
    call: ToolCall,
    #[serde(skip)]
    input: Value,
}

#[derive(Default)]
struct Session {
    items: Vec<Item>,
    status: Status,
    approval: Option<Approval>,
    session_id: Option<String>,
    folder: Option<String>,
    always_allow: HashSet<String>,
    stdin: Option<ChildStdin>,
    child: Option<Child>,
    generation: u64,
    stderr: Arc<Mutex<VecDeque<String>>>,
}

#[derive(Serialize, Clone)]
pub struct SessionView {
    items: Vec<Item>,
    status: Status,
    approval: Option<Approval>,
    folder: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct PanelState {
    chat: SessionView,
    code: SessionView,
    installed: bool,
}

#[derive(Default)]
struct Panel {
    chat: Session,
    code: Session,
    folder_loaded: bool,
}

static PANEL: LazyLock<Mutex<Panel>> = LazyLock::new(|| Mutex::new(Panel::default()));
static USAGE: Mutex<Option<UsageInfo>> = Mutex::new(None);
static WINDOW_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Chat,
    Code,
}

impl Tab {
    fn parse(tab: &str) -> Tab {
        if tab == "code" {
            Tab::Code
        } else {
            Tab::Chat
        }
    }
}

impl Panel {
    fn session(&mut self, tab: Tab) -> &mut Session {
        match tab {
            Tab::Chat => &mut self.chat,
            Tab::Code => &mut self.code,
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

fn setting(app: &AppHandle, key: &str) -> Option<String> {
    crate::utils::get_setting_str(app, key).filter(|value| !value.trim().is_empty())
}

fn setting_on(app: &AppHandle, key: &str) -> bool {
    crate::utils::get_setting_str(app, key).as_deref() != Some("false")
}

// ── Finding Claude Code ────────────────────────────────────────────────────

/// Turns a claude.exe or npm `claude.cmd` path into the exe to run.
fn exe_for(path: PathBuf) -> Option<PathBuf> {
    if !path.is_file() {
        return None;
    }
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    match extension.as_deref() {
        Some("exe") => Some(path),
        // npm installs a .cmd shim; the package ships the real exe beside it.
        Some("cmd") => {
            let exe = path
                .parent()?
                .join("node_modules/@anthropic-ai/claude-code/bin/claude.exe");
            exe.is_file().then_some(exe)
        }
        _ => None,
    }
}

/// Full path to claude.exe, or None when Claude Code isn't installed.
pub fn resolve_claude(app: &AppHandle) -> Option<PathBuf> {
    if let Some(configured) = setting(app, "bloom-claude-path") {
        if let Some(exe) = exe_for(PathBuf::from(configured.trim().trim_matches('"'))) {
            return Some(exe);
        }
    }
    let env = |key: &str| std::env::var_os(key).map(PathBuf::from);
    let mut candidates = Vec::new();
    if let Some(home) = env("USERPROFILE") {
        candidates.push(home.join(".local/bin/claude.exe"));
    }
    if let Some(roaming) = env("APPDATA") {
        candidates.push(roaming.join("npm/node_modules/@anthropic-ai/claude-code/bin/claude.exe"));
    }
    if let Some(local) = env("LOCALAPPDATA") {
        candidates.push(local.join("Programs/claude/claude.exe"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            candidates.push(dir.join("claude.exe"));
            candidates.push(dir.join("claude.cmd"));
        }
    }
    candidates.into_iter().find_map(exe_for)
}

fn base_command(app: &AppHandle, exe: PathBuf) -> Command {
    let mut command = Command::new(exe);
    command.args([
        "-p",
        "--output-format",
        "stream-json",
        "--input-format",
        "stream-json",
        "--verbose",
        "--permission-prompt-tool=stdio",
    ]);
    if let Some(model) = setting(app, "bloom-claude-model") {
        command.args(["--model", model.trim()]);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

fn work_dir(app: &AppHandle, name: &str) -> PathBuf {
    let dir = app
        .path()
        .app_local_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join(name);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

// ── State and events ───────────────────────────────────────────────────────

fn view(session: &Session) -> SessionView {
    SessionView {
        items: session.items.clone(),
        status: session.status,
        approval: session.approval.clone(),
        folder: session.folder.clone(),
    }
}

fn load_folder(app: &AppHandle, panel: &mut Panel) {
    if !panel.folder_loaded {
        panel.folder_loaded = true;
        panel.code.folder = setting(app, "bloom-claude-folder");
    }
}

fn snapshot(app: &AppHandle) -> PanelState {
    let installed = resolve_claude(app).is_some();
    let mut panel = PANEL.lock().unwrap_or_else(|e| e.into_inner());
    load_folder(app, &mut panel);
    PanelState {
        chat: view(&panel.chat),
        code: view(&panel.code),
        installed,
    }
}

fn emit_state(app: &AppHandle) {
    let state = snapshot(app);
    let _ = app.emit(STATE_EVENT, state);
}

fn panel_visible(app: &AppHandle) -> bool {
    app.get_webview_window(WINDOW_LABEL)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// Shows the Code conversation's progress in the Island, like other Claude Code
/// sessions, when the panel isn't open. Respects Bloom's agent alert setting.
fn island_alert(app: &AppHandle, folder: Option<&str>, kind: &str) {
    if !setting_on(app, "bloom-agent-alerts-enabled") || panel_visible(app) {
        return;
    }
    if crate::agent_alerts::agent_alerts_quiet() && kind != "attention" {
        return;
    }
    let project = folder
        .and_then(|f| {
            PathBuf::from(f)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    let alert = crate::agent_alerts::AgentAlert {
        agent: "claude".into(),
        session: PANEL_SESSION.into(),
        project,
        kind: kind.into(),
        activity: None,
        at: None,
    };
    let _ = app.emit_to("main", "agent-alert", alert);
}

fn write_line(session: &mut Session, line: &str) -> bool {
    let Some(stdin) = session.stdin.as_mut() else {
        return false;
    };
    stdin
        .write_all(line.as_bytes())
        .and_then(|_| stdin.write_all(b"\n"))
        .and_then(|_| stdin.flush())
        .is_ok()
}

fn stop_process(session: &mut Session) {
    session.stdin = None;
    if let Some(mut child) = session.child.take() {
        let _ = child.kill();
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
    // Lines from the old process must not reach the new conversation.
    session.generation += 1;
}

// ── Running Claude Code ────────────────────────────────────────────────────

fn ensure_process(app: &AppHandle, tab: Tab, session: &mut Session) -> Result<(), String> {
    if let Some(child) = session.child.as_mut() {
        if matches!(child.try_wait(), Ok(None)) {
            return Ok(());
        }
    }
    stop_process(session);

    let exe = resolve_claude(app).ok_or_else(|| "not_installed".to_string())?;
    let mut command = base_command(app, exe);
    match tab {
        Tab::Chat => {
            command.args([
                "--tools",
                "WebSearch,WebFetch",
                "--append-system-prompt",
                CHAT_PROMPT,
            ]);
            command.current_dir(work_dir(app, "claude-chat"));
        }
        Tab::Code => {
            let folder = session
                .folder
                .clone()
                .ok_or_else(|| "no_folder".to_string())?;
            command.current_dir(folder);
        }
    }
    if let Some(id) = &session.session_id {
        command.args(["--resume", id]);
    }

    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let stderr = child.stderr.take().ok_or("no stderr")?;
    session.stdin = child.stdin.take();
    session.child = Some(child);
    session.generation += 1;
    let generation = session.generation;

    let tail = Arc::clone(&session.stderr);
    if let Ok(mut lines) = tail.lock() {
        lines.clear();
    }
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Ok(mut lines) = tail.lock() {
                lines.push_back(line);
                while lines.len() > 20 {
                    lines.pop_front();
                }
            }
        }
    });

    let reader_app = app.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            handle_line(&reader_app, tab, generation, &line);
        }
        on_exit(&reader_app, tab, generation);
    });

    write_line(session, &protocol::initialize_request("init"));
    Ok(())
}

fn handle_line(app: &AppHandle, tab: Tab, generation: u64, line: &str) {
    let events = protocol::parse_line(line);
    if events.is_empty() {
        return;
    }
    let mut alert: Option<(Option<String>, &str)> = None;
    {
        let mut panel = PANEL.lock().unwrap_or_else(|e| e.into_inner());
        let session = panel.session(tab);
        if session.generation != generation {
            return;
        }
        for event in events {
            match event {
                Event::SessionId(id) => session.session_id = Some(id),
                Event::Text(text) => session.items.push(Item::Assistant { text }),
                Event::ToolUse(call) => session.items.push(Item::Tool { call }),
                Event::TurnDone { error, session_id } => {
                    if session_id.is_some() {
                        session.session_id = session_id;
                    }
                    match error {
                        Some(TurnError::Message(text)) => session.items.push(Item::Error {
                            text: Some(text),
                            code: None,
                        }),
                        Some(TurnError::MaxTurns) => session.items.push(Item::Error {
                            text: None,
                            code: Some("max_turns".into()),
                        }),
                        Some(TurnError::Generic) => session.items.push(Item::Error {
                            text: None,
                            code: Some("failed".into()),
                        }),
                        None => {}
                    }
                    session.approval = None;
                    session.status = Status::Idle;
                    if tab == Tab::Code {
                        alert = Some((session.folder.clone(), "ready"));
                    }
                }
                Event::Permission {
                    request_id,
                    tool,
                    input,
                    title: _,
                } => {
                    if session.always_allow.contains(&tool) {
                        write_line(
                            session,
                            &protocol::permission_response(&request_id, true, &input),
                        );
                    } else {
                        let call = protocol::describe_tool(&tool, &input);
                        session.approval = Some(Approval {
                            request_id,
                            call,
                            input,
                        });
                        session.status = Status::NeedsApproval;
                        if tab == Tab::Code {
                            alert = Some((session.folder.clone(), "attention"));
                        }
                    }
                }
                Event::UnsupportedRequest { request_id } => {
                    write_line(session, &protocol::unsupported_response(&request_id));
                }
                Event::Cancelled { request_id } => {
                    if session
                        .approval
                        .as_ref()
                        .is_some_and(|a| a.request_id == request_id)
                    {
                        session.approval = None;
                        session.status = Status::Working;
                    }
                }
            }
        }
    }
    if let Some((folder, kind)) = alert {
        island_alert(app, folder.as_deref(), kind);
    }
    emit_state(app);
}

fn on_exit(app: &AppHandle, tab: Tab, generation: u64) {
    {
        let mut panel = PANEL.lock().unwrap_or_else(|e| e.into_inner());
        let session = panel.session(tab);
        if session.generation != generation {
            return;
        }
        session.stdin = None;
        session.child = None;
        if matches!(session.status, Status::Working | Status::NeedsApproval) {
            let tail: Vec<String> = session
                .stderr
                .lock()
                .map(|lines| {
                    lines
                        .iter()
                        .filter(|l| !l.trim().is_empty())
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            let text = tail[tail.len().saturating_sub(4)..].join("\n");
            session.items.push(Item::Error {
                text: (!text.is_empty()).then_some(text),
                code: Some("stopped".into()),
            });
            session.approval = None;
            session.status = Status::Error;
        }
    }
    emit_state(app);
}

// ── Commands ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn claude_state(app: AppHandle) -> PanelState {
    snapshot(&app)
}

#[tauri::command]
pub fn claude_cli_path(app: AppHandle) -> Option<String> {
    resolve_claude(&app).map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn claude_send(app: AppHandle, tab: String, text: String) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Ok(());
    }
    let tab = Tab::parse(&tab);
    let result = {
        let mut panel = PANEL.lock().unwrap_or_else(|e| e.into_inner());
        load_folder(&app, &mut panel);
        let session = panel.session(tab);
        if session.status == Status::NeedsApproval {
            return Err("needs_approval".into());
        }
        session.items.push(Item::User { text: text.clone() });
        match ensure_process(&app, tab, session) {
            Ok(()) => {
                if write_line(session, &protocol::user_message(&text)) {
                    session.status = Status::Working;
                    Ok(())
                } else {
                    Err("failed".to_string())
                }
            }
            Err(code) => Err(code),
        }
        .inspect_err(|code| {
            let known = ["not_installed", "no_folder", "failed"].contains(&code.as_str());
            session.items.push(Item::Error {
                text: (!known).then(|| code.clone()),
                code: Some(if known { code.clone() } else { "failed".into() }),
            });
            session.status = Status::Error;
        })
    };
    emit_state(&app);
    result
}

#[tauri::command]
pub fn claude_answer(app: AppHandle, tab: String, allow: bool, always: bool) {
    {
        let mut panel = PANEL.lock().unwrap_or_else(|e| e.into_inner());
        let session = panel.session(Tab::parse(&tab));
        let Some(approval) = session.approval.take() else {
            return;
        };
        if allow && always {
            session.always_allow.insert(approval.call.tool.clone());
        }
        write_line(
            session,
            &protocol::permission_response(&approval.request_id, allow, &approval.input),
        );
        session.items.push(Item::Decision {
            allowed: allow,
            call: approval.call,
        });
        session.status = Status::Working;
    }
    emit_state(&app);
}

#[tauri::command]
pub fn claude_interrupt(tab: String) {
    let mut panel = PANEL.lock().unwrap_or_else(|e| e.into_inner());
    let session = panel.session(Tab::parse(&tab));
    write_line(
        session,
        &protocol::interrupt_request(&format!("interrupt-{}", now_ms())),
    );
}

/// Ends a conversation; the next message starts a fresh one.
#[tauri::command]
pub fn claude_new(app: AppHandle, tab: String) {
    {
        let mut panel = PANEL.lock().unwrap_or_else(|e| e.into_inner());
        let session = panel.session(Tab::parse(&tab));
        stop_process(session);
        session.items.clear();
        session.approval = None;
        session.session_id = None;
        session.always_allow.clear();
        session.status = Status::Idle;
    }
    emit_state(&app);
}

/// Switches the Code conversation to another project folder (a fresh conversation).
/// The UI saves the folder to settings so it's remembered.
#[tauri::command]
pub fn claude_set_folder(app: AppHandle, folder: String) {
    {
        let mut panel = PANEL.lock().unwrap_or_else(|e| e.into_inner());
        panel.folder_loaded = true;
        let session = &mut panel.code;
        stop_process(session);
        session.items.clear();
        session.approval = None;
        session.session_id = None;
        session.always_allow.clear();
        session.status = Status::Idle;
        session.folder = Some(folder);
    }
    emit_state(&app);
}

// ── Usage ──────────────────────────────────────────────────────────────────

/// Asks Claude Code for the plan's usage (what /usage shows). This sends no
/// message to Claude, so it doesn't count against the limits.
fn fetch_usage(app: &AppHandle) -> UsageInfo {
    let failed = |code: &str| UsageInfo {
        error: Some(code.into()),
        checked_at: now_ms(),
        ..Default::default()
    };
    let Some(exe) = resolve_claude(app) else {
        return failed("not_installed");
    };
    let mut command = base_command(app, exe);
    command
        .args(["--tools", "", "--no-session-persistence"])
        .current_dir(work_dir(app, "claude-usage"));
    let Ok(mut child) = command.spawn() else {
        return failed("failed");
    };
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.kill();
        return failed("failed");
    };
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(
            move || {
                for _ in BufReader::new(stderr).lines().map_while(Result::ok) {}
            },
        );
    }

    let request_id = "usage";
    let written = writeln!(stdin, "{}", protocol::initialize_request("init"))
        .and_then(|_| writeln!(stdin, "{}", protocol::usage_request(request_id)))
        .and_then(|_| stdin.flush());

    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(usage) = protocol::parse_usage_line(&line, request_id) {
                let _ = sender.send(usage);
                return;
            }
        }
    });

    let result = match written {
        Ok(()) => receiver.recv_timeout(USAGE_TIMEOUT).ok(),
        Err(_) => None,
    };
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();

    match result {
        Some(mut usage) => {
            usage.checked_at = now_ms();
            usage
        }
        None => failed("no_answer"),
    }
}

fn refresh_usage(app: &AppHandle) -> UsageInfo {
    let usage = fetch_usage(app);
    if let Ok(mut cached) = USAGE.lock() {
        *cached = Some(usage.clone());
    }
    let _ = app.emit(USAGE_EVENT, usage.clone());
    usage
}

/// The last usage check, or a fresh one when `refresh` is set or none exists yet.
#[tauri::command]
pub async fn claude_usage(app: AppHandle, refresh: bool) -> Option<UsageInfo> {
    if !refresh {
        if let Some(cached) = USAGE.lock().ok().and_then(|c| c.clone()) {
            return Some(cached);
        }
    }
    tauri::async_runtime::spawn_blocking(move || refresh_usage(&app))
        .await
        .ok()
}

// ── Panel window ───────────────────────────────────────────────────────────

fn panel_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    let _guard = WINDOW_LOCK.lock().map_err(|e| e.to_string())?;
    if crate::state::SHUTTING_DOWN.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Bloom is shutting down".into());
    }
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        return Ok(window);
    }
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == WINDOW_LABEL)
        .ok_or_else(|| "Missing Claude window template".to_string())?;
    let window = WebviewWindowBuilder::from_config(app, config)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    crate::services::watch_webview_process(app, &window);
    Ok(window)
}

/// Places the panel centered under the Island, on the main monitor.
fn place_window(window: &tauri::WebviewWindow) {
    let Ok(Some(monitor)) = window.primary_monitor() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let scale = monitor.scale_factor();
    let area = monitor.size();
    let origin = monitor.position();
    let x = origin.x + (area.width as i32 - size.width as i32) / 2;
    let y = origin.y + (64.0 * scale) as i32;
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

fn show_panel(app: &AppHandle, tab: Option<String>) {
    match panel_window(app) {
        Ok(window) => {
            place_window(&window);
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
            if let Some(tab) = tab {
                let _ = app.emit_to(WINDOW_LABEL, "claude-open-tab", tab);
            }
        }
        Err(error) => eprintln!("Failed to open the Claude panel: {error}"),
    }
}

/// Opens the panel, on "chat" or "code" when given.
#[tauri::command]
pub fn open_claude_window(app: AppHandle, tab: Option<String>) {
    // WebView2 creation must run off the Windows UI thread.
    tauri::async_runtime::spawn_blocking(move || show_panel(&app, tab));
}

#[tauri::command]
pub fn hide_claude_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.hide();
    }
}

fn toggle_panel(app: &AppHandle) {
    let open = app
        .get_webview_window(WINDOW_LABEL)
        .is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
    if open {
        hide_claude_window(app.clone());
    } else {
        show_panel(app, None);
    }
}

// ── Startup: hotkey and usage checks ───────────────────────────────────────

/// Ctrl+Alt+C opens or closes the panel. Registered once at startup when
/// `bloom-claude-hotkey-enabled` isn't "false".
fn run_hotkey(app: AppHandle) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};
    const HOTKEY_ID: i32 = 0xC1;
    const VK_C: u32 = 0x43;
    unsafe {
        let modifiers = HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_ALT.0 | MOD_NOREPEAT.0);
        if RegisterHotKey(None, HOTKEY_ID, modifiers, VK_C).is_err() {
            eprintln!("Claude panel: Ctrl+Alt+C is taken by another app");
            return;
        }
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            if message.message == WM_HOTKEY && message.wParam.0 == HOTKEY_ID as usize {
                let app = app.clone();
                tauri::async_runtime::spawn_blocking(move || toggle_panel(&app));
            }
        }
    }
}

pub fn setup(app: &AppHandle) {
    if setting_on(app, "bloom-claude-hotkey-enabled") {
        let hotkey_app = app.clone();
        std::thread::spawn(move || run_hotkey(hotkey_app));
    }

    let usage_app = app.clone();
    std::thread::spawn(move || {
        // Let startup finish before the first check.
        std::thread::sleep(Duration::from_secs(20));
        while !crate::state::SHUTTING_DOWN.load(std::sync::atomic::Ordering::Relaxed) {
            if setting_on(&usage_app, "bloom-claude-usage-enabled")
                && resolve_claude(&usage_app).is_some()
            {
                refresh_usage(&usage_app);
            }
            std::thread::sleep(USAGE_INTERVAL);
        }
    });
}

/// Ends both conversations when Bloom exits.
pub fn shutdown() {
    let mut panel = PANEL.lock().unwrap_or_else(|e| e.into_inner());
    stop_process(&mut panel.chat);
    stop_process(&mut panel.code);
}
