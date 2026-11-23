//! Windows notifications on the Island: a popup for each new one, and the
//! drawer listing what is waiting in the notification center.
//!
//! The list comes from the notification center itself (see `notification_store`),
//! so a chat app withdrawing its notifications once a chat is read also clears
//! them from the drawer. Dismissing one in Bloom only hides it in Bloom: Windows
//! offers no way to remove another app's notification without package identity.

use crate::notification_store::{read_toasts, StoredToast};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{LazyLock, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

const POLL: Duration = Duration::from_millis(1500);
/// Only notifications this recent pop up; older ones found later (after the
/// computer wakes, say) go straight to the drawer.
const POPUP_WINDOW_MS: u64 = 120_000;
const ENABLED_KEY: &str = "bloom-notch-notifications-enabled";

/// A notification as the Island shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Notice {
    pub id: i64,
    pub app_id: String,
    /// The sending app's name ("WhatsApp").
    pub app: String,
    pub title: String,
    pub body: String,
    /// Arrival time, Unix milliseconds.
    pub at: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NoticeList {
    pub items: Vec<Notice>,
    /// Why Windows' notifications can't be read, when they can't.
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoticeApp {
    pub id: String,
    pub name: String,
}

/// What Bloom keeps between runs: which notifications were dismissed here,
/// and the apps seen, so Settings can list them.
#[derive(Default, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    hidden: Vec<i64>,
    #[serde(default)]
    apps: Vec<NoticeApp>,
}

#[derive(Default)]
struct Inner {
    items: Vec<Notice>,
    /// Every notification id seen in the last read, hidden or not.
    known: HashSet<i64>,
    hidden: HashSet<i64>,
    apps: HashMap<String, String>,
    error: Option<String>,
    /// Set after the first read, so what was already there doesn't pop up.
    primed: bool,
    loaded: bool,
}

static STATE: LazyLock<Mutex<Inner>> = LazyLock::new(|| Mutex::new(Inner::default()));
static ICONS: LazyLock<Mutex<HashMap<String, Option<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn enabled(app: &AppHandle) -> bool {
    crate::utils::get_setting_str(app, ENABLED_KEY).as_deref() != Some("false")
}

fn database_path() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(local)
            .join("Microsoft")
            .join("Windows")
            .join("Notifications")
            .join("wpndatabase.db"),
    )
}

fn saved_path(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_config_dir().ok()?.join("notifications.json"))
}

fn load(app: &AppHandle, inner: &mut Inner) {
    if inner.loaded {
        return;
    }
    inner.loaded = true;
    let Some(saved) = saved_path(app)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<Saved>(&text).ok())
    else {
        return;
    };
    inner.hidden = saved.hidden.into_iter().collect();
    inner.apps = saved
        .apps
        .into_iter()
        .map(|app| (app.id, app.name))
        .collect();
}

