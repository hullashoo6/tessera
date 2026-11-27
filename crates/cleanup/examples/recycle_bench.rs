//! Measures how long moving many small items to the Recycle Bin / Trash
//! takes, one at a time as cleanup does. Uses disposable files in the temp
//! folder and removes them from the Recycle Bin again afterwards.
//!   cargo run --release -p tessera-cleanup --example recycle_bench -- 200
fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);
    let dir = std::env::temp_dir().join(format!("tessera-recycle-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let files: Vec<_> = (0..n)
        .map(|i| {
            let p = dir.join(format!("f{i}.bin"));
            std::fs::write(&p, vec![0u8; 1024]).unwrap();
            p
        })
        .collect();
    let batch: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let t = std::time::Instant::now();
    let mut landed = Vec::new();
    let mut failed = 0;
    for chunk in files.chunks(batch.max(1)) {
        let refs: Vec<&std::path::Path> = chunk.iter().map(|p| p.as_path()).collect();
        let results = if batch <= 1 {
            refs.iter()
                .map(|p| tessera_cleanup::move_to_trash(p))
                .collect()
        } else {
            tessera_cleanup::move_many_to_trash(&refs)
        };
        for r in results {
            match r {
                Ok(Some(l)) => landed.push(l),
                Ok(None) => {}
                Err(e) => {
                    failed += 1;
                    eprintln!("{e}");
                }
            }
        }
    }
    let el = t.elapsed();
    println!("batch size {batch}");
    println!(
        "{n} files moved in {:.2} s ({:.1} ms each), {failed} failed",
        el.as_secs_f64(),
        el.as_secs_f64() * 1000.0 / n as f64
    );
    // Remove the disposable files from the Recycle Bin again.
    for l in &landed {
        let _ = std::fs::remove_file(l);
        if let (Some(d), Some(name)) = (l.parent(), l.file_name().and_then(|n| n.to_str()))
            && let Some(rest) = name.strip_prefix("$R")
        {
            let _ = std::fs::remove_file(d.join(format!("$I{rest}")));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    println!("{} removed from the Recycle Bin again", landed.len());
}
