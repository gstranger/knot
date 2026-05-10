//! Stage-level boolean tracing on the pathological pairs. Sets
//! KNOT_BOOLEAN_TRACE so the boolean op prints per-stage timings and
//! intermediate counts. Tells us where the 8s budget actually goes
//! per pair — informs whether the next fix is in SSI, split-face,
//! classify, or somewhere else entirely.

use std::path::PathBuf;
use std::time::Duration;
use std::sync::mpsc;
use knot_io::from_step;
use knot_ops::boolean::{boolean, BooleanOp};

fn pair_paths() -> Vec<(PathBuf, PathBuf, &'static str)> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("data").join("abc");
    vec![
        (
            base.join("0000/00000011/00000011_e909f412cda24521865fac0f_step_000.step"),
            base.join("0000/00000012/00000012_f16882934f314832b639ffc0_step_000.step"),
            "(11, 12)",
        ),
        (
            base.join("0000/00000032/00000032_ad34a3f60c4a4caa99646600_step_012.step"),
            base.join("0000/00000033/00000033_ad34a3f60c4a4caa99646600_step_013.step"),
            "(32, 33)",
        ),
        (
            base.join("0000/00000024/00000024_ad34a3f60c4a4caa99646600_step_004.step"),
            base.join("0000/00000025/00000025_ad34a3f60c4a4caa99646600_step_005.step"),
            "(24, 25)",
        ),
    ]
}

fn run_with_timeout<F, R>(timeout: Duration, f: F) -> Option<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(timeout).ok()
}

#[test]
#[ignore]
fn pathological_stage_trace() {
    // Set the env var so the boolean prints per-stage timings.
    // SAFETY: tests don't manipulate the environment elsewhere and
    // we run sequentially within the test process.
    unsafe { std::env::set_var("KNOT_BOOLEAN_TRACE", "1"); }

    for (path_a, path_b, label) in pair_paths() {
        if !path_a.exists() || !path_b.exists() {
            eprintln!("skip {}: file missing", label);
            continue;
        }
        let a_content = std::fs::read_to_string(&path_a).unwrap();
        let b_content = std::fs::read_to_string(&path_b).unwrap();
        let a_brep = match from_step(&a_content) { Ok(b) => b, _ => continue };
        let b_brep = match from_step(&b_content) { Ok(b) => b, _ => continue };

        // Just run Union — same SSI work as Intersection / Subtraction.
        eprintln!("\n========== {} Union ==========", label);
        let a_clone = a_brep.clone();
        let b_clone = b_brep.clone();
        let result = run_with_timeout(Duration::from_secs(15), move || {
            boolean(&a_clone, &b_clone, BooleanOp::Union)
        });
        match result {
            None => eprintln!("  outer watchdog timeout"),
            Some(Ok(_)) => eprintln!("  OK"),
            Some(Err(e)) => eprintln!("  Err: {}",
                e.to_string().chars().take(120).collect::<String>()),
        }
    }
}
