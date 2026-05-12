//! ABC dataset boolean reliability harness.
//!
//! Loads STEP files from data/abc/NNNN/, imports them, runs pairwise booleans,
//! and reports the success rate.
//!
//! Run:
//!   1. ./scripts/fetch_abc_chunk.sh 0
//!   2. cargo test -p knot-io --test abc_harness -- --nocapture --ignored
//!
//! The test is #[ignore]d by default so it doesn't run in regular CI
//! (requires downloaded data).

use std::path::PathBuf;
use std::time::{Duration, Instant};
use knot_io::from_step;
use knot_ops::boolean::{boolean, BooleanOp};
use knot_tessellate::{tessellate, TessellateOptions};

/// Find STEP files in data/abc/ recursively.
///
/// Filesystem iteration order is platform-dependent, which would
/// make the harness non-deterministic across runs (different model
/// pairs sampled, different success counts). Sort by filename so a
/// given corpus produces a fixed sequence and reliability numbers
/// are comparable run-to-run.
fn find_step_files(max_files: usize) -> Vec<PathBuf> {
    let abc_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("data").join("abc");

    if !abc_root.exists() {
        return Vec::new();
    }

    // Honor `KNOT_ABC_CHUNK=NNNN` to run the harness against a
    // specific chunk directory. Without it we walk every chunk
    // present (default historical behavior). Setting the env var
    // is the only safe way to compare reliability across chunks
    // when multiple are extracted on disk.
    let base = match std::env::var("KNOT_ABC_CHUNK") {
        Ok(chunk) => {
            let chunk_dir = abc_root.join(&chunk);
            if !chunk_dir.exists() {
                eprintln!("KNOT_ABC_CHUNK={} not found under {}", chunk, abc_root.display());
                return Vec::new();
            }
            eprintln!("ABC chunk: {}", chunk);
            chunk_dir
        }
        Err(_) => abc_root,
    };

    let mut files = Vec::new();
    walk_dir(&base, usize::MAX, &mut files);
    files.sort();
    files.truncate(max_files);
    files
}

fn walk_dir(dir: &PathBuf, max: usize, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if files.len() >= max { return; }
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, max, files);
        } else if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            if ext == "step" || ext == "stp" {
                files.push(path);
            }
        }
    }
}

#[derive(Default, Debug)]
struct ImportReport {
    total: usize,
    success: usize,
    parse_fail: usize,
    import_fail: usize,
    total_faces: usize,
    total_time_ms: u128,
}

#[derive(Default, Debug)]
struct BooleanReport {
    total: usize,
    valid: usize,
    empty: usize,
    bad_input: usize,
    topo_fail: usize,
    tess_fail: usize,
    crash: usize,
    timeout: usize,
    total_time_ms: u128,
}

impl BooleanReport {
    fn success_rate(&self) -> f64 {
        // Success = valid + correctly-empty + correctly-rejected bad input
        let ok = self.valid + self.empty + self.bad_input;
        if self.total == 0 { return 0.0; }
        ok as f64 / self.total as f64 * 100.0
    }
}

/// Try to import a STEP file. Returns the BRep and face count on success.
fn try_import(path: &PathBuf) -> Result<(knot_topo::BRep, usize), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("read: {}", e))?;

    let brep = from_step(&content)
        .map_err(|e| format!("import: {}", e))?;

    let faces = brep.solids().iter()
        .map(|s| s.outer_shell().face_count())
        .sum();

    Ok((brep, faces))
}

