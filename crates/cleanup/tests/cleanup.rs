//! Cleanup safety tests. Everything happens inside temporary folders; the
//! test "mover" renames items into a holding folder inside the fixture, so
//! nothing is ever deleted. Only `real_recycle_bin` (ignored by default)
//! uses the operating system's Recycle Bin, with one disposable file.

use std::collections::HashSet;
use std::fs;
use std::io;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tessera_cleanup::{
    FailKind, Method, Progress, Protection, RunOptions, Status, build_plan, classify, execute,
    remove_except_in_use, run,
};
use tessera_scan::query::find_by_path;
use tessera_scan::{CancelToken, ROOT, ScanOptions, Tree, scan};
use tessera_testkit as kit;

fn fixture() -> (kit::tempfile::TempDir, Tree) {
    let t = kit::tempfile::Builder::new()
        .prefix("tessera-cleanup-")
        .tempdir()
        .unwrap();
    let r = t.path().join("root");
    kit::dense_file(&r, "a/b/old.log", 4000).unwrap();
    kit::dense_file(&r, "a/keep.txt", 100).unwrap();
    kit::dense_file(&r, "cache/x.bin", 2000).unwrap();
    kit::dense_file(&r, "cache/y.bin", 3000).unwrap();
    let (tree, _) = scan(
        &r,
        &ScanOptions::default(),
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap();
    (t, tree)
}

fn id(tree: &Tree, rel: &str) -> u32 {
    find_by_path(tree, &tree.root_path().join(rel)).unwrap_or_else(|| panic!("{rel} not in tree"))
}

/// Simulated Trash: moves items into a holding folder beside the scan root.
fn holding_mover(hold: PathBuf) -> impl FnMut(&Path) -> io::Result<Option<PathBuf>> {
    fs::create_dir_all(&hold).unwrap();
    let mut n = 0;
    move |p: &Path| {
        n += 1;
        fs::rename(p, hold.join(format!("{n}"))).map(|()| None)
    }
}

fn open() -> Protection {
    Protection::new(Vec::new(), Vec::new())
}

#[test]
fn overlapping_selections_are_merged_and_root_is_refused() {
    let (_t, tree) = fixture();
    let ids = [
        id(&tree, "cache"),
        id(&tree, "cache/x.bin"),
        id(&tree, "cache"),
        ROOT,
    ];
    let plan = build_plan(&tree, &ids, &open());
    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.merged, 1);
    assert_eq!(plan.rejected.len(), 1, "root must be refused");
    assert_eq!(plan.total_logical, 5000);
}

#[test]
fn links_are_never_planned() {
    let (t, tree) = fixture();
    let target = t.path().join("outside");
    fs::create_dir_all(&target).unwrap();
    let link = tree.root_path().join("link");
    #[cfg(windows)]
    kit::junction(&link, &target).unwrap();
    #[cfg(unix)]
    kit::symlink_dir(&link, &target).unwrap();
    let (tree, _) = scan(
        tree.root_path(),
        &ScanOptions::default(),
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap();
    let plan = build_plan(&tree, &[id(&tree, "link")], &open());
    assert!(plan.items.is_empty());
    assert_eq!(plan.rejected.len(), 1);
}

#[test]
fn moves_reviewed_items_and_reports_exact_totals() {
    let (t, tree) = fixture();
    let plan = build_plan(
        &tree,
        &[id(&tree, "cache"), id(&tree, "a/b/old.log")],
        &open(),
    );
    let report = execute(&plan, &open(), &mut holding_mover(t.path().join("hold")));
    assert_eq!((report.moved, report.failed), (2, 0));
    assert_eq!(report.removed_logical, 9000);
    assert!(!tree.root_path().join("cache").exists());
    assert!(
        tree.root_path().join("a/keep.txt").exists(),
        "unselected files untouched"
    );
}

#[test]
fn changed_items_are_refused_and_missing_ones_count_as_gone() {
    let (t, tree) = fixture();
    let plan = build_plan(
        &tree,
        &[id(&tree, "cache/x.bin"), id(&tree, "cache/y.bin")],
        &open(),
    );
    fs::write(tree.root_path().join("cache/x.bin"), b"different contents").unwrap();
    fs::remove_file(tree.root_path().join("cache/y.bin")).unwrap();
    let report = execute(&plan, &open(), &mut holding_mover(t.path().join("hold")));
    assert_eq!((report.moved, report.failed, report.gone), (0, 1, 1));
    assert!(
        tree.root_path().join("cache/x.bin").exists(),
        "changed file left in place"
    );
    let changed = report.outcomes.iter().find(|o| !o.done()).unwrap();
    assert!(matches!(&changed.status, Status::Failed(f) if f.kind == FailKind::Changed));
}

/// Simulated batch remover over a holding folder; counts calls.
fn holding_batch(
    hold: PathBuf,
    calls: &mut usize,
) -> impl FnMut(&[&Path]) -> Vec<io::Result<Option<PathBuf>>> + '_ {
    fs::create_dir_all(&hold).unwrap();
    let mut n = 0;
    move |paths: &[&Path]| {
        *calls += 1;
        paths
            .iter()
            .map(|p| {
                n += 1;
                let to = hold.join(format!("{n}"));
                fs::rename(p, &to).map(|()| Some(to))
            })
            .collect()
    }
}