fn save(app: &AppHandle, inner: &Inner) {
    let Some(path) = saved_path(app) else {
        return;
    };
    let saved = Saved {
        hidden: inner.hidden.iter().copied().collect(),
        apps: app_list(inner),
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string(&saved) {
        let _ = std::fs::write(path, text);
    }
}

fn app_list(inner: &Inner) -> Vec<NoticeApp> {
    let mut apps: Vec<NoticeApp> = inner
        .apps
        .iter()
        .map(|(id, name)| NoticeApp {
            id: id.clone(),
            name: name.clone(),
        })
        .collect();
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

/// A readable name for an app id Windows can't name: the middle of
/// `Publisher.AppName_hash!App`, or the id itself.
fn fallback_name(app_id: &str) -> String {
    let base = app_id.split('!').next().unwrap_or(app_id);
    let base = base.split('_').next().unwrap_or(base);
    let name = base.rsplit('.').next().unwrap_or(base);
    let name = name.strip_suffix("Desktop").unwrap_or(name);
    if name.is_empty() {
        app_id.to_string()
    } else {
        name.to_string()
    }
}

fn app_name(inner: &mut Inner, app_id: &str) -> (String, bool) {
    if let Some(name) = inner.apps.get(app_id) {
        return (name.clone(), false);
    }
    let name = crate::commands::shell_app_name(app_id).unwrap_or_else(|| fallback_name(app_id));
    inner.apps.insert(app_id.to_string(), name.clone());
    (name, true)
}

fn open_database() -> Result<Connection, String> {
    let path = database_path().ok_or("LOCALAPPDATA is not set")?;
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    let _ = conn.busy_timeout(Duration::from_millis(300));
    Ok(conn)
}

/// When the live database can't be opened, a copy of it (and its write-ahead
/// log, which holds the newest notifications) usually can.
fn read_copy() -> Result<Vec<StoredToast>, String> {
    let path = database_path().ok_or("LOCALAPPDATA is not set")?;
    let dir = std::env::temp_dir().join("bloom-notifications");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let copy = dir.join("wpndatabase.db");
    std::fs::copy(&path, &copy).map_err(|e| e.to_string())?;
    for suffix in ["-wal", "-shm"] {
        let mut source = path.clone().into_os_string();
        source.push(suffix);
        let mut target = copy.clone().into_os_string();
        target.push(suffix);
        if std::fs::copy(&source, &target).is_err() {
            let _ = std::fs::remove_file(&target);
        }
    }
    let conn = Connection::open(copy).map_err(|e| e.to_string())?;
    read_toasts(&conn, now_ms()).map_err(|e| e.to_string())
}

fn read(conn: &mut Option<Connection>) -> Result<Vec<StoredToast>, String> {
    if conn.is_none() {
        match open_database() {
            Ok(opened) => *conn = Some(opened),
            Err(open_error) => return read_copy().map_err(|_| open_error),
        }
    }
    let result = conn
        .as_ref()
        .map(|c| read_toasts(c, now_ms()))
        .unwrap_or_else(|| Ok(Vec::new()));
    match result {
        Ok(toasts) => Ok(toasts),
        Err(error) => {
            *conn = None;
            read_copy().map_err(|_| error.to_string())
        }
    }
}

fn emit_list(app: &AppHandle, inner: &Inner) {
    let _ = app.emit(
        "notifications-changed",
        NoticeList {
            items: inner.items.clone(),
            error: inner.error.clone(),
        },
    );
}

fn scan(app: &AppHandle, conn: &mut Option<Connection>) {
    let result = read(conn);
    let Ok(mut inner) = STATE.lock() else {
        return;
    };
    load(app, &mut inner);
    let toasts = match result {
        Ok(toasts) => toasts,
        Err(error) => {
            if inner.error.as_deref() != Some(error.as_str()) {
                inner.error = Some(error);
                emit_list(app, &inner);
            }
            return;
        }
    };
    let had_error = inner.error.take().is_some();
    let present: HashSet<i64> = toasts.iter().map(|toast| toast.id).collect();
    let mut changed_saved = false;

    let hidden_before = inner.hidden.len();
    inner.hidden.retain(|id| present.contains(id));
    changed_saved |= inner.hidden.len() != hidden_before;

    let now = now_ms();
    let mut items = Vec::new();
    let mut fresh = Vec::new();
    for toast in toasts {
        if inner.hidden.contains(&toast.id) {
            continue;
        }
        let (name, new_app) = app_name(&mut inner, &toast.app_id);
        changed_saved |= new_app;
        let notice = Notice {
            id: toast.id,
            app_id: toast.app_id,
            app: name,
            title: toast.title,
            body: toast.body,
            at: toast.at,
        };
        if inner.primed && !inner.known.contains(&notice.id) && notice.at + POPUP_WINDOW_MS >= now {
            fresh.push(notice.clone());
        }
        items.push(notice);
    }
    // Newest first, as on a phone.
    items.reverse();
    inner.known = present;
    inner.primed = true;
    let changed = items != inner.items;
    inner.items = items;
    if changed || had_error {
        emit_list(app, &inner);
    }
    if changed_saved {
        save(app, &inner);
    }
    drop(inner);
    for notice in fresh {
        let _ = app.emit("notification-new", notice);
    }
}

fn clear_list(app: &AppHandle) {
    if let Ok(mut inner) = STATE.lock() {
        if inner.items.is_empty() && inner.error.is_none() && !inner.primed {
            return;
        }
        inner.items.clear();
        inner.known.clear();
        inner.error = None;
        inner.primed = false;
        emit_list(app, &inner);
    }
}

pub fn setup(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let _com = crate::battery::ComGuard::new();
        let mut conn: Option<Connection> = None;
        loop {
            if enabled(&app) {
                scan(&app, &mut conn);
            } else {
                conn = None;
                clear_list(&app);
            }
            std::thread::sleep(POLL);
        }
    });
}