fn run_boolean_timed(
    a: &knot_topo::BRep,
    b: &knot_topo::BRep,
    op: BooleanOp,
    timeout: Duration,
) -> (&'static str, Duration) {
    use std::sync::mpsc;

    let start = Instant::now();
    // Watchdog timeout: spawn the boolean on a worker thread and
    // wait on a channel. If the worker doesn't respond within
    // `timeout`, we declare a timeout and proceed; the worker thread
    // is orphaned (it will run to completion and drop its result, or
    // be cleaned up at process exit). Without this watchdog a true
    // infinite loop in the boolean wedges the whole harness — the
    // post-hoc elapsed-time check below was never sufficient.
    let (tx, rx) = mpsc::channel();
    let a_clone = a.clone();
    let b_clone = b.clone();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            boolean(&a_clone, &b_clone, op)
        }));
        // It's fine if the receiver is gone — we've already moved on.
        let _ = tx.send(result);
    });

    let result = match rx.recv_timeout(timeout) {
        Ok(r) => r,
        Err(mpsc::RecvTimeoutError::Timeout) => return ("timeout", start.elapsed()),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            return ("crash", start.elapsed());
        }
    };
    let elapsed = start.elapsed();

    match result {
        Err(_) => ("crash", elapsed),
        Ok(Err(e)) => {
            let msg = e.to_string();
            if msg.contains("no faces") || msg.contains("empty") || msg.contains("Empty") {
                ("empty", elapsed)
            } else if msg.contains("input A has invalid") || msg.contains("input B has invalid") {
                ("bad_input", elapsed)
            } else if msg.contains("E404") || msg.contains("budget exceeded") {
                ("timeout", elapsed)
            } else {
                ("topo_fail", elapsed)
            }
        }
        Ok(Ok(brep)) => {
            match tessellate(&brep, TessellateOptions::default()) {
                Ok(m) if m.triangle_count() > 0 => ("valid", elapsed),
                Ok(_) => ("tess_fail", elapsed),
                Err(_) => ("tess_fail", elapsed),
            }
        }
    }
}

/// Import test: load N STEP files and report success rate.
#[test]
#[ignore] // requires data/abc/ — run with --ignored
fn abc_import_report() {
    let files = find_step_files(200);
    if files.is_empty() {
        eprintln!("No STEP files found in data/abc/. Run: ./scripts/fetch_abc_chunk.sh 0");
        return;
    }

    let mut report = ImportReport::default();

    for path in &files {
        report.total += 1;
        let start = Instant::now();

        match try_import(path) {
            Ok((_, faces)) => {
                report.success += 1;
                report.total_faces += faces;
            }
            Err(e) => {
                if e.starts_with("import:") {
                    report.import_fail += 1;
                } else {
                    report.parse_fail += 1;
                }
                if report.total <= 20 {
                    eprintln!("  FAIL {}: {}", path.file_name().unwrap().to_string_lossy(), e);
                }
            }
        }

        report.total_time_ms += start.elapsed().as_millis();
    }

    eprintln!("\n╔══════════════════════════════════════════╗");
    eprintln!("║    ABC IMPORT REPORT                     ║");
    eprintln!("╠══════════════════════════════════════════╣");
    eprintln!("║  Files tested:      {:>6}               ║", report.total);
    eprintln!("║  Imported OK:       {:>6}               ║", report.success);
    eprintln!("║  Parse failures:    {:>6}               ║", report.parse_fail);
    eprintln!("║  Import failures:   {:>6}               ║", report.import_fail);
    eprintln!("║  Total faces:       {:>6}               ║", report.total_faces);
    eprintln!("║  Avg time/file:     {:>5}ms              ║",
        if report.total > 0 { report.total_time_ms / report.total as u128 } else { 0 });
    eprintln!("║                                          ║");
    eprintln!("║  IMPORT RATE:       {:>5.1}%              ║",
        if report.total > 0 { report.success as f64 / report.total as f64 * 100.0 } else { 0.0 });
    eprintln!("╚══════════════════════════════════════════╝");
}

