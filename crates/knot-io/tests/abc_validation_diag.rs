//! Diagnostic: which validation rule trips on imported ABC models?
//! Per-model breakdown of error codes so we know which checks are
//! triggering bad_input rejections.

use std::path::PathBuf;
use knot_io::from_step;

fn find_step_files(max: usize) -> Vec<PathBuf> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("data").join("abc");
    if !base.exists() {
        return Vec::new();
    }
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
fn diagnose_validation_failures() {
    let files = find_step_files(100);
    if files.is_empty() {
        eprintln!("No STEP files in data/abc/");
        return;
    }

    let mut imported = 0;
    let mut by_code: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut details: Vec<(String, String)> = Vec::new();

    for path in files.iter().take(60) {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let brep = match from_step(&content) {
            Ok(b) => b,
            Err(_) => continue,
        };
        imported += 1;

        match brep.validate() {
            Ok(()) => { *by_code.entry("OK".into()).or_insert(0) += 1; }
            Err(e) => {
                let s = e.to_string();
                let code = if s.contains("EulerViolation") || s.contains("Euler") {
                    "EulerViolation"
                } else if s.contains("LoopNotClosed") || s.contains("loop not closed") {
                    "LoopNotClosed"
                } else if s.contains("DanglingReference") || s.contains("curve start") || s.contains("curve end") {
                    "DanglingReference"
                } else if s.contains("NonManifoldEdge") || s.contains("non-manifold") {
                    "NonManifoldEdge"
                } else {
                    "Other"
                };
                *by_code.entry(code.into()).or_insert(0) += 1;
                if details.len() < 10 {
                    let name = path.file_name().unwrap().to_string_lossy().to_string();
                    details.push((name, s.chars().take(180).collect()));
                }
            }
        }
    }

    eprintln!("\nImported: {}", imported);
    eprintln!("Validation results:");
    let mut entries: Vec<_> = by_code.iter().collect();
    entries.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    for (k, c) in &entries {
        eprintln!("  {:20} {}", k, c);
    }
    eprintln!("\nFirst {} failure details:", details.len());
    for (n, d) in &details {
        eprintln!("  {}: {}", n, d);
    }
}
