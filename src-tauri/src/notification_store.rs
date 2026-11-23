//! Reads the notifications Windows keeps for the notification center.
//!
//! Windows stores every toast an app shows in a SQLite database in the user's
//! profile (`wpndatabase.db`), and removes it again when the user dismisses it,
//! or when the app withdraws it (as chat apps do once a chat is read). Reading
//! that list, read-only, gives Bloom what the notification center holds without
//! the package identity Windows' notification listener API requires.

use rusqlite::{types::ValueRef, Connection};

/// One toast as Windows stored it.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredToast {
    pub id: i64,
    /// The sending app's id (an AppUserModelID, such as WhatsApp's package id).
    pub app_id: String,
    /// When it arrived, in Unix milliseconds.
    pub at: u64,
    pub title: String,
    pub body: String,
}

/// Seconds between 1601 (the FILETIME epoch) and 1970, in milliseconds.
const FILETIME_UNIX_OFFSET_MS: i64 = 11_644_473_600_000;

pub fn filetime_to_unix_ms(filetime: i64) -> u64 {
    (filetime / 10_000 - FILETIME_UNIX_OFFSET_MS).max(0) as u64
}

pub fn unix_ms_to_filetime(ms: u64) -> i64 {
    (ms as i64 + FILETIME_UNIX_OFFSET_MS) * 10_000
}

/// The toasts in the notification center that have not expired, oldest first.
pub fn read_toasts(conn: &Connection, now_ms: u64) -> rusqlite::Result<Vec<StoredToast>> {
    let mut statement = conn.prepare(
        "SELECT n.Id, h.PrimaryId, n.ArrivalTime, n.ExpiryTime, n.Payload
         FROM Notification n JOIN NotificationHandler h ON n.HandlerId = h.RecordId
         WHERE n.Type = 'toast'
         ORDER BY n.ArrivalTime, n.Id",
    )?;
    let now = unix_ms_to_filetime(now_ms);
    let mut rows = statement.query([])?;
    let mut toasts = Vec::new();
    while let Some(row) = rows.next()? {
        let expiry: Option<i64> = row.get(3).ok();
        if expiry.is_some_and(|expiry| expiry > 0 && expiry < now) {
            continue;
        }
        let payload = match row.get_ref(4)? {
            ValueRef::Blob(bytes) => decode(bytes),
            ValueRef::Text(bytes) => String::from_utf8_lossy(bytes).into_owned(),
            _ => continue,
        };
        let Some((title, body)) = toast_text(&payload) else {
            continue;
        };
        toasts.push(StoredToast {
            id: row.get(0)?,
            app_id: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            at: row
                .get::<_, Option<i64>>(2)?
                .map(filetime_to_unix_ms)
                .unwrap_or(now_ms),
            title,
            body,
        });
    }
    Ok(toasts)
}

/// Payloads are UTF-8, but older ones can be UTF-16 with a byte-order mark.
fn decode(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    let text = String::from_utf8_lossy(bytes);
    text.trim_start_matches('\u{feff}').to_string()
}

/// The toast's title and body: its first line of text, then the rest.
/// Attribution lines ("via Chrome") are left out. None for a toast with no text.
pub fn toast_text(xml: &str) -> Option<(String, String)> {
    let doc = roxmltree::Document::parse(xml.trim_end_matches('\0')).ok()?;
    let binding = doc
        .descendants()
        .filter(|node| node.has_tag_name("binding"))
        .find(|node| {
            node.attribute("template")
                .is_some_and(|template| template.eq_ignore_ascii_case("ToastGeneric"))
        })
        .or_else(|| doc.descendants().find(|node| node.has_tag_name("binding")))?;
    let lines: Vec<String> = binding
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .filter(|node| node.attribute("placement") != Some("attribution"))
        // Text inside a group or subgroup belongs to an adaptive layout, which
        // repeats the main lines; the binding's own text lines are enough.
        .filter(|node| node.parent().is_some_and(|parent| parent == binding))
        .map(|node| {
            node.descendants()
                .filter(|child| child.is_text())
                .filter_map(|child| child.text())
                .collect::<String>()
                .trim()
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .collect();
    let (title, rest) = lines.split_first()?;
    Some((title.clone(), rest.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHATSAPP: &str = r#"<toast launch="chat=123"><visual><binding template="ToastGeneric"><text hint-maxLines="1">Maria Santos</text><text>Are you coming to the site visit tomorrow?</text><image placement="appLogoOverride" src="ms-appdata:///local/a.png"/></binding></visual><actions><input id="reply" type="text"/></actions></toast>"#;

    #[test]
    fn reads_title_and_body() {
        assert_eq!(
            toast_text(WHATSAPP),
            Some((
                "Maria Santos".into(),
                "Are you coming to the site visit tomorrow?".into()
            ))
        );
    }

    #[test]
    fn leaves_out_attribution_and_joins_lines() {
        let xml = r#"<toast><visual><binding template="ToastGeneric"><text>Team chat</text><text>Ana: hello</text><text>Ben: hi</text><text placement="attribution">via Chrome</text></binding></visual></toast>"#;
        assert_eq!(
            toast_text(xml),
            Some(("Team chat".into(), "Ana: hello\nBen: hi".into()))
        );
    }

    #[test]
    fn reads_legacy_templates() {
        let xml = r#"<toast><visual><binding template="ToastText02"><text id="1">Viber</text><text id="2">New message</text></binding></visual></toast>"#;
        assert_eq!(
            toast_text(xml),
            Some(("Viber".into(), "New message".into()))
        );
    }

    #[test]
    fn skips_toasts_without_text() {
        let xml = r#"<toast><visual><binding template="ToastGeneric"><image src="a.png"/></binding></visual></toast>"#;
        assert_eq!(toast_text(xml), None);
        assert_eq!(toast_text("not xml"), None);
    }

    #[test]
    fn decodes_utf16_payloads() {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "<toast/>".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(decode(&bytes), "<toast/>");
    }

    #[test]
    fn converts_filetimes() {
        let ms = 1_763_884_800_000;
        assert_eq!(filetime_to_unix_ms(unix_ms_to_filetime(ms)), ms);
    }

    #[test]
    fn reads_the_notification_tables() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"CREATE TABLE NotificationHandler (RecordId INTEGER PRIMARY KEY, PrimaryId TEXT);
               CREATE TABLE Notification ("Order" INTEGER, Id INTEGER, HandlerId INTEGER, Type TEXT,
                   Payload BLOB, "Group" TEXT, Tag TEXT, ExpiryTime INTEGER, ArrivalTime INTEGER);
               INSERT INTO NotificationHandler VALUES (1, '5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App');"#,
        )
        .unwrap();
        let now = 1_763_884_800_000;
        let insert = |id: i64, kind: &str, expiry: u64, arrival: u64| {
            conn.execute(
                "INSERT INTO Notification (Id, HandlerId, Type, Payload, ExpiryTime, ArrivalTime)
                 VALUES (?1, 1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    id,
                    kind,
                    WHATSAPP.as_bytes(),
                    unix_ms_to_filetime(expiry),
                    unix_ms_to_filetime(arrival)
                ],
            )
            .unwrap();
        };
        insert(7, "toast", now + 60_000, now - 5_000);
        insert(8, "badge", now + 60_000, now - 4_000);
        insert(9, "toast", now - 1_000, now - 3_000);
        let toasts = read_toasts(&conn, now).unwrap();
        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0].id, 7);
        assert_eq!(
            toasts[0].app_id,
            "5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App"
        );
        assert_eq!(toasts[0].at, now - 5_000);
        assert_eq!(toasts[0].title, "Maria Santos");
    }
}
