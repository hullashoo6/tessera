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
}

impl Places {
    pub fn for_this_user() -> Self {
        let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
        Places {
            home: env("USERPROFILE").or_else(|| env("HOME")),
            temp: Some(std::env::temp_dir()),
            local_app_data: env("LOCALAPPDATA"),
        }
    }
}

/// Suggestions smaller than this are not shown.
pub const MIN_BYTES: u64 = 1 << 20;
const DAY: i64 = 86_400;

fn name(tree: &Tree, id: NodeId) -> String {
    tree.node(id).name.to_string_lossy().to_lowercase()
}

fn has_child(tree: &Tree, dir: NodeId, child: &str) -> bool {
    tree.children(dir).any(|c| name(tree, c) == child)
}

fn sibling(tree: &Tree, id: NodeId, file: &str) -> bool {
    let p = tree.node(id).parent;
    p != tessera_scan::NO_NODE && has_child(tree, p, file)
}

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
    b.at(
        join(lad, "CrashDumps"),
        Group::Junk,
        "Crash reports",
        Safety::Regenerable,
        "Memory dumps saved when apps crashed. Only useful when reporting a crash to a developer.",
    );

    // Browser caches (each browser profile).
    for (browser, rel) in [
        ("Chrome", "Google/Chrome/User Data"),
        ("Edge", "Microsoft/Edge/User Data"),
        ("Brave", "BraveSoftware/Brave-Browser/User Data"),
    ] {
        if let Some(ud) = join(lad, rel).and_then(|p| find_by_path(tree, &p)) {
            let profiles: Vec<NodeId> = tree.children(ud).collect();
            for p in profiles {
                for cache in ["Cache", "Code Cache", "GPUCache"] {
                    if let Some(c) = tree
                        .children(p)
                        .find(|&c| tree.node(c).name.to_string_lossy() == cache)
                    {
                        b.add(c, Group::Junk, "Browser cache", Safety::Regenerable,
                            format!("{browser} keeps copies of websites here to load them faster; it rebuilds them. Close {browser} first."));
                    }
                }
            }
        }
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
            ".venv" | "venv" | "env" if has_child(tree, i, "pyvenv.cfg") => b.add(i, Group::Developer, "Python virtual environment", Safety::Regenerable,
                "A Python environment (it contains pyvenv.cfg). It can be recreated from the project's requirements, so check those exist first.".into()),
            ".next" | ".nuxt" | ".turbo" | ".parcel-cache" | ".svelte-kit" if sibling(tree, i, "package.json") => b.add(i, Group::Developer, "Web build cache", Safety::Regenerable,
                "Build output of a JavaScript project; rebuilt on the next build.".into()),
            "__pycache__" | ".pytest_cache" | ".mypy_cache" => b.add(i, Group::Developer, "Python cache", Safety::Regenerable,
                "Python's compiled-file and tool caches; recreated automatically.".into()),
            _ => {}
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
    }
}

/// True if `path` lies inside `base` (both absolute).
pub fn is_inside(path: &Path, base: &Path) -> bool {
    path.starts_with(base)
}