#[test]
fn runs_in_batches_with_progress_and_only_the_chosen_items() {
    let (t, tree) = fixture();
    let ids = [
        id(&tree, "cache/x.bin"),
        id(&tree, "cache/y.bin"),
        id(&tree, "a/b/old.log"),
        id(&tree, "a/keep.txt"),
    ];
    let plan = build_plan(&tree, &ids, &open());
    let only: HashSet<u32> = ids[..3].iter().copied().collect();
    let mut calls = 0;
    let mut seen = Vec::new();
    let report = {
        let mut remover = holding_batch(t.path().join("hold"), &mut calls);
        run(
            &plan,
            &open(),
            &RunOptions {
                only: Some(&only),
                batch: 2,
                ..RunOptions::default()
            },
            &mut |p: &Progress| seen.push((p.done, p.total)),
            &mut remover,
        )
    };
    assert_eq!(calls, 2, "3 items in batches of 2");
    assert_eq!((report.moved, report.failed), (3, 0));
    assert_eq!(seen.first(), Some(&(0, 3)));
    assert_eq!(seen.last(), Some(&(3, 3)));
    assert!(
        tree.root_path().join("a/keep.txt").exists(),
        "not chosen, untouched"
    );
    assert!(
        report.outcomes.iter().all(|o| o.recycled().is_some()),
        "landing places kept for Put back"
    );
}

#[test]
fn stop_leaves_the_rest_untouched() {
    let (t, tree) = fixture();
    let ids = [
        id(&tree, "cache/x.bin"),
        id(&tree, "cache/y.bin"),
        id(&tree, "a/keep.txt"),
    ];
    let plan = build_plan(&tree, &ids, &open());
    let stop = AtomicBool::new(false);
    let mut calls = 0;
    let report = {
        let mut remover = holding_batch(t.path().join("hold"), &mut calls);
        run(
            &plan,
            &open(),
            &RunOptions {
                batch: 1,
                stop: Some(&stop),
                ..RunOptions::default()
            },
            // Press Stop as soon as the first item is done.
            &mut |p: &Progress| {
                if p.done == 1 {
                    stop.store(true, Ordering::Relaxed)
                }
            },
            &mut remover,
        )
    };
    assert!(report.stopped);
    assert_eq!((report.moved, report.not_started), (1, 2));
}

#[test]
fn permanent_deletion_is_reported_as_deleted() {
    let (t, tree) = fixture();
    let plan = build_plan(&tree, &[id(&tree, "cache")], &open());
    let mut calls = 0;
    let report = {
        let mut remover = holding_batch(t.path().join("hold"), &mut calls);
        run(
            &plan,
            &open(),
            &RunOptions {
                method: Method::Permanent,
                ..RunOptions::default()
            },
            &mut |_| {},
            &mut remover,
        )
    };
    assert_eq!((report.deleted, report.moved), (1, 0));
    assert_eq!(report.removed_allocated, plan.total_allocated);
}

