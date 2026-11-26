//! tessera-cli — read-only scanner, fixture generator and benchmark runner.
//!
//!   tessera-cli volumes
//!   tessera-cli scan <path> [--threads N] [--hard-links auto|thorough|off] [--top N]
//!   tessera-cli gen <new-empty-dir> --files N [--seed S]
//!   tessera-cli bench <fixture-dir> [--runs N] [--threads N] [--hard-links MODE] [--out FILE]
//!
//! `scan` and `bench` never modify anything. `gen` only writes into a new or
//! empty directory and marks it as a fixture.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tessera_scan::query::{largest_files, sorted_children};
use tessera_scan::{
    CancelToken, HardLinkCoverage, HardLinkMode, IssueKind, Metric, ROOT, ScanOptions, ScanSummary,
    Tree, flags, list_volumes, scan, volume_space,
};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("volumes") => cmd_volumes(),
        Some("scan") => cmd_scan(&args[1..]),
        Some("gen") => cmd_gen(&args[1..]),
        Some("bench") => cmd_bench(&args[1..]),
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "usage:
  tessera-cli volumes
  tessera-cli scan <path> [--threads N] [--hard-links auto|thorough|off] [--top N]
  tessera-cli gen <new-empty-dir> --files N [--seed S]
  tessera-cli bench <fixture-dir> [--runs N] [--threads N] [--hard-links MODE] [--out FILE]";

struct Args {
    positional: Vec<String>,
    options: Vec<(String, String)>,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut out = Args {
        positional: Vec::new(),
        options: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if let Some(key) = a.strip_prefix("--") {
            let v = it.next().ok_or_else(|| format!("--{key} needs a value"))?;
            out.options.push((key.to_string(), v.clone()));
        } else {
            out.positional.push(a.clone());
        }
    }
    Ok(out)
}

impl Args {
    fn get(&self, key: &str) -> Option<&str> {
        self.options
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
    fn num(&self, key: &str, default: u64) -> Result<u64, String> {
        match self.get(key) {
            None => Ok(default),
            Some(v) => v.parse().map_err(|_| format!("--{key} expects a number")),
        }
    }
    fn scan_options(&self) -> Result<ScanOptions, String> {
        let hard_links = match self.get("hard-links").unwrap_or("auto") {
            "auto" => HardLinkMode::Auto,
            "thorough" => HardLinkMode::Thorough,
            "off" => HardLinkMode::Off,
            other => return Err(format!("unknown --hard-links mode '{other}'")),
        };
        Ok(ScanOptions {
            threads: self.num("threads", 0)? as usize,
            hard_links,
            ..ScanOptions::default()
        })
    }
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.2} {}", UNITS[u])
    }
}

fn cmd_volumes() -> Result<(), String> {
    for v in list_volumes() {
        let space = match v.space {
            Some(s) => format!(
                "total {}  used {}  free {}  available to you {}",
                human(s.total),
                human(s.used()),
                human(s.free),
                human(s.available)
            ),
            None => "capacity unavailable".into(),
        };
        println!(
            "{}  [{:?}, {}]  {}  {}",
            v.mount_point.display(),
            v.kind,
            v.file_system.as_deref().unwrap_or("?"),
            v.label.as_deref().unwrap_or(""),
            space
        );
    }
    Ok(())
}

fn cmd_scan(raw: &[String]) -> Result<(), String> {
    let a = parse(raw)?;
    let path = a.positional.first().ok_or("scan needs a path")?;
    let opts = a.scan_options()?;
    let top = a.num("top", 15)? as usize;
    // Ctrl+C terminates the process, which is safe for a read-only scan.
    let cancel = CancelToken::new();
    let mut last = Instant::now();
    let (tree, summary) = scan(Path::new(path), &opts, &cancel, &mut |p| {
        if last.elapsed() >= Duration::from_millis(500) {
            eprint!(
                "\r  {} files, {} folders, {} so far, {} folders queued   ",
                p.files,
                p.dirs,
                human(p.allocated_bytes),
                p.dirs_pending
            );
            last = Instant::now();
        }
    })
    .map_err(|e| format!("scan failed: {e}"))?;
    eprintln!();
    print_report(&tree, &summary, top);
    Ok(())
}

