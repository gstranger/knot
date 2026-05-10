//! Diagnose a single hanging boolean. Run with a 5-second wall-clock
//! limit so we don't lock up the harness, and dump the stage we're in.

use std::path::PathBuf;
use std::time::{Duration, Instant};
use std::sync::mpsc;
use knot_io::from_step;
use knot_ops::boolean::{boolean, BooleanOp};

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
fn diagnose_hang() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("data").join("abc");
    let f1 = base.join("0000/00000011/00000011_e909f412cda24521865fac0f_step_000.step");
    let f2 = base.join("0000/00000012/00000012_f16882934f314832b639ffc0_step_000.step");
    let c1 = std::fs::read_to_string(&f1).unwrap();
    let c2 = std::fs::read_to_string(&f2).unwrap();
    let b1 = from_step(&c1).unwrap();
    let b2 = from_step(&c2).unwrap();

    eprintln!("b1: {} faces", b1.solids()[0].outer_shell().face_count());
    eprintln!("b2: {} faces", b2.solids()[0].outer_shell().face_count());

    // How many distinct surfaces does each model use?
    let unique_a: std::collections::HashSet<usize> = b1.solids()[0]
        .outer_shell().faces().iter()
        .map(|f| std::sync::Arc::as_ptr(f.surface()) as usize).collect();
    let unique_b: std::collections::HashSet<usize> = b2.solids()[0]
        .outer_shell().faces().iter()
        .map(|f| std::sync::Arc::as_ptr(f.surface()) as usize).collect();
    eprintln!("b1: {} distinct surfaces, b2: {} distinct surfaces", unique_a.len(), unique_b.len());

    for &op in &[BooleanOp::Union, BooleanOp::Intersection, BooleanOp::Subtraction] {
        let label = match op {
            BooleanOp::Union => "U",
            BooleanOp::Intersection => "I",
            BooleanOp::Subtraction => "S",
        };
        let b1c = b1.clone();
        let b2c = b2.clone();
        let start = Instant::now();
        let result = run_with_timeout(Duration::from_secs(20), move || {
            boolean(&b1c, &b2c, op)
        });
        let elapsed = start.elapsed();
        match result {
            None => eprintln!("{} timed out at {}ms", label, elapsed.as_millis()),
            Some(Ok(_)) => eprintln!("{} OK in {}ms", label, elapsed.as_millis()),
            Some(Err(e)) => eprintln!("{} Err in {}ms: {}", label, elapsed.as_millis(),
                e.to_string().chars().take(300).collect::<String>()),
        }
    }
}
