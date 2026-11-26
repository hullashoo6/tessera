//! Search and filter tests on a real temporary tree.

use tessera_scan::query::{Filter, SortKey, search};
use tessera_scan::{CancelToken, Metric, ScanOptions, Tree, scan};
use tessera_testkit as kit;

fn tree() -> (kit::tempfile::TempDir, Tree) {
    let t = kit::tempfile::Builder::new()
        .prefix("tessera-search-")
        .tempdir()
        .unwrap();
    let r = t.path();
    kit::file(r, "Videos/Holiday.MP4", 5_000_000).unwrap();
    kit::file(r, "Videos/clip.mov", 2_000_000).unwrap();
    kit::file(r, "Photos/beach.jpg", 300_000).unwrap();
    kit::file(r, "Photos/holiday-notes.txt", 1_000).unwrap();
    kit::file(r, "Projects/app/node_modules/lib.js", 40_000).unwrap();
    kit::dir(r, "Empty/also-empty").unwrap();
    kit::file(r, ".hidden", 10).unwrap();
    let (tree, _) = scan(r, &ScanOptions::default(), &CancelToken::new(), &mut |_| {}).unwrap();
    (t, tree)
}

fn names(tree: &Tree, ids: &[u32]) -> Vec<String> {
    ids.iter()
        .map(|&i| tree.node(i).display_name().into_owned())
        .collect()
}

fn files() -> Filter {
    Filter {
        files: true,
        ..Filter::default()
    }
}

#[test]
fn name_search_is_case_insensitive_and_sorted_by_size() {
    let (_t, tree) = tree();
    let f = Filter {
        name: Some("HOLIDAY".into()),
        ..files()
    };
    let hits = search(&tree, &f, Metric::Logical, SortKey::Size, 100);
    assert_eq!(
        names(&tree, &hits.ids),
        ["Holiday.MP4", "holiday-notes.txt"]
    );
    assert_eq!(hits.total, 2);
}

#[test]
fn extension_and_size_filters() {
    let (_t, tree) = tree();
    let f = Filter {
        extensions: vec!["mp4".into(), "mov".into()],
        min_size: Some(3_000_000),
        ..files()
    };
    let hits = search(&tree, &f, Metric::Logical, SortKey::Size, 100);
    assert_eq!(names(&tree, &hits.ids), ["Holiday.MP4"]);
    let f = Filter {
        max_size: Some(1_000),
        ..files()
    };
    let hits = search(&tree, &f, Metric::Logical, SortKey::Name, 100);
    assert_eq!(names(&tree, &hits.ids), [".hidden", "holiday-notes.txt"]);
}

#[test]
fn folders_paths_scope_and_empty_folders() {
    let (t, tree) = tree();
    let f = Filter {
        folders: true,
        name: Some("node_modules".into()),
        ..Filter::default()
    };
    assert_eq!(
        names(
            &tree,
            &search(&tree, &f, Metric::Logical, SortKey::Size, 10).ids
        ),
        ["node_modules"]
    );

    let f = Filter {
        path: Some(format!("{}photos", std::path::MAIN_SEPARATOR)),
        ..files()
    };
    assert_eq!(
        search(&tree, &f, Metric::Logical, SortKey::Size, 10).total,
        2
    );

    let photos = tessera_scan::query::find_by_path(&tree, &t.path().join("Photos")).unwrap();
    let f = Filter {
        under: Some(photos),
        ..files()
    };
    assert_eq!(
        names(
            &tree,
            &search(&tree, &f, Metric::Logical, SortKey::Name, 10).ids
        ),
        ["beach.jpg", "holiday-notes.txt"]
    );

    let f = Filter {
        empty_folders: true,
        ..Filter::default()
    };
    assert_eq!(
        names(
            &tree,
            &search(&tree, &f, Metric::Logical, SortKey::Name, 10).ids
        ),
        ["also-empty"]
    );
}

#[test]
fn date_filters_and_limit() {
    let (_t, tree) = tree();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let old = Filter {
        modified_before: Some(now - 365 * 86_400),
        ..files()
    };
    assert_eq!(
        search(&tree, &old, Metric::Logical, SortKey::Size, 10).total,
        0
    );
    let recent = Filter {
        modified_after: Some(now - 3600),
        ..files()
    };
    let hits = search(&tree, &recent, Metric::Logical, SortKey::Size, 3);
    assert_eq!(hits.total, 6);
    assert_eq!(hits.ids.len(), 3);
}

#[test]
fn detached_items_leave_consistent_totals_and_disappear_from_queries() {
    let (_t, mut tree) = tree();
    let root_before = tree.root().logical;
    let files_before = tree.root().files;
    let videos =
        tessera_scan::query::find_by_path(&tree, &tree.root_path().join("Videos")).unwrap();
    let v_size = tree.node(videos).logical;
    let v_files = tree.node(videos).files;
    tree.detach(videos);
    tree.check_invariants().expect("invariants after detach");
    assert_eq!(tree.root().logical, root_before - v_size);
    assert_eq!(tree.root().files, files_before - v_files);
    let kids = tessera_scan::query::sorted_children(&tree, tessera_scan::ROOT, Metric::Logical);
    assert!(
        kids.iter()
            .all(|&k| tree.node(k).display_name() != "Videos")
    );
    let f = Filter {
        name: Some("holiday".into()),
        ..files()
    };
    assert_eq!(
        names(
            &tree,
            &search(&tree, &f, Metric::Logical, SortKey::Size, 10).ids
        ),
        ["holiday-notes.txt"]
    );
    tree.detach(videos); // second call is a no-op
    tree.check_invariants().unwrap();
}
