//! Per-pair failure diagnostic. Prints the model names + op for
//! each pair that doesn't succeed (excluding bad_input rejections,
//! which are upstream model issues we already know about).

use std::path::PathBuf;
use std::time::{Duration, Instant};
use knot_io::from_step;
use knot_ops::boolean::{boolean, BooleanOp};
use knot_tessellate::{tessellate, TessellateOptions};

fn find_step_files(max: usize) -> Vec<PathBuf> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("data").join("abc");
    if !base.exists() { return Vec::new(); }
    let mut files = Vec::new();
    walk(&base, max, &mut files);
    files
}
fn walk(dir: &PathBuf, max: usize, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        if files.len() >= max { return; }
        let p = e.path();
        if p.is_dir() { walk(&p, max, files); }
        else if p.extension().map_or(false, |ext| {
            let ext = ext.to_string_lossy().to_lowercase();
            ext == "step" || ext == "stp"
        }) { files.push(p); }
    }
}

#[test]
#[ignore]
fn list_failing_pairs() {
    let files = find_step_files(100);
    if files.is_empty() {
        eprintln!("No STEP files in data/abc/");
        return;
    }

    let mut models: Vec<(String, knot_topo::BRep)> = Vec::new();
    for path in &files {
        if models.len() >= 50 { break; }
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(brep) = from_step(&content) {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                models.push((name, brep));
            }
        }
    }
    eprintln!("Loaded {} models", models.len());

    let n_pairs = models.len().min(30);
    let timeout = Duration::from_secs(10);

    for i in 0..n_pairs {
        let j = (i + 1) % models.len();
        if i == j { continue; }
        for &op in &[BooleanOp::Union, BooleanOp::Intersection, BooleanOp::Subtraction] {
            let op_name = match op {
                BooleanOp::Union => "U",
                BooleanOp::Intersection => "I",
                BooleanOp::Subtraction => "S",
            };
            let start = Instant::now();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                boolean(&models[i].1, &models[j].1, op)
            }));
            let elapsed = start.elapsed();

            let outcome = if elapsed > timeout {
                "TIMEOUT".to_string()
            } else {
                match result {
                    Err(_) => "CRASH".to_string(),
                    Ok(Err(e)) => {
                        let msg = e.to_string();
                        if msg.contains("input A has invalid") || msg.contains("input B has invalid") {
                            // skip — already-known import issues
                            continue;
                        }
                        if msg.contains("no faces") || msg.contains("empty") || msg.contains("Empty") {
                            continue; // empty result is correct
                        }
                        format!("TOPO_FAIL: {}", msg.chars().take(120).collect::<String>())
                    }
                    Ok(Ok(brep)) => {
                        match tessellate(&brep, TessellateOptions::default()) {
                            Ok(m) if m.triangle_count() > 0 => continue,
                            Ok(_) => "TESS_EMPTY".to_string(),
                            Err(e) => format!("TESS_FAIL: {}", e.to_string().chars().take(80).collect::<String>()),
                        }
                    }
                }
            };
            eprintln!(
                "[{:>4}ms] {} {} {} -> {}",
                elapsed.as_millis(),
                op_name,
                models[i].0.chars().take(48).collect::<String>(),
                models[j].0.chars().take(48).collect::<String>(),
                outcome
            );
        }
    }
}
