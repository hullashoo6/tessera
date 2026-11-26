//! Real-filesystem tests for the scanner. Every test works inside its own
//! temporary directory; nothing outside it is read or written.

use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tessera_scan::query::{find_by_path, largest_files, sorted_children};
use tessera_scan::{
    CancelToken, HardLinkCoverage, HardLinkMode, IssueKind, IssueStage, Metric, NodeKind, ROOT,
    ScanError, ScanOptions, ScanSummary, Tree, flags, prepare_root, scan, scan_shared,
};
use tessera_testkit as kit;

fn run(root: &Path, opts: &ScanOptions) -> (Tree, ScanSummary) {
    let (tree, summary) =
        scan(root, opts, &CancelToken::new(), &mut |_| {}).expect("scan succeeds");
    tree.check_invariants()
        .expect("aggregation invariants hold");
    assert_eq!(summary.internal_errors, 0);
    (tree, summary)
}

/// Skipping is allowed on developer machines (e.g. Windows without Developer
/// Mode), but not in CI, whose runners provide these capabilities: a skipped
/// check must never look like a pass.
fn skip(reason: &str) {
    if std::env::var_os("CI").is_some() {
        panic!("required test capability unavailable in CI: {reason}");
    }
    eprintln!("note: {reason}");
}

fn tmp() -> kit::tempfile::TempDir {
    kit::tempfile::Builder::new()
        .prefix("tessera-test-")
        .tempdir()
        .expect("temp dir")
}

fn child(tree: &Tree, parent: u32, name: &str) -> u32 {
    tree.children(parent)
        .find(|&c| tree.node(c).name.to_str() == Some(name))
        .unwrap_or_else(|| panic!("child {name} not found"))
}

#[test]
fn empty_directory() {
    let t = tmp();
    let (tree, s) = run(t.path(), &ScanOptions::default());
    let root = tree.root();
    assert_eq!(
        (root.files, root.dirs, root.logical, root.allocated),
        (0, 0, 0, 0)
    );
    assert!(!root.has(flags::PENDING));
    assert!(!s.cancelled);
    assert_eq!(s.progress.issues, 0);
}

#[test]
fn exact_sizes_and_aggregation() {
    let t = tmp();
    let r = t.path();
    kit::file(r, "a.txt", 100).unwrap();
    kit::file(r, "b/c.bin", 5_000).unwrap();
    kit::file(r, "b/d/e.dat", 12_345).unwrap();
    kit::file(r, "b/d/f", 0).unwrap();
    kit::dir(r, "g").unwrap();

    let (tree, _) = run(r, &ScanOptions::default());
    let root = tree.root();
    assert_eq!(root.logical, 17_445);
    assert_eq!(root.files, 4);
    assert_eq!(root.dirs, 3);

    let b = child(&tree, ROOT, "b");
    assert_eq!(tree.node(b).logical, 17_345);
    assert_eq!(tree.node(b).files, 3);
    assert_eq!(tree.node(b).dirs, 1);
    let g = child(&tree, ROOT, "g");
    assert_eq!(tree.node(g).kind, NodeKind::Dir);
    assert_eq!(tree.node(g).logical, 0);

    let biggest: Vec<_> = largest_files(&tree, None, 10, Metric::Logical)
        .into_iter()
        .map(|id| tree.node(id).display_name().into_owned())
        .collect();
    assert_eq!(biggest, ["e.dat", "c.bin", "a.txt", "f"]);

    let order: Vec<_> = sorted_children(&tree, ROOT, Metric::Logical)
        .into_iter()
        .map(|id| tree.node(id).display_name().into_owned())
        .collect();
    assert_eq!(order, ["b", "a.txt", "g"]);

    let e = find_by_path(&tree, &r.join("b").join("d").join("e.dat")).expect("found by path");
    assert_eq!(tree.node(e).logical, 12_345);
    assert_eq!(
        tree.path_of(e),
        std::path::absolute(r.join("b/d/e.dat")).unwrap()
    );
}