fn print_report(tree: &Tree, s: &ScanSummary, top: usize) {
    let root = tree.root();
    println!("Scanned      {}", tree.root_path().display());
    println!(
        "Status       {}",
        if s.cancelled {
            "CANCELLED (partial results)"
        } else {
            "complete"
        }
    );
    println!(
        "Elapsed      {:.3} s (first results after {:?})",
        s.elapsed.as_secs_f64(),
        s.time_to_first_result
    );
    println!("Threads      {}", s.threads);
    println!("Files        {}", root.files);
    println!("Folders      {}", root.dirs);
    println!("Links        {} (not followed)", s.progress.links);
    println!(
        "Size         {} ({} bytes)",
        human(root.logical),
        root.logical
    );
    let est = if root.has(flags::SUBTREE_ESTIMATED) {
        " (estimated)"
    } else {
        ""
    };
    println!(
        "On disk      {}{} ({} bytes)",
        human(root.allocated),
        est,
        root.allocated
    );
    if let Some(c) = s.cluster_size {
        println!("Cluster      {c} bytes");
    }
    let coverage = match s.hard_links {
        HardLinkCoverage::AllFiles => "de-duplicated (all files checked)".to_string(),
        HardLinkCoverage::FilesAtLeast(n) => {
            format!(
                "de-duplicated for files of {} or more; smaller ones counted per path",
                human(n)
            )
        }
        HardLinkCoverage::NotChecked => "not checked (each path counted)".to_string(),
    };
    println!("Hard links   {coverage}");
    if root.has(flags::SUBTREE_CLOUD) {
        println!("Cloud        some files are online-only and occupy no local space");
    }
    if let Ok(space) = volume_space(tree.root_path()) {
        println!(
            "Volume       total {}, used {}, free {} (reported by the OS)",
            human(space.total),
            human(space.used()),
            human(space.free)
        );
    }
    let denied = s
        .issues
        .iter()
        .filter(|i| i.kind == IssueKind::PermissionDenied)
        .count();
    println!(
        "Issues       {} total ({} permission denied recorded{})",
        s.progress.issues,
        denied,
        if s.issues_not_recorded > 0 {
            format!(", {} not individually recorded", s.issues_not_recorded)
        } else {
            String::new()
        }
    );
    if s.internal_errors > 0 {
        println!(
            "WARNING      {} internal scanner errors; results incomplete",
            s.internal_errors
        );
    }

    println!("\nLargest items in the root (on disk):");
    for id in sorted_children(tree, ROOT, Metric::Allocated)
        .into_iter()
        .take(top)
    {
        let n = tree.node(id);
        let tag = match n.kind {
            tessera_scan::NodeKind::Dir => "/",
            tessera_scan::NodeKind::Link => " ->",
            _ => "",
        };
        let mut notes = Vec::new();
        if n.has(flags::UNREADABLE) {
            notes.push("unreadable");
        }
        if n.has(flags::SUBTREE_INCOMPLETE) {
            notes.push("incomplete");
        }
        println!(
            "  {:>12}  {}{}  {}",
            human(n.allocated),
            n.display_name(),
            tag,
            notes.join(", ")
        );
    }
    println!("\nLargest files (on disk):");
    for id in largest_files(tree, None, top, Metric::Allocated) {
        let n = tree.node(id);
        println!(
            "  {:>12}  {}",
            human(n.allocated),
            tree.path_of(id).display()
        );
    }
}

