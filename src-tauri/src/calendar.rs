//! Downloads calendars from secret iCal (.ics) addresses, such as Google
//! Calendar's "Secret address in iCal format" or Outlook's published ICS link.
//! Read-only: the island parses the files (ical.js) and shows today's events.
//! Addresses are kept in `bloom-calendar-urls`, one per line.
use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct CalendarFeed {
    /// The ICS text, when the download worked.
    pub text: Option<String>,
    /// Why it didn't: "invalid_url" or "download_failed".
    pub error: Option<String>,
}

/// Google and Outlook hand out webcal:// links; they're plain https underneath.
fn normalize_url(raw: &str) -> Option<String> {
    let url = raw.trim();
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("webcal://") {
        return Some(format!("https://{}", &url["webcal://".len()..]));
    }
    (lower.starts_with("https://") || lower.starts_with("http://")).then(|| url.to_string())
}

/// The addresses saved in settings, one per line.
pub fn configured_urls(raw: &str) -> Vec<String> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

/// Downloads with the Windows URL moniker, so system proxies and certificates
/// apply. The cached copy is dropped first so every refresh is current.
fn download(url: &str) -> CalendarFeed {
    use windows::core::{IUnknown, HSTRING, PCWSTR};
    use windows::Win32::Networking::WinInet::DeleteUrlCacheEntryW;
    use windows::Win32::System::Com::{IBindStatusCallback, Urlmon::URLDownloadToFileW};

    let Some(url) = normalize_url(url) else {
        return CalendarFeed {
            text: None,
            error: Some("invalid_url".into()),
        };
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&url, &mut hasher);
    let file = std::env::temp_dir().join(format!(
        "bloom-calendar-{:x}.ics",
        std::hash::Hasher::finish(&hasher)
    ));

    let wide_url = HSTRING::from(url.as_str());
    let wide_file = HSTRING::from(file.as_os_str());
    let downloaded = unsafe {
        let _com = crate::battery::ComGuard::new();
        let _ = DeleteUrlCacheEntryW(PCWSTR(wide_url.as_ptr()));
        URLDownloadToFileW(
            None::<&IUnknown>,
            PCWSTR(wide_url.as_ptr()),
            PCWSTR(wide_file.as_ptr()),
            0,
            None::<&IBindStatusCallback>,
        )
    };
    let result = match downloaded.ok().and_then(|_| std::fs::read(&file).ok()) {
        Some(bytes) => CalendarFeed {
            text: Some(decode(&bytes)),
            error: None,
        },
        None => CalendarFeed {
            text: None,
            error: Some("download_failed".into()),
        },
    };
    let _ = std::fs::remove_file(&file);
    result
}

/// Downloads every configured calendar. Empty when none are set.
#[tauri::command]
pub async fn calendar_fetch(app: tauri::AppHandle) -> Vec<CalendarFeed> {
    let raw = crate::utils::get_setting_str(&app, "bloom-calendar-urls").unwrap_or_default();
    let urls = configured_urls(&raw);
    tauri::async_runtime::spawn_blocking(move || urls.iter().map(|u| download(u)).collect())
        .await
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webcal_becomes_https() {
        assert_eq!(
            normalize_url(" webcal://calendar.google.com/x/basic.ics "),
            Some("https://calendar.google.com/x/basic.ics".into())
        );
        assert_eq!(
            normalize_url("https://outlook.office365.com/a.ics"),
            Some("https://outlook.office365.com/a.ics".into())
        );
        assert_eq!(normalize_url("file:///c:/x.ics"), None);
        assert_eq!(normalize_url("not a url"), None);
    }

    #[test]
    fn urls_are_read_one_per_line() {
        assert_eq!(
            configured_urls("https://a\n\n  https://b  \r\n"),
            vec!["https://a".to_string(), "https://b".to_string()]
        );
    }

    #[test]
    fn byte_order_mark_is_dropped() {
        assert_eq!(decode(b"\xEF\xBB\xBFBEGIN:VCALENDAR"), "BEGIN:VCALENDAR");
    }
}