#[test]
fn allocated_size_is_reported_for_dense_files() {
    let t = tmp();
    kit::dense_file(t.path(), "dense.bin", 10_000).unwrap();
    let (tree, s) = run(t.path(), &ScanOptions::default());
    let f = child(&tree, ROOT, "dense.bin");
    let n = tree.node(f);
    assert_eq!(n.logical, 10_000);
    assert!(n.allocated >= 10_000, "allocated {} < logical", n.allocated);
    if cfg!(windows) {
        let cluster = s.cluster_size.expect("cluster size known on NTFS");
        assert_eq!(n.allocated % cluster, 0);
        assert!(n.has(flags::ALLOC_ESTIMATED));
        assert!(tree.root().has(flags::SUBTREE_ESTIMATED));
    }
}

#[test]
fn unicode_and_unusual_names_round_trip() {
    let t = tmp();
    let names = [
        "naïve café.txt",
        "日本語のファイル.txt",
        "emoji 😀🎉.bin",
        "עברית.doc",
        "Ελληνικά",
        "e\u{301} combining",
        "  leading spaces",
        "semi;colon & ampersand",
        "#hash %percent",
    ];
    for (i, n) in names.iter().enumerate() {
        kit::file(t.path(), n, i as u64 * 10 + 1).unwrap();
    }
    let (tree, _) = run(t.path(), &ScanOptions::default());
    assert_eq!(tree.root().files as usize, names.len());
    for (i, n) in names.iter().enumerate() {
        let id = child(&tree, ROOT, n);
        assert_eq!(tree.node(id).logical, i as u64 * 10 + 1);
        assert!(
            std::fs::symlink_metadata(tree.path_of(id)).is_ok(),
            "path_of({n}) resolves"
        );
    }
}

#[test]
fn long_paths_beyond_260_characters() {
    let t = tmp();
    let segment = "a_rather_long_directory_name_used_for_testing_paths";
    let mut rel = std::path::PathBuf::new();
    for _ in 0..8 {
        rel.push(segment);
    }
    rel.push("deep_file.bin");
    let full = t.path().join(&rel);
    assert!(full.as_os_str().len() > 300);
    kit::file(t.path(), &rel, 777).unwrap();

    let (tree, s) = run(t.path(), &ScanOptions::default());
    assert_eq!(s.progress.issues, 0);
    assert_eq!(tree.root().logical, 777);
    assert_eq!(tree.root().dirs, 8);
    let id = largest_files(&tree, None, 1, Metric::Logical)[0];
    assert!(std::fs::metadata(tree.path_of(id)).is_ok());
}

#[test]
fn links_and_junctions_are_not_followed() {
    let t = tmp();
    let outside = tmp();
    kit::file(outside.path(), "big.bin", 8 * 1024 * 1024).unwrap();
    kit::file(t.path(), "real.txt", 10).unwrap();

    let mut made = 0;
    #[cfg(windows)]
    {
        kit::junction(&t.path().join("junction"), outside.path())
            .expect("mklink /J works without admin");
        made += 1;
    }
    if kit::symlink_dir(&t.path().join("symlink"), outside.path()).is_ok() {
        made += 1;
    } else {
        skip("directory symlinks need Developer Mode on Windows; symlink case skipped");
    }
    assert!(made > 0, "at least one kind of link must be testable");

    let (tree, _) = run(t.path(), &ScanOptions::default());
    assert!(
        tree.root().logical < 1024,
        "link targets were counted: {}",
        tree.root().logical
    );
    assert_eq!(tree.root().files, 1);
    for c in tree.children(ROOT) {
        let n = tree.node(c);
        if n.name.to_str() != Some("real.txt") {
            assert_eq!(
                n.kind,
                NodeKind::Link,
                "{} should be a link",
                n.display_name()
            );
            assert_eq!(n.child_count, 0);
        }
    }
    assert!(
        tree.nodes()
            .iter()
            .all(|n| n.name.to_str() != Some("big.bin"))
    );
}

#[test]
fn hard_links_are_counted_once_when_checked() {
    let t = tmp();
    let a = kit::dense_file(t.path(), "a/original.bin", 1 << 20).unwrap();
    kit::dir(t.path(), "b").unwrap();
    std::fs::hard_link(&a, t.path().join("b").join("link.bin")).unwrap();

    // Default options: a 1 MiB file is above the Windows Auto threshold and
    // every file is checked on Unix.
    let (tree, s) = run(t.path(), &ScanOptions::default());
    assert_ne!(s.hard_links, HardLinkCoverage::NotChecked);
    assert_eq!(tree.root().files, 2);
    assert_eq!(tree.root().logical, 1 << 20);
    let linked: Vec<_> = tree
        .nodes()
        .iter()
        .filter(|n| n.has(flags::HARD_LINKED))
        .collect();
    assert_eq!(linked.len(), 2);
    assert_eq!(
        linked
            .iter()
            .filter(|n| n.has(flags::COUNTED_ELSEWHERE))
            .count(),
        1
    );

    let (tree, s) = run(
        t.path(),
        &ScanOptions {
            hard_links: HardLinkMode::Off,
            ..ScanOptions::default()
        },
    );
    assert_eq!(s.hard_links, HardLinkCoverage::NotChecked);
    assert_eq!(tree.root().logical, 2 << 20);
}