fn cmd_gen(raw: &[String]) -> Result<(), String> {
    let a = parse(raw)?;
    let dir = PathBuf::from(a.positional.first().ok_or("gen needs a directory")?);
    let files = a.num("files", 0)?;
    if files == 0 {
        return Err("--files N is required".into());
    }
    let mut spec = tessera_testkit::GenSpec::new(files);
    spec.seed = a.num("seed", spec.seed)?;
    let started = Instant::now();
    let mut last = Instant::now();
    let stats = tessera_testkit::generate(&dir, spec, |done| {
        if last.elapsed() >= Duration::from_secs(1) {
            eprint!("\r  {done}/{files} files");
            last = Instant::now();
        }
    })
    .map_err(|e| format!("generation failed: {e}"))?;
    eprintln!();
    println!(
        "Generated {} files in {} folders, {} logical bytes, in {:.1} s",
        stats.files,
        stats.dirs,
        stats.logical_bytes,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

fn cmd_bench(raw: &[String]) -> Result<(), String> {
    let a = parse(raw)?;
    let dir = PathBuf::from(
        a.positional
            .first()
            .ok_or("bench needs a fixture directory")?,
    );
    if !dir.join(tessera_testkit::MARKER).exists() {
        return Err("bench only runs on generated fixtures (marker file missing)".into());
    }
    let runs = a.num("runs", 5)?.max(1);
    let opts = a.scan_options()?;
    let mut durations = Vec::new();
    let mut first = Vec::new();
    let mut files = 0u64;
    let mut dirs = 0u64;
    let mut threads = 0;
    for run in 0..=runs {
        let cancel = CancelToken::new();
        let (tree, s) = scan(&dir, &opts, &cancel, &mut |_| {}).map_err(|e| e.to_string())?;
        if run == 0 {
            eprintln!("  warm-up run: {:.3} s", s.elapsed.as_secs_f64());
            continue; // warm-up: populate the OS metadata cache
        }
        durations.push(s.elapsed.as_secs_f64());
        first.push(
            s.time_to_first_result
                .map(|d| d.as_secs_f64() * 1000.0)
                .unwrap_or(f64::NAN),
        );
        files = u64::from(tree.root().files);
        dirs = u64::from(tree.root().dirs);
        threads = s.threads;
        eprintln!("  run {run}: {:.3} s", s.elapsed.as_secs_f64());
    }

    // Cancellation latency: cancel shortly after the scan starts and time
    // how long the scan takes to return.
    let mut cancel_ms = Vec::new();
    for _ in 0..3 {
        let cancel = CancelToken::new();
        let fired: AtomicBool = AtomicBool::new(false);
        let mut cancelled_at: Option<Instant> = None;
        let started = Instant::now();
        let _ = scan(
            &dir,
            &ScanOptions {
                progress_interval: Duration::from_millis(20),
                ..opts.clone()
            },
            &cancel,
            &mut |_| {
                if started.elapsed() >= Duration::from_millis(60)
                    && !fired.swap(true, Ordering::Relaxed)
                {
                    cancel.cancel();
                    cancelled_at = Some(Instant::now());
                }
            },
        );
        if let Some(t) = cancelled_at {
            cancel_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        }
    }

    let peak = peak_memory_bytes();
    durations.sort_by(f64::total_cmp);
    let median = durations[durations.len() / 2];
    let min = durations[0];
    let max = durations[durations.len() - 1];
    let first_med = {
        let mut f = first.clone();
        f.sort_by(f64::total_cmp);
        f[f.len() / 2]
    };
    let cancel_max = cancel_ms.iter().copied().fold(f64::NAN, f64::max);
    let json = format!(
        "{{\n  \"tool_version\": \"{}\",\n  \"os\": \"{}\",\n  \"arch\": \"{}\",\n  \"threads\": {},\n  \
         \"hard_links\": \"{:?}\",\n  \"files\": {},\n  \"dirs\": {},\n  \"runs\": {},\n  \
         \"warm_cache\": true,\n  \"scan_seconds_median\": {:.4},\n  \"scan_seconds_min\": {:.4},\n  \
         \"scan_seconds_max\": {:.4},\n  \"first_result_ms_median\": {:.2},\n  \
         \"cancel_latency_ms_max\": {:.2},\n  \"cancel_samples\": {},\n  \"peak_memory_bytes\": {}\n}}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        threads,
        opts.hard_links,
        files,
        dirs,
        runs,
        median,
        min,
        max,
        first_med,
        cancel_max,
        cancel_ms.len(),
        peak.map(|p| p.to_string()).unwrap_or_else(|| "null".into()),
    );
    print!("{json}");
    if let Some(out) = a.get("out") {
        std::fs::write(out, &json).map_err(|e| format!("cannot write {out}: {e}"))?;
    }
    Ok(())
}

#[cfg(windows)]
fn peak_memory_bytes() -> Option<u64> {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    // SAFETY: zeroed counters with correct cb; pseudo-handle needs no close.
    unsafe {
        let mut c: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) == 0 {
            return None;
        }
        Some(c.PeakWorkingSetSize as u64)
    }
}

#[cfg(unix)]
fn peak_memory_bytes() -> Option<u64> {
    // SAFETY: zeroed rusage is a valid out-parameter.
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) } != 0 {
        return None;
    }
    let raw = ru.ru_maxrss as u64;
    // macOS reports bytes; Linux reports kibibytes.
    Some(if cfg!(target_os = "macos") {
        raw
    } else {
        raw * 1024
    })
}