/// A file held open by another process (a PowerShell child of the test):
/// detected, named, explained, and a folder holding it can still be emptied
/// of everything else.
#[cfg(windows)]
#[test]
fn files_in_use_are_found_named_and_worked_around() {
    use std::process::{Command, Stdio};
    use tessera_cleanup::inuse;
    let (t, tree) = fixture();
    let locked = tree.root_path().join("cache/x.bin");
    // Opens the file without sharing and waits until told to stop.
    let ps = format!(
        "$f=[IO.File]::Open('{}','Open','Read','None'); [Console]::Out.WriteLine('open'); [Console]::In.ReadLine() | Out-Null; $f.Close()",
        locked.display()
    );
    let mut child = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("powershell");
    let mut line = String::new();
    std::io::BufRead::read_line(
        &mut std::io::BufReader::new(child.stdout.take().unwrap()),
        &mut line,
    )
    .unwrap();
    assert_eq!(line.trim(), "open");

    assert!(inuse::is_locked(&locked));
    assert!(!inuse::is_locked(&tree.root_path().join("cache/y.bin")));
    let inside = inuse::locked_files_in(
        &tree.root_path().join("cache"),
        10,
        1000,
        std::time::Duration::from_secs(2),
    );
    assert!(inside.complete && inside.files == vec![locked.clone()]);
    let apps = inuse::apps_using(std::slice::from_ref(&locked));
    eprintln!("held open by: {apps:?}");
    assert!(!apps.is_empty(), "Restart Manager names the holder");

    let f = classify(&io::Error::from_raw_os_error(32), &locked);
    assert_eq!(f.kind, FailKind::InUse);
    assert!(!f.apps.is_empty());
    // A folder whose move fails with "access denied" because of the open file.
    let f = classify(
        &io::Error::from_raw_os_error(5),
        &tree.root_path().join("cache"),
    );
    assert_eq!(f.kind, FailKind::InUse, "not a permission problem");
    assert_eq!(f.in_use_files, vec![locked.clone()]);

    let mut calls = 0;
    let partial = {
        let mut remover = holding_batch(t.path().join("hold"), &mut calls);
        remove_except_in_use(&tree.root_path().join("cache"), &open(), &mut remover)
    };
    assert!(partial.checked);
    assert_eq!(partial.in_use, vec![locked.clone()]);
    assert_eq!(partial.removed.len(), 1, "y.bin removed");
    assert!(locked.exists() && !tree.root_path().join("cache/y.bin").exists());

    use std::io::Write;
    writeln!(child.stdin.take().unwrap()).unwrap();
    child.wait().unwrap();
}

/// Real Recycle Bin: a batch of disposable files with one held open. The
/// open one is explained, the others move, and everything moved is taken
/// out of the Recycle Bin again afterwards.
/// cargo test -p tessera-cleanup -- --ignored real_batch
#[cfg(windows)]
#[test]
#[ignore]
fn real_batch_with_a_file_in_use() {
    let t = kit::tempfile::Builder::new()
        .prefix("tessera-batch-test-")
        .tempdir()
        .unwrap();
    let files: Vec<PathBuf> = (0..5)
        .map(|i| {
            let p = t.path().join(format!("batch-{i}.txt"));
            fs::write(&p, b"disposable").unwrap();
            p
        })
        .collect();
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&files[2])
        .unwrap();
    let refs: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
    let results = tessera_cleanup::move_many_to_trash(&refs);
    drop(held);
    for (i, r) in results.iter().enumerate() {
        if i == 2 {
            let e = r.as_ref().unwrap_err();
            assert_eq!(classify(e, &files[2]).kind, FailKind::InUse, "{e}");
            assert!(files[2].exists());
        } else {
            let landed = r.as_ref().unwrap().clone().expect("landed");
            assert!(!files[i].exists());
            // Take it out of the Recycle Bin again.
            tessera_cleanup::put_back(&landed, &files[i]).unwrap();
        }
    }
}

#[test]
fn folder_swapped_for_a_link_after_review_is_refused() {
    let (t, tree) = fixture();
    let plan = build_plan(&tree, &[id(&tree, "a/b/old.log")], &open());
    // Attack: move the real folder away and put a link with the same name
    // in its place. The file still "exists" at the reviewed path.
    let root = tree.root_path();
    fs::rename(root.join("a"), t.path().join("elsewhere")).unwrap();
    #[cfg(windows)]
    kit::junction(&root.join("a"), &t.path().join("elsewhere")).unwrap();
    #[cfg(unix)]
    kit::symlink_dir(&root.join("a"), &t.path().join("elsewhere")).unwrap();
    assert!(root.join("a/b/old.log").exists());

    let report = execute(&plan, &open(), &mut holding_mover(t.path().join("hold")));
    assert_eq!((report.moved, report.failed), (0, 1));
    assert!(
        t.path().join("elsewhere/b/old.log").exists(),
        "file behind the link untouched"
    );
}

