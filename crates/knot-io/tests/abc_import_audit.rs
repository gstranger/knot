//! Larger-scale STEP-import audit. Distinguishes:
//!
//! - **Parse failures**: the reader returned an Err. Likely an
//!   unsupported entity type or malformed source. Captures the error
//!   message so we can see which types come up most often.
//! - **Validation failures**: the reader returned a BRep but it
//!   doesn't satisfy the topology contract. Categorized by error
//!   code (LoopNotClosed, DanglingReference, NonManifoldEdge,
//!   EulerViolation).
//! - **Validate-OK**: import + validation both clean.
//!
//! Sample size is 200 files by default — enough for stable error-
//! class proportions, fast enough to finish in well under a minute.

use std::path::PathBuf;
use knot_io::from_step;

fn find_step_files(max: usize) -> Vec<PathBuf> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("data").join("abc");
    if !base.exists() { return Vec::new(); }
    let mut files = Vec::new();
    walk(&base, max, &mut files);
    files.sort(); // deterministic ordering
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
fn audit_import_at_scale() {
    let sample_size: usize = std::env::var("ABC_SAMPLE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);

    let files = find_step_files(sample_size);
    if files.is_empty() {
        eprintln!("No STEP files in data/abc/");
        return;
    }

    let mut total = 0usize;
    let mut parse_fail = 0usize;
    let mut validate_ok = 0usize;
    let mut validate_fail_by_code: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();

    // For parse failures, group by the first ~120 chars of the error
    // message to see which patterns dominate.
    let mut parse_errs_by_kind: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut parse_err_examples: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();

    // For Euler-violation, capture the V/E/F values to see if there's
    // a pattern (e.g., off-by-1 vs widely off).
    let mut euler_offsets: Vec<i64> = Vec::new();

    // For NonManifoldEdge, capture how many uses (4? more?).
    let mut non_manifold_uses: Vec<usize> = Vec::new();

    for path in &files {
        total += 1;
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        match from_step(&content) {
            Err(e) => {
                parse_fail += 1;
                let s = e.to_string();
                let kind = parse_error_kind(&s);
                *parse_errs_by_kind.entry(kind.clone()).or_insert(0) += 1;
                parse_err_examples.entry(kind).or_insert(
                    s.chars().take(160).collect(),
                );
            }
            Ok(brep) => {
                match brep.validate() {
                    Ok(()) => validate_ok += 1,
                    Err(e) => {
                        let s = e.to_string();
                        let code = if s.contains("E204") || s.contains("Euler") {
                            // Try to parse "V-E+F = N" out of the message
                            if let Some(off) = parse_euler_offset(&s) {
                                euler_offsets.push(off);
                            }
                            "EulerViolation".to_string()
                        } else if s.contains("E205") || s.contains("loop not closed") {
                            "LoopNotClosed".to_string()
                        } else if s.contains("E203") || s.contains("curve start") || s.contains("curve end") {
                            "DanglingReference".to_string()
                        } else if s.contains("E201") || s.contains("non-manifold") {
                            if let Some(uses) = parse_non_manifold_uses(&s) {
                                non_manifold_uses.push(uses);
                            }
                            "NonManifoldEdge".to_string()
                        } else {
                            "Other".to_string()
                        };
                        *validate_fail_by_code.entry(code).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    eprintln!("\n══════════════════════════════════════════════════════");
    eprintln!("STEP IMPORT AUDIT — sample size {}", total);
    eprintln!("══════════════════════════════════════════════════════");
    eprintln!("  Parse OK   : {} ({:.1}%)", total - parse_fail, pct(total - parse_fail, total));
    eprintln!("  Parse FAIL : {} ({:.1}%)", parse_fail, pct(parse_fail, total));
    eprintln!("");
    eprintln!("  Of files that parsed:");
    eprintln!("    Validate OK   : {} ({:.1}%)", validate_ok, pct(validate_ok, total - parse_fail));
    let validate_fail_total: usize = validate_fail_by_code.values().sum();
    eprintln!("    Validate FAIL : {} ({:.1}%)", validate_fail_total, pct(validate_fail_total, total - parse_fail));
    for (code, n) in &validate_fail_by_code {
        eprintln!("      {:20}: {}", code, n);
    }

    if !parse_errs_by_kind.is_empty() {
        eprintln!("\n  Parse-failure breakdown:");
        let mut kinds: Vec<_> = parse_errs_by_kind.iter().collect();
        kinds.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        for (k, c) in &kinds {
            eprintln!("    {:40} {} ({:.1}% of parse fails)",
                k, c, pct(**c, parse_fail));
            if let Some(ex) = parse_err_examples.get(*k) {
                eprintln!("        e.g.: {}", ex);
            }
        }
    }

    if !euler_offsets.is_empty() {
        let off1 = euler_offsets.iter().filter(|&&o| o.abs() == 1).count();
        let off3 = euler_offsets.iter().filter(|&&o| o.abs() == 3).count();
        let off_more = euler_offsets.iter().filter(|&&o| o.abs() > 3).count();
        eprintln!("\n  EulerViolation offsets from 0:");
        eprintln!("    |off| = 1     : {} (single missing/extra element)", off1);
        eprintln!("    |off| = 3     : {} (small genus mismatch)", off3);
        eprintln!("    |off| > 3     : {} (significant defect)", off_more);
        eprintln!("    range         : [{}, {}]",
            euler_offsets.iter().min().copied().unwrap_or(0),
            euler_offsets.iter().max().copied().unwrap_or(0));
    }

    if !non_manifold_uses.is_empty() {
        let use4 = non_manifold_uses.iter().filter(|&&u| u == 4).count();
        let use_more = non_manifold_uses.iter().filter(|&&u| u > 4).count();
        eprintln!("\n  NonManifoldEdge use counts:");
        eprintln!("    used 4 times  : {} (T-junction or thin-shell case)", use4);
        eprintln!("    used >4 times : {} (more pathological)", use_more);
    }

    eprintln!("══════════════════════════════════════════════════════");
}

fn parse_error_kind(s: &str) -> String {
    // Extract a short categorization from the error message.
    if s.contains("unsupported curve type") {
        // Try to capture the type name
        if let Some(start) = s.find("unsupported curve type ") {
            let rest = &s[start + "unsupported curve type ".len()..];
            let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            return format!("unsupported curve: {}", name);
        }
        return "unsupported curve type".to_string();
    }
    if s.contains("unsupported surface type") {
        if let Some(start) = s.find("unsupported surface type ") {
            let rest = &s[start + "unsupported surface type ".len()..];
            let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            return format!("unsupported surface: {}", name);
        }
        return "unsupported surface type".to_string();
    }
    if s.contains("dropped") && s.contains("faces") { return "shell: dropped faces".to_string(); }
    if s.contains("dropped") && s.contains("oriented edges") { return "edge loop: dropped edges".to_string(); }
    if s.contains("no MANIFOLD_SOLID_BREP") { return "no MANIFOLD_SOLID_BREP".to_string(); }
    if s.contains("only") && s.contains("faces read") { return "shell: too few faces".to_string(); }
    if s.contains("CONICAL_SURFACE missing") { return "CONICAL_SURFACE: missing data".to_string(); }
    if s.contains("STEP parse error") { return "STEP grammar parse error".to_string(); }
    if s.contains("missing") { return "missing entity reference".to_string(); }
    "other".to_string()
}

fn parse_euler_offset(s: &str) -> Option<i64> {
    // Looks for "V-E+F = N"
    let key = "V-E+F = ";
    let i = s.find(key)?;
    let rest = &s[i + key.len()..];
    let num: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
    num.parse().ok()
}

fn parse_non_manifold_uses(s: &str) -> Option<usize> {
    // Looks for "edge used N times"
    let key = "edge used ";
    let i = s.find(key)?;
    let rest = &s[i + key.len()..];
    let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    num.parse().ok()
}

fn pct(num: usize, denom: usize) -> f64 {
    if denom == 0 { 0.0 } else { 100.0 * num as f64 / denom as f64 }
}
