//! Duplicate detection on a disposable tree.

use std::fs;
use std::sync::atomic::AtomicBool;
use tessera_dupes::{Options, find};
use tessera_scan::{CancelToken, HardLinkMode, ScanOptions, scan};
use tessera_testkit as kit;

fn content(seed: u8, len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| ((i * 31 + seed as usize) % 251) as u8)
        .collect()
}

#[test]
fn only_identical_content_is_reported() {
    let t = kit::tempfile::Builder::new()
        .prefix("tessera-dupes-")
        .tempdir()
        .unwrap();
    let r = t.path().join("root");
    let big = content(7, 3 << 20);
    for p in [
        "a/holiday.mp4",
        "b/holiday copy.mp4",
        "c/backup/holiday.mp4",
    ] {
        let path = r.join(p);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, &big).unwrap();
    }
    // Same size, same first and last 64 KiB, different middle: NOT a duplicate.
    let mut near = big.clone();
    near[(3 << 20) / 2] ^= 0xFF;
    fs::create_dir_all(r.join("d")).unwrap();
    fs::write(r.join("d/almost.mp4"), &near).unwrap();
    // A second identical pair of a different size.
    let doc = content(3, 2 << 20);
    fs::create_dir_all(r.join("e")).unwrap();
    fs::write(r.join("e/report.pdf"), &doc).unwrap();
    fs::write(r.join("e/report (1).pdf"), &doc).unwrap();
    // Small identical files below the threshold are ignored.
    fs::write(r.join("e/tiny1.txt"), b"same").unwrap();
    fs::write(r.join("e/tiny2.txt"), b"same").unwrap();
    // A hard link shares storage: not a duplicate to remove.
    fs::hard_link(r.join("e/report.pdf"), r.join("e/report-link.pdf")).unwrap();

    let opts = ScanOptions {
        hard_links: HardLinkMode::Thorough,
        ..ScanOptions::default()
    };
    let (tree, _) = scan(&r, &opts, &CancelToken::new(), &mut |_| {}).unwrap();
    let report = find(
        &tree,
        &Options::default(),
        &AtomicBool::new(false),
        &mut |_| {},
    );

    assert!(!report.cancelled);
    assert_eq!(report.groups.len(), 2, "{:?}", report.groups);
    let first = &report.groups[0];
    assert_eq!(first.size, 3 << 20);
    assert_eq!(first.ids.len(), 3, "the near-identical file is excluded");
    assert_eq!(first.reclaimable(), 2 * (3 << 20));
    let names: Vec<String> = first
        .ids
        .iter()
        .map(|&i| tree.node(i).display_name().into_owned())
        .collect();
    assert!(!names.iter().any(|n| n == "almost.mp4"));
    let second = &report.groups[1];
    assert_eq!(second.ids.len(), 2, "hard link not counted as a duplicate");
    assert_eq!(second.hash.len(), 64);
}

#[test]
fn files_changed_after_the_scan_are_skipped_and_cancel_stops() {
    let t = kit::tempfile::Builder::new()
        .prefix("tessera-dupes-")
        .tempdir()
        .unwrap();
    let r = t.path().join("root");
    let data = content(1, 2 << 20);
    fs::create_dir_all(&r).unwrap();
    fs::write(r.join("x.bin"), &data).unwrap();
    fs::write(r.join("y.bin"), &data).unwrap();
    let (tree, _) = scan(
        &r,
        &ScanOptions::default(),
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap();
    fs::write(r.join("y.bin"), b"now different and shorter").unwrap();
    let report = find(
        &tree,
        &Options::default(),
        &AtomicBool::new(false),
        &mut |_| {},
    );
    assert!(report.groups.is_empty());
    assert_eq!(report.skipped, 1);

    let cancelled = find(
        &tree,
        &Options::default(),
        &AtomicBool::new(true),
        &mut |_| {},
    );
    assert!(cancelled.cancelled);
}