#[test]
fn mover_failures_are_reported_not_retried() {
    let (t, tree) = fixture();
    let plan = build_plan(
        &tree,
        &[id(&tree, "cache/x.bin"), id(&tree, "a/keep.txt")],
        &open(),
    );
    let hold = t.path().join("hold");
    fs::create_dir_all(&hold).unwrap();
    let mut calls = 0;
    let report = execute(&plan, &open(), &mut |p: &Path| {
        calls += 1;
        if p.ends_with("x.bin") {
            Err(io::Error::other("simulated failure"))
        } else {
            fs::rename(p, hold.join("k")).map(|()| None)
        }
    });
    assert_eq!(calls, 2, "one attempt per item");
    assert_eq!((report.moved, report.failed), (1, 1));
    assert!(tree.root_path().join("cache/x.bin").exists());
}

#[test]
fn success_claimed_but_item_still_present_counts_as_failure() {
    let (_t, tree) = fixture();
    let plan = build_plan(&tree, &[id(&tree, "a/keep.txt")], &open());
    let report = execute(&plan, &open(), &mut |_p: &Path| Ok(None));
    assert_eq!((report.moved, report.failed), (0, 1));
}

#[test]
fn system_locations_are_protected() {
    let p = Protection::for_this_system();
    if cfg!(windows) {
        let sys = std::env::var("SystemRoot").unwrap();
        let home = PathBuf::from(std::env::var("USERPROFILE").unwrap());
        for blocked in [
            PathBuf::from("C:\\"),
            PathBuf::from(&sys),
            Path::new(&sys).join("System32\\drivers"),
            PathBuf::from(std::env::var("ProgramFiles").unwrap()).join("Something"),
            PathBuf::from("C:\\Users"),
            home.clone(),
            home.join("Documents"),
            PathBuf::from("C:\\pagefile.sys"),
        ] {
            assert!(
                p.check(&blocked).is_err(),
                "{} must be blocked",
                blocked.display()
            );
        }
        assert!(p.check(&home.join("Downloads\\old-installer.exe")).is_ok());
        assert!(p.check(&home.join("AppData\\Local\\Temp\\x")).is_ok());
    } else {
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        for blocked in [
            PathBuf::from("/"),
            PathBuf::from("/System/Library"),
            PathBuf::from("/usr/bin"),
            home.clone(),
            home.join("Library"),
        ] {
            assert!(
                p.check(&blocked).is_err(),
                "{} must be blocked",
                blocked.display()
            );
        }
        assert!(p.check(&home.join("Downloads/old.dmg")).is_ok());
    }
    assert!(p.check(Path::new("relative/../escape")).is_err());
}

