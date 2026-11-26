//! `tessera-cli ui-bench <dir>`: times every request the app's screens make
//! of a finished scan, the same way the desktop backend makes them (storage
//! map layout, contents list, largest files, search, Smart Scan, paths).
//! The window never waits for these (they run off the window thread), but a
//! slow answer still feels like lag, so each has a time budget. With
//! `--check`, exceeding a budget fails the command.

use std::path::{Path, PathBuf};
use std::time::Instant;
use tessera_scan::query::{Filter, SortKey, largest_files, search, sorted_children};
use tessera_scan::{CancelToken, Metric, NodeKind, ROOT, ScanOptions, Tree, scan};
use tessera_treemap::{Item, Params, Rect, layout};

/// (what, budget in ms). Interactive requests: 100 ms; whole-tree searches
/// and Smart Scan, which show a "working" state: 1 s.
const BUDGETS: [(&str, f64); 8] = [
    ("storage map, whole scan (1600x1000)", 100.0),
    ("storage map, largest folder", 100.0),
    ("contents list, first page", 100.0),
    ("largest files, top 100", 250.0),
    ("search by name, 2000 results", 1000.0),
    ("search: files over 100 MB", 1000.0),
    ("Smart Scan", 1000.0),
    ("full paths for 2000 items", 100.0),
];

fn map(tree: &Tree, id: u32, w: f64, h: f64) -> usize {
    layout(
        id,
        Rect::new(0.0, 0.0, w, h),
        &Params::default(),
        |parent| {
            sorted_children(tree, parent, Metric::Allocated)
                .into_iter()
                .map(|c| Item {
                    id: c,
                    weight: Metric::Allocated.of(tree, c),
                    container: tree.node(c).kind == NodeKind::Dir && tree.node(c).child_count > 0,
                })
                .collect()
        },
    )
    .len()
}

fn time<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let t = Instant::now();
    let out = f();
    (out, t.elapsed().as_secs_f64() * 1000.0)
}

pub fn run(raw: &[String]) -> Result<(), String> {
    let dir = raw.first().ok_or("ui-bench needs a folder")?;
    let check = raw.iter().any(|a| a == "--check");
    let (scanned, scan_ms) = time(|| {
        scan(
            Path::new(dir),
            &ScanOptions::default(),
            &CancelToken::new(),
            &mut |_| {},
        )
    });
    let (tree, _) = scanned.map_err(|e| e.to_string())?;
    println!("scan: {} items in {:.0} ms", tree.len(), scan_ms);

    let biggest = sorted_children(&tree, ROOT, Metric::Allocated)
        .into_iter()
        .find(|&c| tree.node(c).kind == NodeKind::Dir)
        .unwrap_or(ROOT);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // The fixture stands in for the home folder so every Smart Scan rule,
    // including the whole-folder "large old files" walk, does its full work.
    let places = tessera_insights::Places {
        home: Some(PathBuf::from(dir)),
        ..tessera_insights::Places::default()
    };

    let mut results: Vec<(String, f64)> = Vec::new();
    let (tiles, ms) = time(|| map(&tree, ROOT, 1600.0, 1000.0));
    results.push((format!("{} ({tiles} tiles)", BUDGETS[0].0), ms));
    let (tiles, ms) = time(|| map(&tree, biggest, 1600.0, 1000.0));
    results.push((format!("{} ({tiles} tiles)", BUDGETS[1].0), ms));
    let (_, ms) = time(|| {
        sorted_children(&tree, ROOT, Metric::Allocated)
            .into_iter()
            .take(100)
            .map(|c| tree.display_name(c).len())
            .sum::<usize>()
    });
    results.push((BUDGETS[2].0.to_string(), ms));
    let (_, ms) = time(|| largest_files(&tree, None, 100, Metric::Allocated));
    results.push((BUDGETS[3].0.to_string(), ms));
    let by_name = Filter {
        name: Some("1".into()),
        files: true,
        folders: true,
        ..Filter::default()
    };
    let (hits, ms) = time(|| {
        let hits = search(&tree, &by_name, Metric::Allocated, SortKey::Size, 2000);
        // The map highlight also needs every folder above each result.
        let mut containing = std::collections::BTreeSet::new();
        for &id in &hits.ids {
            containing.extend(tree.ancestors(id));
        }
        hits.total
    });
    results.push((format!("{} ({hits} matches)", BUDGETS[4].0), ms));
    let big = Filter {
        files: true,
        min_size: Some(100 << 20),
        ..Filter::default()
    };
    let (_, ms) = time(|| search(&tree, &big, Metric::Allocated, SortKey::Size, 2000).total);
    results.push((BUDGETS[5].0.to_string(), ms));
    let (found, ms) = time(|| tessera_insights::analyse(&tree, &places, now).len());
    results.push((format!("{} ({found} suggestions)", BUDGETS[6].0), ms));
    let ids: Vec<u32> = (1..tree.len() as u32)
        .step_by((tree.len() / 2000).max(1))
        .take(2000)
        .collect();
    let (_, ms) = time(|| {
        ids.iter()
            .map(|&i| tree.path_of(i).as_os_str().len())
            .sum::<usize>()
    });
    results.push((BUDGETS[7].0.to_string(), ms));

    let mut over = Vec::new();
    for ((label, ms), (_, budget)) in results.iter().zip(BUDGETS) {
        let flag = if *ms > budget { "  OVER BUDGET" } else { "" };
        println!("{label}: {ms:.1} ms (budget {budget:.0} ms){flag}");
        if *ms > budget {
            over.push(label.clone());
        }
    }
    if check && !over.is_empty() {
        return Err(format!("over budget: {}", over.join("; ")));
    }
    Ok(())
}
