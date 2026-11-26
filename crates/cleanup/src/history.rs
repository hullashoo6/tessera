//! "Recently moved": items Tessera itself moved to the Recycle Bin / Trash,
//! kept locally so they can be put back. Only path, size and time are
//! stored; never file contents. Entries older than `KEEP_DAYS` or beyond
//! `KEEP_MAX` are dropped. The file never leaves the computer.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const KEEP_DAYS: i64 = 30;
pub const KEEP_MAX: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moved {
    pub original: PathBuf,
    /// Location inside the Recycle Bin, when the system reported it.
    pub recycled: Option<PathBuf>,
    pub bytes: u64,
    pub moved_at: i64,
    #[serde(default)]
    pub restored: bool,
}

pub fn load(file: &Path) -> Vec<Moved> {
    std::fs::read(file)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save(file: &Path, items: &[Moved]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(items).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(tmp, file)
}

/// Newest first, pruned by age and count.
pub fn prune(mut items: Vec<Moved>, now: i64) -> Vec<Moved> {
    items.retain(|m| now - m.moved_at <= KEEP_DAYS * 86_400);
    items.sort_by_key(|m| std::cmp::Reverse(m.moved_at));
    items.truncate(KEEP_MAX);
    items
}

pub fn append(file: &Path, new: impl IntoIterator<Item = Moved>, now: i64) -> std::io::Result<()> {
    let mut items = load(file);
    items.extend(new);
    save(file, &prune(items, now))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(t: i64) -> Moved {
        Moved {
            original: PathBuf::from(format!("C:/x/{t}")),
            recycled: None,
            bytes: 1,
            moved_at: t,
            restored: false,
        }
    }

    #[test]
    fn keeps_recent_newest_first_and_round_trips() {
        let now = 100 * 86_400;
        let items = prune(vec![m(now - 31 * 86_400), m(now - 10), m(now - 5)], now);
        assert_eq!(
            items.iter().map(|i| i.moved_at).collect::<Vec<_>>(),
            vec![now - 5, now - 10]
        );
        let dir = std::env::temp_dir().join(format!("tessera-history-{}", std::process::id()));
        let file = dir.join("moved.json");
        append(&file, items.clone(), now).unwrap();
        assert_eq!(load(&file), items);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
