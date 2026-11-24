//! The Claude panel's claude.ai view: the user's own claude.ai, in its phone
//! layout, in a window of its own.
//!
//! The page gets no access to Bloom: no capability lists this window, so it
//! cannot call Bloom's commands. Its one link back is a small switch button the
//! page script adds, which navigates to a reserved address Bloom intercepts.
//! Links to other sites open in the default browser.

use tauri::{
    webview::NewWindowResponse, AppHandle, LogicalSize, Manager, Url, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};

pub const LABEL: &str = "claude-web";
const HOME: &str = "https://claude.ai/new";
/// Navigating here (from the switch button) shows the Code view instead.
const SWITCH_PATH: &str = "/__bloom/code";
/// The size the panel asks for; it shrinks to fit above the dock.
pub const SIZE: LogicalSize<f64> = LogicalSize {
    width: 440.0,
    height: 720.0,
};

/// What to do with a page navigation.
#[derive(Debug, PartialEq, Eq)]
pub enum Navigation {
    /// claude.ai, or a sign-in page it relies on: load it here.
    Stay,
    /// The switch button: show the Code view.
    SwitchToCode,
    /// Any other site: open it in the default browser.
    Browser,
}

/// Sites the panel loads itself: claude.ai and the pages its sign-in and
/// checkout go through. Everything else opens in the browser.
fn stays_in_panel(host: &str) -> bool {
    const SITES: &[&str] = &[
        "claude.ai",
        "claude.com",
        "anthropic.com",
        "accounts.google.com",
        "appleid.apple.com",
        "challenges.cloudflare.com",
        "js.stripe.com",
        "checkout.stripe.com",
    ];
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    SITES
        .iter()
        .any(|site| host == *site || host.ends_with(&format!(".{site}")))
}

pub fn classify(url: &Url) -> Navigation {
    match url.scheme() {
        "http" | "https" => {}
        // about:blank, data: and blob: pages belong to the current page.
        "about" | "data" | "blob" => return Navigation::Stay,
        _ => return Navigation::Browser,
    }
    let host = url.host_str().unwrap_or_default();
    if host.eq_ignore_ascii_case("claude.ai") && url.path() == SWITCH_PATH {
        return Navigation::SwitchToCode;
    }
    if stays_in_panel(host) {
        Navigation::Stay
    } else {
        Navigation::Browser
    }
}

fn open_in_browser(url: &Url) {
    let url = url.to_string();
    std::thread::spawn(move || {
        let _com = crate::battery::ComGuard::new();
        crate::commands::launch_path(&url);
    });
}

/// Adds the switch to Code in the top-right corner of claude.ai, under the
/// header. Only on claude.ai itself, not on sign-in pages.
const SWITCH_SCRIPT: &str = r#"
(() => {
  if (location.hostname !== "claude.ai" || window.top !== window) return;
  const add = () => {
    if (!document.body || document.getElementById("bloom-code-switch")) return;
    const button = document.createElement("button");
    button.id = "bloom-code-switch";
    button.type = "button";
    button.title = "Bloom: switch to Code";
    button.setAttribute("aria-label", "Switch to Code");
    button.innerHTML = '<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>';
    Object.assign(button.style, {
      position: "fixed", top: "60px", right: "10px", zIndex: "2147483647",
      width: "30px", height: "30px", borderRadius: "50%", border: "none",
      display: "grid", placeItems: "center", cursor: "pointer", padding: "0",
      background: "rgba(127,127,127,0.18)", color: "inherit", opacity: "0.6",
      backdropFilter: "blur(6px)", transition: "opacity 0.15s"
    });
    button.addEventListener("mouseenter", () => (button.style.opacity = "1"));
    button.addEventListener("mouseleave", () => (button.style.opacity = "0.6"));
    button.addEventListener("click", () => location.assign("https://claude.ai/__bloom/code"));
    document.body.appendChild(button);
  };
  // claude.ai redraws the page as you move around it; put the switch back.
  document.addEventListener("DOMContentLoaded", add);
  setInterval(add, 1500);
})();
"#;