#[tauri::command]
pub fn notifications_list() -> NoticeList {
    STATE
        .lock()
        .map(|inner| NoticeList {
            items: inner.items.clone(),
            error: inner.error.clone(),
        })
        .unwrap_or(NoticeList {
            items: Vec::new(),
            error: None,
        })
}

#[tauri::command]
pub fn notifications_apps(app: AppHandle) -> Vec<NoticeApp> {
    STATE
        .lock()
        .map(|mut inner| {
            load(&app, &mut inner);
            app_list(&inner)
        })
        .unwrap_or_default()
}

fn hide(app: &AppHandle, keep: impl Fn(&Notice) -> bool) {
    let Ok(mut inner) = STATE.lock() else {
        return;
    };
    let (kept, gone): (Vec<Notice>, Vec<Notice>) =
        inner.items.drain(..).partition(|notice| keep(notice));
    inner.items = kept;
    if gone.is_empty() {
        return;
    }
    inner.hidden.extend(gone.iter().map(|notice| notice.id));
    emit_list(app, &inner);
    save(app, &inner);
}

#[tauri::command]
pub fn notification_dismiss(app: AppHandle, id: i64) {
    hide(&app, |notice| notice.id != id);
}

/// Clears one app's notifications, or all of them.
#[tauri::command]
pub fn notifications_clear(app: AppHandle, app_id: Option<String>) {
    hide(&app, |notice| {
        app_id.as_deref().is_some_and(|only| notice.app_id != only)
    });
}

/// Opens the app a notification came from, and takes it off the drawer.
#[tauri::command]
pub fn notification_open(app: AppHandle, id: i64) {
    let app_id = STATE.lock().ok().and_then(|inner| {
        inner
            .items
            .iter()
            .find(|notice| notice.id == id)
            .map(|notice| notice.app_id.clone())
    });
    let Some(app_id) = app_id else {
        return;
    };
    hide(&app, |notice| notice.id != id);
    std::thread::spawn(move || {
        let _com = crate::battery::ComGuard::new();
        crate::commands::launch_path(&format!("shell:AppsFolder\\{app_id}"));
    });
}

/// The sending app's icon as a base64 PNG, if Windows has one for it.
#[tauri::command]
pub async fn notification_app_icon(app_id: String) -> Option<String> {
    if let Some(icon) = ICONS
        .lock()
        .ok()
        .and_then(|icons| icons.get(&app_id).cloned())
    {
        return icon;
    }
    let id = app_id.clone();
    let icon = tauri::async_runtime::spawn_blocking(move || {
        let _com = crate::battery::ComGuard::new();
        crate::commands::icon_from_aumid(&id)
    })
    .await
    .ok()
    .flatten();
    if let Ok(mut icons) = ICONS.lock() {
        icons.insert(app_id, icon.clone());
    }
    icon
}

#[cfg(test)]
mod tests {
    use super::fallback_name;

    #[test]
    fn names_unknown_apps_from_their_id() {
        assert_eq!(
            fallback_name("5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App"),
            "WhatsApp"
        );
        assert_eq!(fallback_name("Viber"), "Viber");
    }
}
