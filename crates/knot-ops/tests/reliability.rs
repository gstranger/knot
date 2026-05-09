//! Boolean reliability harness.
//!
//! Generates random primitive solid pairs, runs all three boolean operations,
//! classifies outcomes, reports success rate.
//!
//! This is the baseline measurement before shared-edge topology work.
//! The number from this test drives prioritization.

use knot_geom::Point3;
use knot_ops::primitives;
use knot_ops::boolean::{boolean, BooleanOp};
use knot_tessellate::{tessellate, TessellateOptions};

/// Deterministic PRNG (xorshift64) for reproducible test cases.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self { Self(seed) }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Uniform f64 in [lo, hi]
    fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        let t = (self.next() & 0xFFFFFFFF) as f64 / 0xFFFFFFFF_u64 as f64;
        lo + t * (hi - lo)
    }

    /// Random integer in [0, n)
    fn range(&mut self, n: usize) -> usize {
        (self.next() as usize) % n
    }
}

#[derive(Debug)]
enum Outcome {
    Valid,
    EmptyResult,
    InvalidTopology(String),
    TessellationFailed(String),
    Crash(String),
}

fn make_random_solid(rng: &mut Rng) -> knot_topo::BRep {
    let shape = rng.range(3);
    let ox = rng.uniform(-2.0, 2.0);
    let oy = rng.uniform(-2.0, 2.0);
    let oz = rng.uniform(-2.0, 2.0);
    let size = rng.uniform(0.5, 3.0);

    match shape {
        0 => {
            // Box
            let sx = rng.uniform(0.5, size);
            let sy = rng.uniform(0.5, size);
            let sz = rng.uniform(0.5, size);
            make_offset_box(ox, oy, oz, sx, sy, sz)
        }
        1 => {
            // Sphere
            let r = rng.uniform(0.3, size * 0.6);
            let n_lon = 6 + rng.range(10) as u32;
            let n_lat = 3 + rng.range(5) as u32;
            primitives::make_sphere(Point3::new(ox, oy, oz), r, n_lon, n_lat).unwrap()
        }
        _ => {
            // Cylinder
            let r = rng.uniform(0.2, size * 0.4);
            let h = rng.uniform(0.5, size);
            let n = 6 + rng.range(12) as u32;
            primitives::make_cylinder(Point3::new(ox, oy, oz), r, h, n).unwrap()
        }
    }
}

fn run_boolean(a: &knot_topo::BRep, b: &knot_topo::BRep, op: BooleanOp) -> Outcome {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        boolean(a, b, op)
    }));

    match result {
        Err(_) => Outcome::Crash("panic during boolean".into()),
        Ok(Err(e)) => {
            let msg = e.to_string();
            if msg.contains("no faces") || msg.contains("empty") || msg.contains("Empty") {
                Outcome::EmptyResult
            } else {
                Outcome::InvalidTopology(msg)
            }
        }
        Ok(Ok(brep)) => {
            // Try to tessellate — this catches many subtle topology issues
            match tessellate(&brep, TessellateOptions::default()) {
                Ok(mesh) => {
                    if mesh.triangle_count() == 0 {
                        Outcome::TessellationFailed("zero triangles".into())
                    } else {
                        Outcome::Valid
                    }
                }
                Err(e) => Outcome::TessellationFailed(e.to_string()),
            }
        }
    }
}

/// Run the reliability harness with N random solid pairs.
fn reliability_report(n_pairs: usize, seed: u64) -> ReliabilityReport {
    let mut rng = Rng::new(seed);
    let mut report = ReliabilityReport::default();

    for pair_idx in 0..n_pairs {
        let a = make_random_solid(&mut rng);
        let b = make_random_solid(&mut rng);

        for &op in &[BooleanOp::Union, BooleanOp::Intersection, BooleanOp::Subtraction] {
            let outcome = run_boolean(&a, &b, op);
            match &outcome {
                Outcome::Valid => report.valid += 1,
                Outcome::EmptyResult => report.empty += 1,
                Outcome::InvalidTopology(_) => report.invalid_topo += 1,
                Outcome::TessellationFailed(_) => report.tess_failed += 1,
                Outcome::Crash(_) => report.crash += 1,
            }
            report.total += 1;
        }
    }

    report
}

#[derive(Default, Debug)]
struct ReliabilityReport {
    total: usize,
    valid: usize,
    empty: usize,
    invalid_topo: usize,
    tess_failed: usize,
    crash: usize,
}

impl ReliabilityReport {
    fn success_rate(&self) -> f64 {
        // "Success" = valid result OR correctly-reported empty result.
        // Failures = invalid topology, tessellation failure, crash.
        let successes = self.valid + self.empty;
        if self.total == 0 { return 0.0; }
        successes as f64 / self.total as f64 * 100.0
    }