/// Whether the panel should close because focus moved away: not when focus
/// went to a dialog or sign-in popup of Bloom's own (a file picker for an
/// attachment, say), only when it went to another app or another Bloom window.
fn focus_left_panel(app: &AppHandle, panel: isize) -> bool {
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    let foreground = unsafe { GetForegroundWindow() };
    let handle = foreground.0 as isize;
    if handle == 0 {
        return true;
    }
    // The panel itself, or its other view (switching to Code).
    if handle == panel || crate::claude::panel_hwnds(app).contains(&handle) {
        return false;
    }
    let mut process = 0u32;
    unsafe { GetWindowThreadProcessId(foreground, Some(&mut process)) };
    if process != unsafe { GetCurrentProcessId() } {
        return true;
    }
    // One of Bloom's own windows (the dock, the Island): close. Anything else
    // in Bloom's process is a dialog or popup the page opened: stay.
    app.webview_windows()
        .values()
        .filter_map(|window| window.hwnd().ok())
        .any(|hwnd| hwnd.0 as isize == handle)
}

/// Rounds the borderless window's corners on Windows 11, like Bloom's other panels.
fn round_corners(window: &tauri::WebviewWindow) {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    if let Ok(hwnd) = window.hwnd() {
        let preference = DWMWCP_ROUND;
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &preference as *const _ as *const _,
                std::mem::size_of_val(&preference) as u32,
            );
        }
    }
}

pub fn build(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    let home = HOME.parse::<Url>().map_err(|e| e.to_string())?;
    let nav_app = app.clone();
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(home))
        .title("Claude")
        .inner_size(SIZE.width, SIZE.height)
        .decorations(false)
        .shadow(true)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .focused(true)
        .initialization_script(SWITCH_SCRIPT)
        .on_navigation(move |url| match classify(url) {
            Navigation::Stay => true,
            Navigation::SwitchToCode => {
                let app = nav_app.clone();
                std::thread::spawn(move || {
                    crate::claude::show_view(&app, crate::claude::View::Code)
                });
                false
            }
            Navigation::Browser => {
                open_in_browser(url);
                false
            }
        })
        .on_new_window(|url, _features| match classify(&url) {
            // Sign-in popups (Google, Apple) open as Bloom windows.
            Navigation::Stay => NewWindowResponse::Allow,
            Navigation::SwitchToCode | Navigation::Browser => {
                open_in_browser(&url);
                NewWindowResponse::Deny
            }
        })
        .build()
        .map_err(|e| e.to_string())?;
    round_corners(&window);
    crate::services::watch_webview_process(app, &window);

    let blur_app = app.clone();
    let panel = window.hwnd().map(|hwnd| hwnd.0 as isize).unwrap_or(0);
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            let app = blur_app.clone();
            std::thread::spawn(move || {
                // Let the new foreground window settle first.
                std::thread::sleep(std::time::Duration::from_millis(150));
                let still_away = app
                    .get_webview_window(LABEL)
                    .is_some_and(|window| !window.is_focused().unwrap_or(false));
                if still_away && focus_left_panel(&app, panel) {
                    crate::claude::hide_claude_window(app, Some(true));
                }
            });
        }
    });
    Ok(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nav(url: &str) -> Navigation {
        classify(&url.parse().unwrap())
    }

    #[test]
    fn keeps_claude_and_sign_in_pages() {
        assert_eq!(nav("https://claude.ai/chat/123"), Navigation::Stay);
        assert_eq!(nav("https://www.anthropic.com/legal"), Navigation::Stay);
        assert_eq!(
            nav("https://accounts.google.com/o/oauth2"),
            Navigation::Stay
        );
        assert_eq!(nav("about:blank"), Navigation::Stay);
    }

    #[test]
    fn sends_other_sites_to_the_browser() {
        assert_eq!(
            nav("https://en.wikipedia.org/wiki/OCPP"),
            Navigation::Browser
        );
        assert_eq!(nav("https://claude.ai.evil.example/"), Navigation::Browser);
        assert_eq!(nav("https://notclaude.ai/"), Navigation::Browser);
        assert_eq!(nav("mailto:someone@example.com"), Navigation::Browser);
    }

    #[test]
    fn recognises_the_switch() {
        assert_eq!(
            nav("https://claude.ai/__bloom/code"),
            Navigation::SwitchToCode
        );
        assert_eq!(nav("https://claude.ai/__bloom/codex"), Navigation::Stay);
    }
}