/// Boolean reliability test on ABC models.
/// Loads N models, runs pairwise booleans on random pairs.
#[test]
#[ignore] // requires data/abc/ — run with --ignored
fn abc_boolean_reliability() {
    let files = find_step_files(100);
    if files.is_empty() {
        eprintln!("No STEP files found in data/abc/. Run: ./scripts/fetch_abc_chunk.sh 0");
        return;
    }

    // Import all files we can
    eprintln!("Importing {} STEP files...", files.len());
    let mut models: Vec<(String, knot_topo::BRep)> = Vec::new();
    for path in &files {
        if let Ok((brep, _)) = try_import(path) {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            models.push((name, brep));
        }
        if models.len() >= 50 { break; } // cap at 50 models for speed
    }

    eprintln!("Imported {} models successfully", models.len());
    if models.len() < 2 {
        eprintln!("Need at least 2 importable models for boolean testing");
        return;
    }

    // Run pairwise booleans on random pairs
    let mut report = BooleanReport::default();
    let n_pairs = models.len().min(30); // test up to 30 pairs
    let timeout = Duration::from_secs(10);

    // Deterministic pair selection
    let mut pair_idx = 0u64;
    for i in 0..n_pairs {
        let j = (i + 1) % models.len();
        if i == j { continue; }

        for &op in &[BooleanOp::Union, BooleanOp::Intersection, BooleanOp::Subtraction] {
            report.total += 1;

            let (outcome, elapsed) = run_boolean_timed(&models[i].1, &models[j].1, op, timeout);
            report.total_time_ms += elapsed.as_millis();

            match outcome {
                "valid" => report.valid += 1,
                "empty" => report.empty += 1,
                "bad_input" => report.bad_input += 1,
                "topo_fail" => {
                    report.topo_fail += 1;
                    let op_name = match op {
                        BooleanOp::Union => "U",
                        BooleanOp::Intersection => "I",
                        BooleanOp::Subtraction => "S",
                    };
                    // Recompute the error message for diagnostic.
                    let detail = match boolean(&models[i].1, &models[j].1, op) {
                        Err(e) => e.to_string(),
                        _ => String::from("(no error on retry)"),
                    };
                    eprintln!(
                        "  TOPO_FAIL [{:>4}ms] {} {} {} — {}",
                        elapsed.as_millis(), op_name,
                        models[i].0.chars().take(40).collect::<String>(),
                        models[j].0.chars().take(40).collect::<String>(),
                        detail.chars().take(180).collect::<String>(),
                    );
                }
                "tess_fail" => report.tess_fail += 1,
                "crash" => report.crash += 1,
                "timeout" => {
                    report.timeout += 1;
                    let op_name = match op {
                        BooleanOp::Union => "U",
                        BooleanOp::Intersection => "I",
                        BooleanOp::Subtraction => "S",
                    };
                    eprintln!(
                        "  TIMEOUT [{:>4}ms] {} {} {}",
                        elapsed.as_millis(), op_name,
                        models[i].0.chars().take(40).collect::<String>(),
                        models[j].0.chars().take(40).collect::<String>()
                    );
                }
                _ => {}
            }
        }
    }

    eprintln!("\n╔══════════════════════════════════════════╗");
    eprintln!("║    ABC BOOLEAN RELIABILITY               ║");
    eprintln!("╠══════════════════════════════════════════╣");
    eprintln!("║  Models loaded:     {:>6}               ║", models.len());
    eprintln!("║  Boolean ops:       {:>6}               ║", report.total);
    eprintln!("║  Valid results:     {:>6}               ║", report.valid);
    eprintln!("║  Empty (correct):   {:>6}               ║", report.empty);
    eprintln!("║  Bad input (reject):{:>6}               ║", report.bad_input);
    eprintln!("║  Topology fail:     {:>6}               ║", report.topo_fail);
    eprintln!("║  Tess fail:         {:>6}               ║", report.tess_fail);
    eprintln!("║  Crashes:           {:>6}               ║", report.crash);
    eprintln!("║  Timeouts:          {:>6}               ║", report.timeout);
    eprintln!("║  Avg time/op:       {:>5}ms              ║",
        if report.total > 0 { report.total_time_ms / report.total as u128 } else { 0 });
    eprintln!("║                                          ║");
    eprintln!("║  SUCCESS RATE:      {:>5.1}%              ║", report.success_rate());
    eprintln!("╚══════════════════════════════════════════╝");

    assert_eq!(report.crash, 0, "no crashes allowed on ABC models");
}
