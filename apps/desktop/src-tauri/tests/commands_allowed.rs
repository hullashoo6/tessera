//! Every command the app registers must also be declared in build.rs and
//! allowed in capabilities/default.json, and nothing else may be allowed.
//! A command missing from either list fails only when someone presses its
//! button ("... not allowed. Command not found"), so check it here.

use std::collections::BTreeSet;

fn read(rel: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The names inside `tauri::generate_handler![...]`, without module paths.
fn registered() -> BTreeSet<String> {
    let src = read("src/lib.rs");
    let start = src.find("generate_handler![").expect("handler list") + "generate_handler![".len();
    let end = start + src[start..].find(']').expect("end of handler list");
    src[start..end]
        .split(',')
        .map(|s| s.trim().rsplit("::").next().unwrap_or("").to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn declared() -> BTreeSet<String> {
    let src = read("build.rs");
    let start = src.find("COMMANDS: &[&str] = &[").expect("COMMANDS");
    let end = start + src[start..].find("];").expect("end of COMMANDS");
    src[start..end]
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

fn allowed() -> BTreeSet<String> {
    let json: serde_json::Value = serde_json::from_str(&read("capabilities/default.json")).unwrap();
    json["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p.as_str())
        .filter_map(|p| p.strip_prefix("allow-"))
        .map(|p| p.replace('-', "_"))
        .collect()
}

#[test]
fn every_registered_command_is_declared_and_allowed() {
    let registered = registered();
    assert!(registered.len() > 30, "handler list parsed: {registered:?}");
    let declared = declared();
    let allowed = allowed();
    let missing_decl: Vec<_> = registered.difference(&declared).collect();
    let missing_allow: Vec<_> = registered.difference(&allowed).collect();
    let extra: Vec<_> = allowed.difference(&registered).collect();
    assert!(
        missing_decl.is_empty(),
        "add to build.rs COMMANDS: {missing_decl:?}"
    );
    assert!(
        missing_allow.is_empty(),
        "add to capabilities/default.json: {missing_allow:?}"
    );
    assert!(extra.is_empty(), "allowed but not registered: {extra:?}");
}
