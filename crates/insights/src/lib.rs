//! Smart Scan: deterministic, explainable storage suggestions.
//!
//! Every suggestion states what an item is, why it was flagged and how safe
//! it is to remove. A folder is never flagged just because of its name:
//! name rules also require evidence (a marker file next to it, or a known
//! location). Nothing here removes anything; suggestions feed the normal
//! review-first cleanup, which applies its own protections again.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tessera_scan::query::find_by_path;
use tessera_scan::{NodeId, NodeKind, Tree, flags};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    /// Temporary files, caches and crash dumps.
    Junk,
    /// Developer dependencies, build output and package caches.
    Developer,
    /// Large re-downloadable data such as AI models and disk images.
    Downloads,
    /// Files kept in a cloud service that also take space on this computer.
    Cloud,
    /// The person's own backups and recordings: worth a look, never automatic.
    Backups,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Safety {
    /// Rebuilt automatically by the tool that made it.
    Regenerable,
    /// Downloaded again when next needed.
    Redownloadable,
    /// May contain things the person wants; look before removing.
    Review,
    /// Managed by an app; use that app's own clean-up instead of moving files.
    UseAppTool,
    /// Stays in the cloud; only the copy on this computer is removed.
    StaysInCloud,
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub id: NodeId,
    pub group: Group,
    pub label: &'static str,
    pub reason: String,
    pub safety: Safety,
    /// Bytes on disk counted for the item in the scan.
    pub bytes: u64,
}

/// Known folders of the person running the app.
#[derive(Debug, Clone, Default)]
pub struct Places {
    pub home: Option<PathBuf>,
    pub temp: Option<PathBuf>,
    pub local_app_data: Option<PathBuf>,
    /// Cloud sync folders (OneDrive, iCloud Drive).
    pub cloud_roots: Vec<PathBuf>,
    /// The drive the system runs from: `C:\` (or wherever Windows is) or `/`.
    pub system_root: Option<PathBuf>,
}

impl Places {
    pub fn for_this_user() -> Self {
        let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
        let system_root = if cfg!(windows) {
            std::env::var("SystemDrive")
                .ok()
                .map(|d| PathBuf::from(format!("{}\\", d.trim_end_matches('\\'))))
        } else {
            Some(PathBuf::from("/"))
        };
        Places {
            home: env("USERPROFILE").or_else(|| env("HOME")),
            temp: Some(std::env::temp_dir()),
            local_app_data: env("LOCALAPPDATA"),
            cloud_roots: Vec::new(),
            system_root,
        }
    }
}

/// Suggestions smaller than this are not shown.
pub const MIN_BYTES: u64 = 1 << 20;
const DAY: i64 = 86_400;

fn name(tree: &Tree, id: NodeId) -> String {
    tree.name(id).to_string_lossy().to_lowercase()
}

fn has_child(tree: &Tree, dir: NodeId, child: &str) -> bool {
    tree.children(dir).any(|c| name(tree, c) == child)
}

fn sibling(tree: &Tree, id: NodeId, file: &str) -> bool {
    let p = tree.node(id).parent;
    p != tessera_scan::NO_NODE && has_child(tree, p, file)
}

/// A file beside `id` whose name ends with `suffix` (lower case).
fn sibling_ending(tree: &Tree, id: NodeId, suffix: &str) -> bool {
    let p = tree.node(id).parent;
    p != tessera_scan::NO_NODE
        && tree
            .children(p)
            .any(|c| tree.node(c).kind == NodeKind::File && name(tree, c).ends_with(suffix))
}

/// Folders beside `id` with all of these names (lower case).
fn sibling_dirs(tree: &Tree, id: NodeId, dirs: &[&str]) -> bool {
    let p = tree.node(id).parent;
    p != tessera_scan::NO_NODE
        && dirs.iter().all(|d| {
            tree.children(p)
                .any(|c| tree.node(c).kind == NodeKind::Dir && name(tree, c) == *d)
        })
}

/// Installers people download and rarely need again once the app is in.
const INSTALLER_EXTENSIONS: [&str; 8] = [
    ".exe",
    ".msi",
    ".msix",
    ".msixbundle",
    ".appx",
    ".appxbundle",
    ".dmg",
    ".pkg",
];

struct Builder<'a> {
    tree: &'a Tree,
    out: Vec<Suggestion>,
    taken: HashSet<NodeId>,
}