#[cfg(windows)]
#[test]
fn small_hard_links_below_windows_threshold_are_reported_as_unchecked() {
    let t = tmp();
    let a = kit::dense_file(t.path(), "x.bin", 4096).unwrap();
    std::fs::hard_link(&a, t.path().join("y.bin")).unwrap();

    let (tree, s) = run(t.path(), &ScanOptions::default());
    assert_eq!(s.hard_links, HardLinkCoverage::FilesAtLeast(64 * 1024));
    assert_eq!(
        tree.root().logical,
        8192,
        "small links are counted per path"
    );

    let thorough = ScanOptions {
        hard_links: HardLinkMode::Thorough,
        ..ScanOptions::default()
    };
    let (tree, s) = run(t.path(), &thorough);
    assert_eq!(s.hard_links, HardLinkCoverage::AllFiles);
    assert_eq!(tree.root().logical, 4096);
}

#[test]
fn sparse_files_report_small_allocation() {
    let t = tmp();
    let p = t.path().join("sparse.img");
    kit::make_sparse(&p, 256 * 1024 * 1024).unwrap();
    let (tree, _) = run(t.path(), &ScanOptions::default());
    let n = tree.node(child(&tree, ROOT, "sparse.img"));
    assert_eq!(n.logical, 256 * 1024 * 1024);
    assert!(
        n.allocated < 16 * 1024 * 1024,
        "sparse file allocated {} bytes",
        n.allocated
    );
    if cfg!(windows) {
        assert!(n.has(flags::SPARSE));
        assert!(
            !n.has(flags::ALLOC_ESTIMATED),
            "sparse allocation is measured, not estimated"
        );
    }
}

#[test]
fn permission_denied_directories_are_reported_not_fatal() {
    let t = tmp();
    kit::file(t.path(), "visible.txt", 50).unwrap();
    let locked = kit::dir(t.path(), "locked").unwrap();
    kit::file(&locked, "secret.txt", 999).unwrap();
    let guard = match kit::DenyListing::new(&locked) {
        Ok(g) => g,
        Err(e) => {
            skip(&format!("cannot restrict permissions here ({e})"));
            return;
        }
    };
    let (tree, s) = run(t.path(), &ScanOptions::default());
    drop(guard);

    let id = child(&tree, ROOT, "locked");
    assert!(tree.node(id).has(flags::UNREADABLE));
    assert!(tree.root().has(flags::SUBTREE_INCOMPLETE));
    assert_eq!(tree.root().logical, 50);
    assert!(s.issues.iter().any(|i| i.node == id
        && i.kind == IssueKind::PermissionDenied
        && i.stage == IssueStage::OpenDirectory));
}

#[test]
fn generated_fixture_totals_match_exactly() {
    let t = tmp();
    let root = t.path().join("fixture");
    let stats = kit::generate(&root, kit::GenSpec::new(5_000), |_| {}).unwrap();
    for threads in [1, 4] {
        let (tree, s) = run(
            &root,
            &ScanOptions {
                threads,
                ..ScanOptions::default()
            },
        );
        assert_eq!(
            u64::from(tree.root().files),
            stats.files,
            "threads={threads}"
        );
        assert_eq!(u64::from(tree.root().dirs), stats.dirs);
        assert_eq!(tree.root().logical, stats.logical_bytes);
        assert_eq!(s.progress.issues, 0);
        assert!(s.time_to_first_result.is_some());
    }
}

#[test]
fn cancellation_before_start_returns_immediately() {
    let t = tmp();
    let root = t.path().join("fixture");
    kit::generate(&root, kit::GenSpec::new(2_000), |_| {}).unwrap();
    let cancel = CancelToken::new();
    cancel.cancel();
    let started = Instant::now();
    let (tree, s) = scan(&root, &ScanOptions::default(), &cancel, &mut |_| {}).unwrap();
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(s.cancelled);
    tree.check_invariants().unwrap();
    assert!(tree.root().has(flags::SUBTREE_INCOMPLETE));
}

