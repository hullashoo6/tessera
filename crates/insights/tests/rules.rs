//! Smart Scan rules on a disposable tree. Each rule must need its evidence.

use std::fs;
use std::time::{Duration, SystemTime};
use tessera_insights::{Group, Places, Safety, analyse};
use tessera_scan::{CancelToken, ScanOptions, scan};
use tessera_testkit as kit;

const MB2: usize = 2 << 20;

#[test]
fn rules_fire_only_with_evidence() {
    let t = kit::tempfile::Builder::new()
        .prefix("tessera-insights-")
        .tempdir()
        .unwrap();
    let r = t.path().join("root");
    let home = r.join("home");
    let lad = home.join("AppData").join("Local");
    let tmp = r.join("tmp");

    // Evidence present.
    kit::file(&home, "projects/web/package.json", 10).unwrap();
    kit::dense_file(&home, "projects/web/node_modules/dep/index.js", MB2).unwrap();
    kit::file(&home, "projects/web/node_modules/dep/package.json", 10).unwrap();
    kit::dense_file(
        &home,
        "projects/web/node_modules/dep/node_modules/inner.js",
        MB2,
    )
    .unwrap();
    kit::file(&home, "projects/rs/Cargo.toml", 10).unwrap();
    kit::file(&home, "projects/rs/target/CACHEDIR.TAG", 10).unwrap();
    kit::dense_file(&home, "projects/rs/target/debug/app.exe", MB2).unwrap();
    kit::file(&home, "projects/py/.venv/pyvenv.cfg", 10).unwrap();
    kit::dense_file(&home, "projects/py/.venv/lib/big.so", MB2).unwrap();
    kit::dense_file(&home, ".cache/huggingface/model.bin", MB2).unwrap();
    kit::dense_file(&lad, "Docker/wsl/disk.vhdx", MB2).unwrap();
    kit::dense_file(&lad, "Google/Chrome/User Data/Default/Cache/data_1", MB2).unwrap();
    let old = kit::dense_file(&tmp, "old.tmp", MB2).unwrap();
    kit::dense_file(&tmp, "new.tmp", MB2).unwrap();
    fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(10 * 86_400))
        .unwrap();

    // Evidence missing: must not be suggested.
    kit::dense_file(&home, "projects/fake/node_modules/x.js", MB2).unwrap();
    kit::dense_file(&home, "Documents/cache/photos.zip", MB2).unwrap();
    kit::dense_file(&home, "OneDrive/Pictures/trip.jpg", MB2).unwrap();
    kit::file(&home, "projects/notrust/target/debug/x", 10).unwrap();

    let (tree, _) = scan(
        &r,
        &ScanOptions::default(),
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap();
    let places = Places {
        home: Some(home.clone()),
        temp: Some(tmp.clone()),
        local_app_data: Some(lad.clone()),
        cloud_roots: vec![home.join("OneDrive")],
        system_root: None,
    };
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let found = analyse(&tree, &places, now);
    let paths: Vec<String> = found
        .iter()
        .map(|s| {
            tree.path_of(s.id)
                .strip_prefix(&r)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    let has = |p: &str| paths.iter().any(|x| x == p);

    assert!(has("home/projects/web/node_modules"), "{paths:?}");
    assert!(
        !has("home/projects/web/node_modules/dep/node_modules"),
        "nested node_modules counted once"
    );
    assert!(has("home/projects/rs/target"));
    assert!(has("home/projects/py/.venv"));
    assert!(has("home/.cache/huggingface"));
    assert!(has("home/AppData/Local/Docker/wsl"));
    assert!(has(
        "home/AppData/Local/Google/Chrome/User Data/Default/Cache"
    ));
    assert!(has("tmp/old.tmp"));
    assert!(!has("tmp/new.tmp"), "recent temporary files are left alone");
    assert!(
        !has("home/projects/fake/node_modules"),
        "no package.json beside it"
    );
    assert!(
        !paths.iter().any(|p| p.contains("Documents/cache")),
        "a folder named cache is not evidence"
    );
    assert!(!has("home/projects/notrust/target"));

    let docker = found
        .iter()
        .find(|s| s.label == "Docker disk image")
        .unwrap();
    assert_eq!(docker.safety, Safety::UseAppTool);
    let venv = found
        .iter()
        .find(|s| s.label == "Python virtual environment")
        .unwrap();
    assert_eq!(
        venv.safety,
        Safety::Review,
        "recreating a venv needs the project's requirements"
    );
    let cloud = found
        .iter()
        .find(|s| s.group == Group::Cloud)
        .expect("downloaded cloud file suggested");
    assert_eq!(cloud.safety, Safety::StaysInCloud);
    let hf = found.iter().find(|s| s.group == Group::Downloads).unwrap();
    assert_eq!(hf.safety, Safety::Review);
    assert!(
        found
            .iter()
            .all(|s| !s.reason.is_empty() && s.bytes >= tessera_insights::MIN_BYTES)
    );
    assert!(
        found.windows(2).all(|w| w[0].bytes >= w[1].bytes),
        "largest first"
    );
}

fn set_age(path: &std::path::Path, days: u64) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(days * 86_400))
        .unwrap();
}

