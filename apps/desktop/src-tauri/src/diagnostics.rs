//! A small log kept on this computer for support: crashes, errors that
//! reached the screen, and failed scans, each with the time, Tessera's
//! version and the operating system.
//!
//! It never contains file or folder names: the home folder is replaced by
//! `~` and anything else that looks like a path becomes `<path>`. Nothing is
//! sent anywhere. Settings > Diagnostics shows the file, so the person can
//! attach it to an email if they choose. The file is capped at 256 KB (one
//! older file is kept).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

const MAX_BYTES: u64 = 256 * 1024;
/// Errors the interface may record per session (a loop cannot fill the disk).
const MAX_SCREEN_ERRORS: u32 = 50;
const MAX_MESSAGE: usize = 2_000;

static LOG: OnceLock<Log> = OnceLock::new();
static SCREEN_ERRORS: AtomicU32 = AtomicU32::new(0);

struct Log {
    path: PathBuf,
    home: Option<String>,
}

/// Sets up the log and records crashes. Called once at start-up.
pub fn init(app: &AppHandle) {
    let Ok(dir) = app.path().app_log_dir() else {
        return;
    };
    let home = app
        .path()
        .home_dir()
        .ok()
        .map(|h| h.to_string_lossy().into_owned());
    let _ = LOG.set(Log {
        path: dir.join("tessera.log"),
        home,
    });
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown".into());
        let at = info
            .location()
            .map(|l| format!(" at {}:{}", l.file(), l.line()))
            .unwrap_or_default();
        record("crash", &format!("{payload}{at}"));
        previous(info);
    }));
    record(
        "start",
        &format!(
            "Tessera {} on {} {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
    );
}

/// Appends one line. Failures to write are ignored: logging must never
/// break the app.
pub fn record(kind: &str, message: &str) {
    let Some(log) = LOG.get() else { return };
    let line = format!(
        "{} {kind}: {}\n",
        utc(super::unix_now()),
        one_line(&scrub(message, log.home.as_deref()))
    );
    let _ = append(&log.path, &line);
}

fn append(path: &Path, line: &str) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    if std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) > MAX_BYTES {
        let _ = std::fs::rename(path, path.with_extension("old.log"));
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(line.as_bytes())
}

fn one_line(s: &str) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    flat.chars().take(MAX_MESSAGE).collect()
}

/// Removes anything that could name the person's files. Must never panic:
/// it also runs while a crash is being recorded.
pub fn scrub(message: &str, home: Option<&str>) -> String {
    let mut s = message.to_string();
    if let Some(home) = home.filter(|h| h.len() > 3) {
        s = s.replace(home, "~");
        // Windows names are case-insensitive. ASCII lowering keeps byte
        // positions identical, so the ranges below are always valid.
        if cfg!(windows) && home.is_ascii() {
            let h = home.to_ascii_lowercase();
            while let Some(i) = s.to_ascii_lowercase().find(&h) {
                s.replace_range(i..i + h.len(), "~");
            }
        }
    }
    words(&quoted(&s))
}