#[test]
fn cancellation_mid_scan_leaves_consistent_partial_tree() {
    let t = tmp();
    let root = t.path().join("fixture");
    kit::generate(&root, kit::GenSpec::new(20_000), |_| {}).unwrap();
    let cancel = CancelToken::new();
    let mut cancelled_at = None;
    let opts = ScanOptions {
        threads: 2,
        progress_interval: Duration::from_millis(10),
        ..ScanOptions::default()
    };
    let (tree, s) = scan(&root, &opts, &cancel, &mut |_| {
        if cancelled_at.is_none() {
            cancel.cancel();
            cancelled_at = Some(Instant::now());
        }
    })
    .unwrap();
    tree.check_invariants().unwrap();
    if let (true, Some(at)) = (s.cancelled, cancelled_at) {
        assert!(
            at.elapsed() < Duration::from_secs(1),
            "cancel took {:?}",
            at.elapsed()
        );
        assert!(tree.root().has(flags::SUBTREE_INCOMPLETE));
    } else {
        eprintln!("note: scan finished before the first progress callback");
    }
}

#[test]
fn files_changing_during_scan_do_not_break_it() {
    let t = tmp();
    let root = t.path().join("fixture");
    kit::generate(&root, kit::GenSpec::new(5_000), |_| {}).unwrap();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let churn_root = root.clone();
    let churn_stop = stop.clone();
    let churn = std::thread::spawn(move || {
        let mut i = 0u64;
        while !churn_stop.load(std::sync::atomic::Ordering::Relaxed) {
            let d = churn_root.join(format!("churn_{}", i % 7));
            let _ = std::fs::create_dir_all(&d);
            let _ = std::fs::write(d.join(format!("f{i}")), b"x");
            if i.is_multiple_of(3) {
                let _ = std::fs::remove_dir_all(churn_root.join(format!("churn_{}", (i + 3) % 7)));
            }
            i += 1;
        }
    });
    for _ in 0..3 {
        let (_tree, s) = run(&root, &ScanOptions::default());
        assert!(!s.cancelled);
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    churn.join().unwrap();
}

#[test]
fn live_tree_is_consistent_while_scanning() {
    let t = tmp();
    let root = t.path().join("fixture");
    kit::generate(&root, kit::GenSpec::new(10_000), |_| {}).unwrap();
    let shared = Arc::new(RwLock::new(Tree::new(prepare_root(&root).unwrap())));
    let reader_tree = shared.clone();
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reader_done = done.clone();
    let reader = std::thread::spawn(move || {
        let mut checks = 0;
        while !reader_done.load(std::sync::atomic::Ordering::Relaxed) {
            reader_tree
                .read()
                .unwrap()
                .check_invariants()
                .expect("partial tree consistent");
            checks += 1;
            std::thread::sleep(Duration::from_millis(1));
        }
        checks
    });
    let s = scan_shared(
        &shared,
        &ScanOptions::default(),
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap();
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    let checks = reader.join().unwrap();
    assert!(checks > 0);
    assert!(!s.cancelled);
    assert_eq!(shared.read().unwrap().root().files, 10_001); // + marker file
}

#[test]
fn invalid_roots_are_rejected() {
    let t = tmp();
    let missing = t.path().join("does-not-exist");
    assert!(matches!(
        scan(
            &missing,
            &ScanOptions::default(),
            &CancelToken::new(),
            &mut |_| {}
        ),
        Err(ScanError::RootNotFound(_))
    ));
    let f = kit::file(t.path(), "plain.txt", 1).unwrap();
    assert!(matches!(
        scan(
            &f,
            &ScanOptions::default(),
            &CancelToken::new(),
            &mut |_| {}
        ),
        Err(ScanError::RootNotDirectory(_))
    ));
}

#[test]
fn volumes_are_listed_with_os_reported_capacity() {
    let vols = tessera_scan::list_volumes();
    assert!(!vols.is_empty());
    let t = tmp();
    let space = tessera_scan::volume_space(t.path()).expect("capacity of temp volume");
    assert!(space.total > 0 && space.free <= space.total && space.available <= space.total);
}