/// Uses the real Recycle Bin / Trash with one disposable file.
/// Run with: cargo test -p tessera-cleanup -- --ignored
#[test]
#[ignore]
fn real_recycle_bin() {
    let t = kit::tempfile::Builder::new()
        .prefix("tessera-recycle-test-")
        .tempdir()
        .unwrap();
    let r = t.path().join("root");
    kit::dense_file(&r, "tessera-recycle-test.txt", 64).unwrap();
    let (tree, _) = scan(
        &r,
        &ScanOptions::default(),
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap();
    let plan = build_plan(&tree, &[id(&tree, "tessera-recycle-test.txt")], &open());
    let report = execute(&plan, &open(), &mut |p: &Path| {
        tessera_cleanup::move_to_trash(p)
    });
    assert_eq!(
        (report.moved, report.failed),
        (1, 0),
        "{:?}",
        report.outcomes
    );
    assert!(!r.join("tessera-recycle-test.txt").exists());
}

#[test]
fn links_above_the_scanned_folder_are_allowed() {
    // Reproduces macOS /var -> /private/var and Windows short-name paths:
    // the user scans a folder whose own path passes through a link.
    let t = kit::tempfile::Builder::new()
        .prefix("tessera-cleanup-")
        .tempdir()
        .unwrap();
    kit::dense_file(&t.path().join("real/root"), "junk.tmp", 500).unwrap();
    let via = t.path().join("via");
    #[cfg(windows)]
    kit::junction(&via, &t.path().join("real")).unwrap();
    #[cfg(unix)]
    kit::symlink_dir(&via, &t.path().join("real")).unwrap();
    let (tree, _) = scan(
        &via.join("root"),
        &ScanOptions::default(),
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap();
    let plan = build_plan(&tree, &[id(&tree, "junk.tmp")], &open());
    let report = execute(&plan, &open(), &mut holding_mover(t.path().join("hold")));
    assert_eq!(
        (report.moved, report.failed),
        (1, 0),
        "{:?}",
        report.outcomes
    );
}

/// Real Recycle Bin / Trash round trip with disposable files. Run with:
/// cargo test -p tessera-cleanup -- --ignored put_back
#[cfg(any(windows, target_os = "macos"))]
#[test]
#[ignore]
fn put_back_restores_from_the_real_recycle_bin_and_never_overwrites() {
    let t = tessera_testkit::tempfile::Builder::new()
        .prefix("tessera-putback-")
        .tempdir()
        .unwrap();
    let original = t.path().join("put-back-trial.txt");
    fs::write(&original, b"restore me exactly").unwrap();
    let recycled = tessera_cleanup::move_to_trash(&original)
        .unwrap()
        .expect("Windows reports where it landed");
    assert!(!original.exists() && recycled.exists());
    tessera_cleanup::put_back(&recycled, &original).unwrap();
    assert_eq!(fs::read(&original).unwrap(), b"restore me exactly");
    assert!(!recycled.exists(), "no longer in the Recycle Bin");
    #[cfg(windows)]
    {
        let index = recycled.with_file_name(
            recycled
                .file_name()
                .unwrap()
                .to_string_lossy()
                .replacen("$R", "$I", 1),
        );
        assert!(!index.exists(), "Recycle Bin entry removed");
    }

    // Something new at the original path: never overwritten.
    let second = t.path().join("collision-trial.txt");
    fs::write(&second, b"old").unwrap();
    let recycled2 = tessera_cleanup::move_to_trash(&second).unwrap().unwrap();
    fs::write(&second, b"new file the person made").unwrap();
    let err = tessera_cleanup::put_back(&recycled2, &second).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(&second).unwrap(), b"new file the person made");
    fs::remove_file(&second).unwrap(); // the test's own file
    tessera_cleanup::put_back(&recycled2, &second).unwrap();
    assert_eq!(fs::read(&second).unwrap(), b"old");
}

/// Real cloud test: TESSERA_CLOUD_TEST_FILE must name a disposable file
/// inside a OneDrive (or other Cloud Files) folder that has finished syncing.
/// cargo test -p tessera-cleanup -- --ignored free_up_space
#[cfg(windows)]
#[test]
#[ignore]
fn free_up_space_with_a_real_cloud_file() {
    use tessera_cleanup::cloud::{CloudState, free_up, state};
    let file = std::path::PathBuf::from(
        std::env::var("TESSERA_CLOUD_TEST_FILE").expect("set TESSERA_CLOUD_TEST_FILE"),
    );
    let roots = tessera_cleanup::cloud::sync_roots();
    eprintln!(
        "sync roots: {}; file under a root: {}",
        roots.len(),
        roots.iter().any(|r| file.starts_with(r))
    );
    eprintln!("find data: {:x?}", tessera_cleanup::cloud::find_data(&file));
    assert_eq!(
        state(&file),
        CloudState::Downloaded,
        "the test file should be synced and downloaded"
    );
    free_up(&file).unwrap();
    let mut s = state(&file);
    for _ in 0..60 {
        if s == CloudState::OnlineOnly {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
        s = state(&file);
    }
    assert_eq!(
        s,
        CloudState::OnlineOnly,
        "the sync app should have removed the local copy"
    );
    assert!(
        file.exists(),
        "the file is still listed (kept in the cloud)"
    );
    // A normal local file is refused.
    let local = std::env::temp_dir().join("tessera-not-cloud.txt");
    fs::write(&local, b"x").unwrap();
    assert!(free_up(&local).is_err());
    fs::remove_file(&local).unwrap();
}
