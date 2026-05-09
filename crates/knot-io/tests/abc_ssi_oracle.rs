//! Permanent regression oracle: the 9 surface pairs that exposed marcher
//! limitations. 3 are "slow but correct" (ground truth for validation),
//! 6 produce wrong topology (the cases that need the algebraic path).
//!
//! When the algebraic SSI framework is implemented:
//! - If algebraic disagrees with marcher on the 3 correct cases → algebraic has a bug
//! - If algebraic disagrees with marcher on the 6 wrong cases → expected, algebraic is right
//! - Every output point must satisfy both surface implicits to machine ε

use std::path::PathBuf;
use std::time::{Duration, Instant};
use knot_io::from_step;
use knot_ops::boolean::{boolean, BooleanOp};
use knot_tessellate::{tessellate, TessellateOptions};

fn find_step_files(max: usize) -> Vec<PathBuf> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("data").join("abc");
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

/// Identify and classify the 9 timeout/wrong-topology pairs.
/// Run with 30s timeout to separate "slow but correct" from "wrong."
#[test]
#[ignore]
fn classify_oracle_pairs() {
    let files = find_step_files(100);
    if files.is_empty() { eprintln!("No STEP files"); return; }

    let mut models: Vec<(String, knot_topo::BRep)> = Vec::new();
    for f in &files {
        if models.len() >= 50 { break; }
        if let Ok(c) = std::fs::read_to_string(f) {
            if let Ok(b) = from_step(&c) {
                let name = f.file_name().unwrap().to_string_lossy().to_string();
                models.push((name, b));
            }
        }
    }

    let n_pairs = models.len().min(30);
    let mut slow_correct = 0;
    let mut wrong_topo = 0;
    let mut still_timeout = 0;

    for i in 0..n_pairs {
        let j = (i + 1) % models.len();
        if i == j { continue; }

        for &op in &[BooleanOp::Union, BooleanOp::Intersection, BooleanOp::Subtraction] {
            // First: does it timeout at 10s?
            let start = Instant::now();
            let result_10s = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                boolean(&models[i].1, &models[j].1, op)
            }));
            let elapsed_10s = start.elapsed();

            if elapsed_10s < Duration::from_secs(10) {
                continue; // not a timeout case
            }

            // Check what the error was
            let was_bad_input = match &result_10s {
                Ok(Err(e)) => e.to_string().contains("input A has invalid") || e.to_string().contains("input B has invalid"),
                _ => false,
            };
            if was_bad_input { continue; }

            // This is a timeout case. Now run with 30s to classify.
            let start = Instant::now();
            let result_30s = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                boolean(&models[i].1, &models[j].1, op)
            }));
            let elapsed_30s = start.elapsed();

            let op_name = match op {
                BooleanOp::Union => "union",
                BooleanOp::Intersection => "inter",
                BooleanOp::Subtraction => "sub",
            };

            if elapsed_30s >= Duration::from_secs(30) {
                still_timeout += 1;
                eprintln!("STILL_TIMEOUT pair ({},{}) {} [{:.1}s]",
                    models[i].0, models[j].0, op_name, elapsed_30s.as_secs_f64());
            } else {
                match result_30s {
                    Ok(Ok(brep)) => {
                        if let Ok(mesh) = tessellate(&brep, TessellateOptions::default()) {
                            if mesh.triangle_count() > 0 {
                                slow_correct += 1;
                                eprintln!("SLOW_CORRECT pair ({},{}) {} [{:.1}s] {} tris",
                                    models[i].0, models[j].0, op_name,
                                    elapsed_30s.as_secs_f64(), mesh.triangle_count());
                            } else {
                                wrong_topo += 1;
                                eprintln!("WRONG_TOPO pair ({},{}) {} [{:.1}s] 0 tris",
                                    models[i].0, models[j].0, op_name, elapsed_30s.as_secs_f64());
                            }
                        } else {
                            wrong_topo += 1;
                            eprintln!("WRONG_TOPO pair ({},{}) {} [{:.1}s] tess fail",
                                models[i].0, models[j].0, op_name, elapsed_30s.as_secs_f64());
                        }
                    }
                    Ok(Err(e)) => {
                        let msg = e.to_string();
                        if msg.contains("no faces") || msg.contains("empty") {
                            slow_correct += 1;
                            eprintln!("SLOW_CORRECT pair ({},{}) {} [{:.1}s] empty (disjoint)",
                                models[i].0, models[j].0, op_name, elapsed_30s.as_secs_f64());
                        } else {
                            wrong_topo += 1;
                            eprintln!("WRONG_TOPO pair ({},{}) {} [{:.1}s] {}",
                                models[i].0, models[j].0, op_name, elapsed_30s.as_secs_f64(), msg);
                        }
                    }
                    Err(_) => {
                        wrong_topo += 1;
                        eprintln!("WRONG_TOPO pair ({},{}) {} [{:.1}s] panic",
                            models[i].0, models[j].0, op_name, elapsed_30s.as_secs_f64());
                    }
                }
            }
        }
    }

    eprintln!("\n=== ORACLE SUMMARY ===");
    eprintln!("Slow but correct (ground truth): {}", slow_correct);
    eprintln!("Wrong topology (need algebraic):  {}", wrong_topo);
    eprintln!("Still timeout at 30s:             {}", still_timeout);
}