impl Builder<'_> {
    fn add(
        &mut self,
        id: NodeId,
        group: Group,
        label: &'static str,
        safety: Safety,
        reason: String,
    ) {
        let n = self.tree.node(id);
        if n.has(flags::REMOVED)
            || self.taken.contains(&id)
            || self.tree.ancestors(id).any(|a| self.taken.contains(&a))
        {
            return;
        }
        let bytes = n.counted().1;
        if bytes < MIN_BYTES {
            return;
        }
        self.taken.insert(id);
        self.out.push(Suggestion {
            id,
            group,
            label,
            reason,
            safety,
            bytes,
        });
    }

    fn at(
        &mut self,
        path: Option<PathBuf>,
        group: Group,
        label: &'static str,
        safety: Safety,
        reason: &str,
    ) {
        if let Some(id) = path.and_then(|p| find_by_path(self.tree, &p))
            && self.tree.node(id).kind == NodeKind::Dir
        {
            self.add(id, group, label, safety, reason.to_string());
        }
    }

    fn children_at(&self, path: Option<PathBuf>) -> Vec<NodeId> {
        path.and_then(|p| find_by_path(self.tree, &p))
            .filter(|&d| self.tree.node(d).kind == NodeKind::Dir)
            .map(|d| self.tree.children(d).collect())
            .unwrap_or_default()
    }

    /// A browser's cache folders inside each of its profiles under `base`.
    fn profile_caches(&mut self, base: Option<PathBuf>, browser: &str, caches: &[&str]) {
        for profile in self.children_at(base) {
            let found: Vec<NodeId> = self
                .tree
                .children(profile)
                .filter(|&c| {
                    caches
                        .iter()
                        .any(|n| self.tree.name(c).to_string_lossy() == *n)
                })
                .collect();
            for c in found {
                self.add(c, Group::Junk, "Browser cache", Safety::Regenerable,
                    format!("{browser} keeps copies of websites here to load them faster; it rebuilds them. Close {browser} first."));
            }
        }
    }

    /// Each folder directly inside `dir`, as its own suggestion.
    fn each_folder_in(
        &mut self,
        dir: Option<PathBuf>,
        group: Group,
        label: &'static str,
        safety: Safety,
        reason: &str,
    ) {
        for c in self.children_at(dir) {
            if self.tree.node(c).kind == NodeKind::Dir {
                self.add(c, group, label, safety, reason.to_string());
            }
        }
    }

    /// Files directly inside `dir` that `keep` accepts, as separate suggestions.
    fn files_in(
        &mut self,
        dir: Option<PathBuf>,
        keep: impl Fn(&str, i64) -> bool,
        group: Group,
        label: &'static str,
        safety: Safety,
        reason: &str,
    ) {
        for c in self.children_at(dir) {
            let n = self.tree.node(c);
            if n.kind == NodeKind::File && keep(&name(self.tree, c), n.modified) {
                self.add(c, group, label, safety, reason.to_string());
            }
        }
    }

    /// Each item directly inside `dir` older than `min_age_days`.
    fn contents_older_than(
        &mut self,
        dir: Option<PathBuf>,
        days: i64,
        now: i64,
        group: Group,
        label: &'static str,
        reason: &str,
    ) {
        let Some(id) = dir.and_then(|p| find_by_path(self.tree, &p)) else {
            return;
        };
        let kids: Vec<NodeId> = self.tree.children(id).collect();
        for c in kids {
            let m = self.tree.node(c).modified;
            if m != i64::MIN
                && now - m >= days * DAY
                && matches!(self.tree.node(c).kind, NodeKind::File | NodeKind::Dir)
            {
                self.add(c, group, label, Safety::Regenerable, reason.to_string());
            }
        }
    }
}

fn join(base: &Option<PathBuf>, rel: &str) -> Option<PathBuf> {
    base.as_ref()
        .map(|b| rel.split('/').fold(b.clone(), |p, part| p.join(part)))
}