/// Both systems' locations are checked on every system (the rules only look
/// at the scanned tree), so Windows and Mac get the same suggestions.
#[test]
fn newer_rules_cover_both_systems_and_need_evidence() {
    let t = kit::tempfile::Builder::new()
        .prefix("tessera-insights-")
        .tempdir()
        .unwrap();
    let r = t.path().join("root");
    let home = r.join("home");
    let lad = home.join("AppData").join("Local");
    let sys = r.join("sys");
    let f = |base: &std::path::Path, rel: &str| kit::dense_file(base, rel, MB2).unwrap();

    // Browser caches: Mac Chrome (two places) and Firefox on both systems.
    f(&home, "Library/Caches/Google/Chrome/Default/Cache/data_1");
    f(
        &home,
        "Library/Application Support/Google/Chrome/Default/GPUCache/data_1",
    );
    f(
        &home,
        "Library/Application Support/Google/Chrome/Default/Bookmarks.big",
    );
    f(
        &lad,
        "Mozilla/Firefox/Profiles/abc.default/cache2/entries/e1",
    );
    f(
        &home,
        "Library/Caches/Firefox/Profiles/xyz.default/cache2/entries/e1",
    );
    // Package caches and developer tools.
    f(&home, "Library/Caches/pip/http/a");
    f(&lad, "Android/Sdk/system-images/android-34/img");
    f(&home, ".android/avd/Pixel.avd/userdata.img");
    f(&home, "Library/Developer/CoreSimulator/Devices/ABC/data");
    // Unity and Unreal projects, with and without evidence.
    kit::file(&home, "games/unity/Assets/scene.unity", 10).unwrap();
    kit::file(&home, "games/unity/ProjectSettings/p.asset", 10).unwrap();
    f(&home, "games/unity/Library/ArtifactDB");
    f(&home, "notes/Library/book.pdf");
    kit::file(&home, "games/ue/Game.uproject", 10).unwrap();
    f(&home, "games/ue/DerivedDataCache/ddc.bin");
    f(&home, "games/plain/Intermediate/x.bin");
    // Operating-system leftovers.
    f(&sys, "Windows.old/Windows/x");
    f(
        &sys,
        "Applications/Install macOS Sequoia.app/Contents/SharedSupport/x",
    );
    f(&sys, "Applications/Install Other.app/Contents/x");
    // Installers in Downloads: only old ones.
    set_age(&f(&home, "Downloads/setup.exe"), 40);
    set_age(&f(&home, "Downloads/tool.dmg"), 40);
    f(&home, "Downloads/new.msi");
    set_age(&f(&home, "Downloads/photo.jpg"), 40);
    // Backups and recordings.
    f(&home, "Apple/MobileSync/Backup/00008030-AAAA/Manifest.db");
    f(
        &home,
        "Library/Application Support/MobileSync/Backup/00008030-BBBB/Manifest.db",
    );
    f(&home, "Videos/Captures/game.mp4");
    f(&home, "Desktop/Screen Recording 2026-01-01 at 10.00.00.mov");
    f(&home, "Desktop/holiday.mov");

    let (tree, _) = scan(
        &r,
        &ScanOptions::default(),
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap();
    let places = Places {
        home: Some(home.clone()),
        temp: None,
        local_app_data: Some(lad.clone()),
        cloud_roots: Vec::new(),
        system_root: Some(sys.clone()),
    };
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let found = analyse(&tree, &places, now);
    let rel = |id| {
        tree.path_of(id)
            .strip_prefix(&r)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    };
    let get = |p: &str| found.iter().find(|s| rel(s.id) == p);
    let paths: Vec<String> = found.iter().map(|s| rel(s.id)).collect();
    let expect = |p: &str, safety: Safety| {
        let s = get(p).unwrap_or_else(|| panic!("{p} not suggested: {paths:?}"));
        assert_eq!(s.safety, safety, "{p}");
        assert!(!s.reason.is_empty());
    };

    expect(
        "home/Library/Caches/Google/Chrome/Default/Cache",
        Safety::Regenerable,
    );
    expect(
        "home/Library/Application Support/Google/Chrome/Default/GPUCache",
        Safety::Regenerable,
    );
    expect(
        "home/AppData/Local/Mozilla/Firefox/Profiles/abc.default/cache2",
        Safety::Regenerable,
    );
    expect(
        "home/Library/Caches/Firefox/Profiles/xyz.default/cache2",
        Safety::Regenerable,
    );
    expect("home/Library/Caches/pip", Safety::Redownloadable);
    expect(
        "home/AppData/Local/Android/Sdk/system-images",
        Safety::Redownloadable,
    );
    expect("home/.android/avd/Pixel.avd", Safety::UseAppTool);
    expect(
        "home/Library/Developer/CoreSimulator/Devices",
        Safety::UseAppTool,
    );
    expect("home/games/unity/Library", Safety::Regenerable);
    expect("home/games/ue/DerivedDataCache", Safety::Regenerable);
    expect("sys/Windows.old", Safety::UseAppTool);
    expect(
        "sys/Applications/Install macOS Sequoia.app",
        Safety::Redownloadable,
    );
    expect("home/Downloads/setup.exe", Safety::Review);
    expect("home/Downloads/tool.dmg", Safety::Review);
    expect("home/Apple/MobileSync/Backup/00008030-AAAA", Safety::Review);
    expect(
        "home/Library/Application Support/MobileSync/Backup/00008030-BBBB",
        Safety::Review,
    );
    expect("home/Videos/Captures/game.mp4", Safety::Review);
    expect(
        "home/Desktop/Screen Recording 2026-01-01 at 10.00.00.mov",
        Safety::Review,
    );

    for p in [
        "home/Library/Application Support/Google/Chrome/Default/Bookmarks.big",
        "home/notes/Library",
        "home/games/plain/Intermediate",
        "sys/Applications/Install Other.app",
        "home/Downloads/new.msi",
        "home/Downloads/photo.jpg",
        "home/Desktop/holiday.mov",
    ] {
        assert!(get(p).is_none(), "{p} must not be suggested");
    }
    assert!(
        found
            .iter()
            .filter(|s| s.group == Group::Backups)
            .all(|s| s.safety == Safety::Review),
        "backups and recordings are never added automatically"
    );
}
