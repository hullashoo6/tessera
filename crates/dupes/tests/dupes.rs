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
        .map(|&i| tree.display_name(i).into_owned())
        .collect();
    assert!(!names.iter().any(|n| n == "almost.mp4"));
    let second = &report.groups[1];
    assert_eq!(second.ids.len(), 2, "hard link not counted as a duplicate");
    assert_eq!(second.hash.len(), 64);
}

/// Progress has exact totals for each stage (so the screen can show a bar
/// and time left), and the groups found so far add up to the final result,
/// with several files read at once.
#[test]
fn progress_has_exact_totals_and_counts_what_was_found() {
    let t = kit::tempfile::Builder::new()
        .prefix("tessera-dupes-")
        .tempdir()
        .unwrap();
    let r = t.path().join("root");
    fs::create_dir_all(&r).unwrap();
    // 4 pairs of copies, each pair a different size and content.
    for k in 0..4u8 {
        let data = content(10 + k, (1 << 20) + usize::from(k) * 4096);
        fs::write(r.join(format!("a{k}.bin")), &data).unwrap();
        fs::write(r.join(format!("b{k}.bin")), &data).unwrap();
    }
    // Same size as pair 0 but different start: ruled out by its samples.
    let mut other = content(10, 1 << 20);
    other[0] ^= 1;
    fs::write(r.join("other.bin"), &other).unwrap();
    let (tree, _) = scan(
        &r,
        &ScanOptions::default(),
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap();
    let mut seen: Vec<tessera_dupes::Progress> = Vec::new();
    let opts = Options {
        sample_threads: 3,
        full_threads: 3,
        ..Options::default()
    };
    let report = find(&tree, &opts, &AtomicBool::new(false), &mut |p| {
        seen.push(*p)
    });
    assert_eq!(report.groups.len(), 4, "{:?}", report.groups);
    let s1 = seen.iter().find(|p| p.stage == 1).unwrap();
    assert_eq!(
        s1.files_total, 9,
        "every file that shares a size is sampled"
    );
    let last = seen.last().unwrap();
    assert_eq!(last.stage, 2);
    assert_eq!(last.files_total, 8, "the odd one out is not read in full");
    let pairs: u64 = (0..4u64).map(|k| 2 * ((1 << 20) + k * 4096)).sum();
    assert_eq!(last.bytes_total, pairs);
    assert_eq!((last.files_done, last.bytes_done), (8, pairs));
    assert_eq!(last.groups, 4);
    assert_eq!(
        last.reclaimable,
        report.groups.iter().map(|g| g.reclaimable()).sum::<u64>()
    );
    // Largest first.
    assert!(
        report
            .groups
            .windows(2)
            .all(|w| w[0].reclaimable() >= w[1].reclaimable())
    );
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
