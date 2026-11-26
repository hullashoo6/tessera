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