    fn print(&self) {
        eprintln!("╔══════════════════════════════════════════╗");
        eprintln!("║    BOOLEAN RELIABILITY REPORT            ║");
        eprintln!("╠══════════════════════════════════════════╣");
        eprintln!("║  Total operations:  {:>6}               ║", self.total);
        eprintln!("║  Valid results:     {:>6}               ║", self.valid);
        eprintln!("║  Empty (correct):   {:>6}               ║", self.empty);
        eprintln!("║  Invalid topology:  {:>6}               ║", self.invalid_topo);
        eprintln!("║  Tess failed:       {:>6}               ║", self.tess_failed);
        eprintln!("║  Crashes:           {:>6}               ║", self.crash);
        eprintln!("║                                          ║");
        eprintln!("║  SUCCESS RATE:      {:>5.1}%              ║", self.success_rate());
        eprintln!("╚══════════════════════════════════════════╝");
    }
}

/// Main reliability test: 100 random pairs x 3 ops = 300 boolean operations.
#[test]
fn boolean_reliability_100_pairs() {
    let report = reliability_report(100, 42);
    report.print();

    // Assert no crashes — the fail-or-correct contract means we never panic
    assert_eq!(report.crash, 0, "boolean operations must not crash");

    // Report the success rate. This is the baseline number.
    // We don't assert a minimum rate yet — we need the number first.
    eprintln!("Baseline success rate: {:.1}%", report.success_rate());
}

/// Smaller quick test for CI.
#[test]
fn boolean_reliability_20_pairs() {
    let report = reliability_report(20, 123);
    assert_eq!(report.crash, 0, "no crashes allowed");
}

/// Offset box helper (duplicated from boolean.rs tests to keep this self-contained).
fn make_offset_box(ox: f64, oy: f64, oz: f64, sx: f64, sy: f64, sz: f64) -> knot_topo::BRep {
    use std::sync::Arc;
    use knot_geom::Vector3;
    use knot_geom::curve::{Curve, LineSeg};
    use knot_geom::surface::{Surface, Plane};
    use knot_topo::*;

    let hx = sx / 2.0;
    let hy = sy / 2.0;
    let hz = sz / 2.0;

    let v = [
        Arc::new(Vertex::new(Point3::new(ox - hx, oy - hy, oz - hz))),
        Arc::new(Vertex::new(Point3::new(ox + hx, oy - hy, oz - hz))),
        Arc::new(Vertex::new(Point3::new(ox + hx, oy + hy, oz - hz))),
        Arc::new(Vertex::new(Point3::new(ox - hx, oy + hy, oz - hz))),
        Arc::new(Vertex::new(Point3::new(ox - hx, oy - hy, oz + hz))),
        Arc::new(Vertex::new(Point3::new(ox + hx, oy - hy, oz + hz))),
        Arc::new(Vertex::new(Point3::new(ox + hx, oy + hy, oz + hz))),
        Arc::new(Vertex::new(Point3::new(ox - hx, oy + hy, oz + hz))),
    ];

    let make_face = |vi: [usize; 4], origin: Point3, normal: Vector3| -> Face {
        let mut edges = Vec::new();
        for i in 0..4 {
            let j = (i + 1) % 4;
            let start = v[vi[i]].clone();
            let end = v[vi[j]].clone();
            let curve = Arc::new(Curve::Line(LineSeg::new(*start.point(), *end.point())));
            let edge = Arc::new(Edge::new(start, end, curve, 0.0, 1.0));
            edges.push(HalfEdge::new(edge, true));
        }
        let loop_ = Loop::new(edges, true).unwrap();
        let surface = Arc::new(Surface::Plane(Plane::new(origin, normal)));
        Face::new(surface, loop_, vec![], true).unwrap()
    };

    let faces = vec![
        make_face([0, 3, 2, 1], Point3::new(ox, oy, oz - hz), -Vector3::z()),
        make_face([4, 5, 6, 7], Point3::new(ox, oy, oz + hz), Vector3::z()),
        make_face([0, 1, 5, 4], Point3::new(ox, oy - hy, oz), -Vector3::y()),
        make_face([2, 3, 7, 6], Point3::new(ox, oy + hy, oz), Vector3::y()),
        make_face([0, 4, 7, 3], Point3::new(ox - hx, oy, oz), -Vector3::x()),
        make_face([1, 2, 6, 5], Point3::new(ox + hx, oy, oz), Vector3::x()),
    ];

    let shell = Shell::new(faces, true).unwrap();
    let solid = Solid::new(shell, vec![]).unwrap();
    BRep::new(vec![solid]).unwrap()
}