/// Finds suggestions in a finished scan. `now` is Unix seconds.
pub fn analyse(tree: &Tree, places: &Places, now: i64) -> Vec<Suggestion> {
    let mut b = Builder {
        tree,
        out: Vec::new(),
        taken: HashSet::new(),
    };
    let home = &places.home;
    let lad = &places.local_app_data;

    // Managed by an app: never move these files directly.
    b.at(join(lad, "Docker/wsl"), Group::Developer, "Docker disk image", Safety::UseAppTool,
        "Docker Desktop's virtual disk. Free space with Docker's own clean-up (for example `docker system prune`), not by moving the file.");
    b.at(
        join(home, "Library/Containers/com.docker.docker"),
        Group::Developer,
        "Docker disk image",
        Safety::UseAppTool,
        "Docker Desktop's data. Use Docker's own clean-up instead of moving these files.",
    );

    // Known caches in the person's own folders.
    for (rel, label, why) in [
        (
            "AppData/Local/npm-cache",
            "npm package cache",
            "Copies of downloaded npm packages; npm downloads them again when needed.",
        ),
        (
            ".npm/_cacache",
            "npm package cache",
            "Copies of downloaded npm packages; npm downloads them again when needed.",
        ),
        (
            "AppData/Local/pip/Cache",
            "pip package cache",
            "Downloaded Python packages; pip fetches them again when needed.",
        ),
        (
            "Library/Caches/pip",
            "pip package cache",
            "Downloaded Python packages; pip fetches them again when needed.",
        ),
        (
            "Library/Caches/Yarn",
            "Yarn package cache",
            "Downloaded JavaScript packages; Yarn fetches them again when needed.",
        ),
        (
            "AppData/Local/Unity/cache",
            "Unity package cache",
            "Packages Unity downloaded; it downloads them again when a project needs them.",
        ),
        (
            "Library/Unity/cache",
            "Unity package cache",
            "Packages Unity downloaded; it downloads them again when a project needs them.",
        ),
        (
            "AppData/Local/Android/Sdk/system-images",
            "Android emulator images",
            "System images for Android emulators; Android Studio's SDK Manager downloads them again if needed.",
        ),
        (
            "Library/Android/sdk/system-images",
            "Android emulator images",
            "System images for Android emulators; Android Studio's SDK Manager downloads them again if needed.",
        ),
        (
            "Library/Developer/Xcode/iOS DeviceSupport",
            "Xcode device support files",
            "Debug files copied from iPhones and iPads; Xcode copies them again when a device is connected.",
        ),
        (
            ".cache/pip",
            "pip package cache",
            "Downloaded Python packages; pip fetches them again when needed.",
        ),
        (
            "AppData/Local/Yarn/Cache",
            "Yarn package cache",
            "Downloaded JavaScript packages; Yarn fetches them again when needed.",
        ),
        (
            ".gradle/caches",
            "Gradle cache",
            "Downloaded build dependencies; Gradle fetches them again on the next build.",
        ),
        (
            ".m2/repository",
            "Maven repository",
            "Downloaded Java dependencies; Maven fetches them again when needed.",
        ),
        (
            ".nuget/packages",
            "NuGet package cache",
            "Downloaded .NET packages; restored again when a project builds.",
        ),
        (
            ".cargo/registry",
            "Cargo registry cache",
            "Downloaded Rust crates; Cargo fetches them again when needed.",
        ),
        (
            "Library/Developer/Xcode/DerivedData",
            "Xcode DerivedData",
            "Xcode build output; Xcode rebuilds it.",
        ),
    ] {
        b.at(
            join(home, rel),
            Group::Developer,
            label,
            Safety::Redownloadable,
            why,
        );
    }
    for (rel, label) in [
        (".ollama/models", "Ollama AI models"),
        (".cache/huggingface", "Hugging Face models and datasets"),
        (".lmstudio/models", "LM Studio AI models"),
    ] {
        b.at(join(home, rel), Group::Downloads, label, Safety::Review,
            "Downloaded AI models are large and can be downloaded again, but only remove models you no longer use.");
    }
    for dumps in [
        join(lad, "CrashDumps"),
        join(home, "Library/Logs/DiagnosticReports"),
    ] {
        b.at(
            dumps,
            Group::Junk,
            "Crash reports",
            Safety::Regenerable,
            "Reports saved when apps crashed. Only useful when reporting a crash to a developer.",
        );
    }

    // Browser caches, in each browser profile. Windows keeps them in one
    // folder per browser; macOS splits them between Caches and Application
    // Support.
    let chromium = ["Cache", "Code Cache", "GPUCache"];
    for (browser, rel) in [
        ("Chrome", "Google/Chrome/User Data"),
        ("Edge", "Microsoft/Edge/User Data"),
        ("Brave", "BraveSoftware/Brave-Browser/User Data"),
    ] {
        b.profile_caches(join(lad, rel), browser, &chromium);
    }
    for (browser, rel) in [
        ("Chrome", "Google/Chrome"),
        ("Edge", "Microsoft Edge"),
        ("Brave", "BraveSoftware/Brave-Browser"),
    ] {
        b.profile_caches(
            join(home, &format!("Library/Caches/{rel}")),
            browser,
            &chromium,
        );
        b.profile_caches(
            join(home, &format!("Library/Application Support/{rel}")),
            browser,
            &chromium,
        );
    }
    b.profile_caches(
        join(lad, "Mozilla/Firefox/Profiles"),
        "Firefox",
        &["cache2"],
    );
    b.profile_caches(
        join(home, "Library/Caches/Firefox/Profiles"),
        "Firefox",
        &["cache2"],
    );

    // Developer tools that manage their own data.
    b.each_folder_in(join(home, ".android/avd"), Group::Developer, "Android virtual device", Safety::UseAppTool,
        "An Android emulator with its saved data. Remove devices you no longer use in Android Studio's Device Manager, so its list stays correct.");
    b.at(join(home, "Library/Developer/CoreSimulator/Devices"), Group::Developer, "iOS simulators", Safety::UseAppTool,
        "Simulated iPhones and iPads with their data. Remove unused ones in Xcode (Window > Devices and Simulators) or with `xcrun simctl delete unavailable`.");
    b.at(join(home, "Library/Developer/Xcode/Archives"), Group::Developer, "Xcode archives", Safety::Review,
        "Builds made with Xcode's Archive command. Keep the ones for app versions you may still need to investigate.");

    // Leftovers from installing or upgrading the operating system.
    let sys = &places.system_root;
    b.at(join(sys, "Windows.old"), Group::Junk, "Previous Windows installation", Safety::UseAppTool,
        "Your previous version of Windows, kept after an upgrade so you can go back. Remove it with Settings > System > Storage > Temporary files > Previous Windows installation(s).");
    b.at(join(sys, "Windows/SoftwareDistribution/Download"), Group::Junk, "Windows Update downloads", Safety::UseAppTool,
        "Update files Windows downloaded. Windows clears them itself; to do it now, use Settings > System > Storage > Temporary files > Windows Update Cleanup.");
    for app in b.children_at(join(sys, "Applications")) {
        let n = name(tree, app);
        if tree.node(app).kind == NodeKind::Dir
            && n.starts_with("install macos")
            && n.ends_with(".app")
        {
            b.add(app, Group::Downloads, "macOS installer", Safety::Redownloadable,
                "An installer for macOS from the App Store. After upgrading it is no longer needed, and it can be downloaded again.".into());
        }
    }

    // Installers left in Downloads for over a month.
    b.files_in(
        join(home, "Downloads"),
        |n, modified| {
            INSTALLER_EXTENSIONS.iter().any(|e| n.ends_with(e))
                && modified != i64::MIN
                && now - modified >= 30 * DAY
        },
        Group::Downloads,
        "Old installer",
        Safety::Review,
        "An installer downloaded over a month ago. Once an app is installed its installer is usually not needed, and it can be downloaded again from the publisher.",
    );

    // The person's own backups and recordings.
    for backups in [
        join(home, "AppData/Roaming/Apple Computer/MobileSync/Backup"),
        join(home, "Apple/MobileSync/Backup"),
        join(home, "Library/Application Support/MobileSync/Backup"),
    ] {
        b.each_folder_in(backups, Group::Backups, "iPhone or iPad backup", Safety::Review,
            "A backup of an iPhone or iPad. If it is for a device you no longer use, it can usually go. You can also manage backups in Finder on a Mac, or in Apple Devices or iTunes on Windows.");
    }
    let recording = "A screen recording. Keep it if you still need it; recordings are often large.";
    for dir in [
        join(home, "Videos/Captures"),
        join(home, "Videos/Screen Recordings"),
    ] {
        b.files_in(
            dir,
            |_, _| true,
            Group::Backups,
            "Screen recording",
            Safety::Review,
            recording,
        );
    }
    for dir in [join(home, "Desktop"), join(home, "Movies")] {
        b.files_in(
            dir,
            |n, _| n.starts_with("screen recording") && n.ends_with(".mov"),
            Group::Backups,
            "Screen recording",
            Safety::Review,
            recording,
        );
    }

    // Temporary files untouched for a week.
    b.contents_older_than(places.temp.clone(), 7, now, Group::Junk, "Temporary files",
        "Left in the temporary folder by apps and not changed for over a week. Items still in use will be skipped.");

    // Developer folders found anywhere, each with evidence beside it.
    for i in 1..tree.len() as NodeId {
        let n = tree.node(i);
        if n.kind != NodeKind::Dir || n.has(flags::REMOVED) {
            continue;
        }
        match name(tree, i).as_str() {
            "node_modules" if sibling(tree, i, "package.json") => b.add(i, Group::Developer, "node_modules", Safety::Regenerable,
                "Installed JavaScript packages next to a package.json; `npm install` recreates them.".into()),
            "target" if sibling(tree, i, "cargo.toml") && (has_child(tree, i, "cachedir.tag") || has_child(tree, i, "debug") || has_child(tree, i, "release")) => {
                b.add(i, Group::Developer, "Rust build output", Safety::Regenerable, "Compiled files next to a Cargo.toml; `cargo build` recreates them.".into())
            }
            ".venv" | "venv" | "env" if has_child(tree, i, "pyvenv.cfg") => b.add(i, Group::Developer, "Python virtual environment", Safety::Review,
                "A Python environment (it contains pyvenv.cfg). It can be recreated from the project's requirements, so check those exist first.".into()),
            ".next" | ".nuxt" | ".turbo" | ".parcel-cache" | ".svelte-kit" if sibling(tree, i, "package.json") => b.add(i, Group::Developer, "Web build cache", Safety::Regenerable,
                "Build output of a JavaScript project; rebuilt on the next build.".into()),
            "__pycache__" | ".pytest_cache" | ".mypy_cache" => b.add(i, Group::Developer, "Python cache", Safety::Regenerable,
                "Python's compiled-file and tool caches; recreated automatically.".into()),
            "library" if sibling_dirs(tree, i, &["assets", "projectsettings"]) => b.add(i, Group::Developer, "Unity project cache", Safety::Regenerable,
                "Unity's imported-asset cache for the project beside it (it has Assets and ProjectSettings); Unity rebuilds it when the project is next opened, which can take a while.".into()),
            "deriveddatacache" | "intermediate" if sibling_ending(tree, i, ".uproject") => b.add(i, Group::Developer, "Unreal Engine build cache", Safety::Regenerable,
                "Build and shader cache of the Unreal Engine project beside it; the editor rebuilds it.".into()),
            _ => {}
        }
    }

    // Downloaded cloud files: freeing space keeps them in the cloud.
    for root in &places.cloud_roots {
        let Some(top) = find_by_path(tree, root) else {
            continue;
        };
        let mut stack = vec![top];
        let mut files = Vec::new();
        while let Some(d) = stack.pop() {
            for c in tree.children(d) {
                let n = tree.node(c);
                match n.kind {
                    NodeKind::Dir => stack.push(c),
                    NodeKind::File if !n.has(flags::CLOUD_ONLY) && n.counted().1 >= MIN_BYTES => {
                        files.push(c)
                    }
                    _ => {}
                }
            }
        }
        files.sort_by_key(|&f| std::cmp::Reverse(tree.node(f).counted().1));
        for f in files.into_iter().take(500) {
            b.add(f, Group::Cloud, "Downloaded cloud file", Safety::StaysInCloud,
                "Kept in your cloud storage. Freeing space removes only the copy on this computer; it downloads again when you open it.".into());
        }
    }

    b.out.sort_by_key(|s| std::cmp::Reverse(s.bytes));
    b.out
}

/// Human description of a safety level.
pub fn safety_text(s: Safety) -> &'static str {
    match s {
        Safety::Regenerable => "Recreated automatically",
        Safety::Redownloadable => "Downloaded again when needed",
        Safety::Review => "Look before removing",
        Safety::UseAppTool => "Use the app's own clean-up",
        Safety::StaysInCloud => "Stays in the cloud",
    }
}

/// True if `path` lies inside `base` (both absolute).
pub fn is_inside(path: &Path, base: &Path) -> bool {
    path.starts_with(base)
}