/// Quoted text that is a path is replaced whole (paths may contain spaces).
fn quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find(['"', '`']) {
        let q = rest[start..].chars().next().unwrap_or('"');
        out.push_str(&rest[..=start]);
        let after = &rest[start + 1..];
        match after.find(q) {
            Some(end) => {
                let inner = &after[..end];
                out.push_str(if looks_like_path(inner) {
                    "<path>"
                } else {
                    inner
                });
                out.push(q);
                rest = &after[end + 1..];
            }
            None => {
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Unquoted paths: a word that looks like a path, and any following words
/// that continue it (they contain a separator), become one `<path>`.
fn words(s: &str) -> String {
    let is_sep = |c: char| {
        c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '(' | ')' | ',' | ';' | '<' | '>')
    };
    let mut toks: Vec<(bool, String)> = Vec::new();
    for c in s.chars() {
        let word = !is_sep(c);
        match toks.last_mut() {
            Some((true, w)) if word => w.push(c),
            _ => toks.push((word, c.to_string())),
        }
    }
    let continues = |t: &(bool, String)| t.0 && (t.1.contains('/') || t.1.contains('\\'));
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while let Some((word, t)) = toks.get(i) {
        i += 1;
        if *word && looks_like_path(t) {
            out.push_str("<path>");
            while let (Some(gap), Some(next)) = (toks.get(i), toks.get(i + 1)) {
                if !gap.0 && gap.1.trim().is_empty() && continues(next) {
                    i += 2;
                } else {
                    break;
                }
            }
        } else {
            out.push_str(t);
        }
    }
    out
}

fn looks_like_path(t: &str) -> bool {
    let t = t.trim();
    if t == "<path>" {
        return false;
    }
    let b = t.as_bytes();
    let drive =
        b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/');
    let unc = t.starts_with("\\\\");
    let home = t.starts_with("~/") || t.starts_with("~\\");
    let unix = t.starts_with('/') && t[1..].contains('/');
    // Source locations of Tessera's own code (crash reports) are kept.
    let ours = t.starts_with("crates/") || t.starts_with("apps/") || t.starts_with("src/");
    !ours && (drive || unc || home || unix)
}

/// "2026-11-27 01:23:45Z" from Unix seconds (UTC, no time zone data needed).
fn utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

/// An error that reached the screen (the interface's safety net).
#[tauri::command(async)]
pub fn log_error(message: String) {
    if SCREEN_ERRORS.fetch_add(1, Ordering::Relaxed) < MAX_SCREEN_ERRORS {
        record("screen error", &message);
    }
}

/// Shows the log file in File Explorer or Finder.
#[tauri::command(async)]
pub fn show_log_file(app: AppHandle) -> Result<(), String> {
    let log = LOG.get().ok_or("The log is not available.")?;
    if !log.path.exists() {
        append(&log.path, "").map_err(|_| "The log could not be created.".to_string())?;
    }
    app.opener()
        .reveal_item_in_dir(&log.path)
        .map_err(|_| "The log could not be shown.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_never_reach_the_log() {
        let home = if cfg!(windows) {
            r"C:\Users\someone"
        } else {
            "/Users/someone"
        };
        let msg = format!(
            r#"could not read "{home}{sep}Secret Plans{sep}a.txt": denied; also D:\Photos\x.jpg and /Volumes/USB/b and \\server\share\c"#,
            sep = std::path::MAIN_SEPARATOR
        );
        let out = scrub(&msg, Some(home));
        for bad in ["someone", "Secret", "Plans", "Photos", "USB", "server"] {
            assert!(!out.contains(bad), "{bad} leaked: {out}");
        }
        assert!(out.starts_with("could not read"), "{out}");
        assert!(out.contains("denied"), "{out}");
    }

    #[test]
    fn unquoted_paths_with_spaces_are_removed_whole() {
        let out = scrub(r"failed on D:\My Photos\Trip 2024\a.jpg today", None);
        assert_eq!(out, "failed on <path> today");
    }

    #[cfg(windows)]
    #[test]
    fn home_is_matched_in_any_case() {
        let out = scrub(r"c:\users\SOMEONE\x", Some(r"C:\Users\someone"));
        assert!(!out.to_lowercase().contains("someone"), "{out}");
    }

    #[test]
    fn our_source_locations_and_plain_words_stay() {
        let out = scrub("index out of bounds at crates/scan/src/model.rs:120", None);
        assert_eq!(out, "index out of bounds at crates/scan/src/model.rs:120");
    }

    #[test]
    fn dates_are_formatted_in_utc() {
        assert_eq!(utc(0), "1970-01-01 00:00:00Z");
        assert_eq!(utc(1_795_725_180), "2026-11-26 20:33:00Z");
        assert_eq!(utc(951_782_400), "2000-02-29 00:00:00Z");
    }

    #[test]
    fn the_file_is_capped() {
        let dir = std::env::temp_dir().join(format!("tessera-log-test-{}", std::process::id()));
        let path = dir.join("tessera.log");
        let line = format!("{}\n", "x".repeat(1_000));
        for _ in 0..300 {
            append(&path, &line).unwrap();
        }
        let size = std::fs::metadata(&path).unwrap().len();
        assert!(size <= MAX_BYTES + 1_001, "{size}");
        assert!(path.with_extension("old.log").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
