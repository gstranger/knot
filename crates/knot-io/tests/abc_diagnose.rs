//! Diagnostic harness: classifies ABC boolean failures by root cause.

use std::path::PathBuf;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use knot_io::from_step;
use knot_ops::boolean::{boolean, BooleanOp};
use knot_intersect::surface_surface::intersect_surfaces;
use knot_geom::surface::Surface;

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
        else if p.extension().map_or(false, |e| {
            let e = e.to_string_lossy().to_lowercase();
            e == "step" || e == "stp"
        }) { files.push(p); }
    }
}

#[derive(Debug, Default)]
struct FailureCounts {
    missing_ssi: usize,       // overlapping faces but SSI found nothing
    ssi_timeout: usize,       // SSI took >2s on a single pair
    no_split: usize,          // SSI found traces but no faces were split
    bad_classification: usize, // split happened but all faces classified same way
    euler_violation: usize,    // result has odd Euler characteristic
    non_manifold: usize,       // result has edge used >2 times
    empty_result: usize,       // no faces selected (might be correct for disjoint)
    other: usize,
}

#[test]
#[ignore]
fn diagnose_abc_failures() {
    let files = find_step_files(100);
    if files.is_empty() {
        eprintln!("No STEP files. Run: ./scripts/fetch_abc_chunk.sh 0");
        return;
    }

    // Import models
    let mut models: Vec<knot_topo::BRep> = Vec::new();
    for f in &files {
        if models.len() >= 50 { break; }
        if let Ok(content) = std::fs::read_to_string(f) {
            if let Ok(brep) = from_step(&content) { models.push(brep); }
        }
    }
    eprintln!("Loaded {} models", models.len());

    let mut counts = FailureCounts::default();
    let mut total = 0usize;
    let mut success = 0usize;
    let mut surface_pair_fails: HashMap<String, usize> = HashMap::new();

    let timeout = Duration::from_secs(10);
    let n_pairs = models.len().min(30);

    for i in 0..n_pairs {
        let j = (i + 1) % models.len();
        if i == j { continue; }

        for &op in &[BooleanOp::Union, BooleanOp::Intersection, BooleanOp::Subtraction] {
            total += 1;
            let start = Instant::now();

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                boolean(&models[i], &models[j], op)
            }));

            if start.elapsed() > timeout {
                counts.ssi_timeout += 1;
                continue;
            }

            match result {
                Err(_) => { counts.other += 1; }
                Ok(Ok(brep)) => {
                    if let Ok(mesh) = knot_tessellate::tessellate(&brep, knot_tessellate::TessellateOptions::default()) {
                        if mesh.triangle_count() > 0 { success += 1; continue; }
                    }
                    counts.other += 1;
                }
                Ok(Err(e)) => {
                    let msg = e.to_string();
                    if msg.contains("no faces") || msg.contains("empty") {
                        // Dig deeper: is this a true disjoint or a missed intersection?
                        let diag = diagnose_empty(&models[i], &models[j]);
                        match diag {
                            EmptyReason::TrulyDisjoint => { success += 1; } // correct
                            EmptyReason::MissedSSI => { counts.missing_ssi += 1; }
                            EmptyReason::AllSameClassification => { counts.bad_classification += 1; }
                        }
                    } else if msg.contains("NonManifold") || msg.contains("non-manifold") {
                        counts.non_manifold += 1;
                        // Check what surface types are involved
                        record_surface_types(&models[i], &models[j], &mut surface_pair_fails);
                    } else if msg.contains("Euler") {
                        counts.euler_violation += 1;
                        record_surface_types(&models[i], &models[j], &mut surface_pair_fails);
                    } else {
                        counts.other += 1;
                    }
                }
            }
        }
    }

    eprintln!("\n╔══════════════════════════════════════════════╗");
    eprintln!("║    FAILURE DIAGNOSIS                         ║");
    eprintln!("╠══════════════════════════════════════════════╣");
    eprintln!("║  Total ops:           {:>4}                   ║", total);
    eprintln!("║  Success:             {:>4}                   ║", success);
    eprintln!("║                                              ║");
    eprintln!("║  Missing SSI:         {:>4}  (surfaces overlap ║", counts.missing_ssi);
    eprintln!("║                             but no trace)     ║");
    eprintln!("║  SSI timeout:         {:>4}  (>10s total)      ║", counts.ssi_timeout);
    eprintln!("║  Bad classification:  {:>4}  (all faces same   ║", counts.bad_classification);
    eprintln!("║                             side)             ║");
    eprintln!("║  Non-manifold edge:   {:>4}                   ║", counts.non_manifold);
    eprintln!("║  Euler violation:     {:>4}                   ║", counts.euler_violation);
    eprintln!("║  Empty result:        {:>4}                   ║", counts.empty_result);
    eprintln!("║  Other:               {:>4}                   ║", counts.other);
    eprintln!("╚══════════════════════════════════════════════╝");

    if !surface_pair_fails.is_empty() {
        eprintln!("\nSurface types in failing pairs:");
        let mut pairs: Vec<_> = surface_pair_fails.iter().collect();
        pairs.sort_by(|a, b| b.1.cmp(a.1));
        for (k, v) in pairs.iter().take(10) {
            eprintln!("  {:>4}x  {}", v, k);
        }
    }
}

enum EmptyReason {
    TrulyDisjoint,
    MissedSSI,
    AllSameClassification,
}

fn diagnose_empty(a: &knot_topo::BRep, b: &knot_topo::BRep) -> EmptyReason {
    let solid_a = a.single_solid().unwrap();
    let solid_b = b.single_solid().unwrap();

    // Check if bounding boxes overlap at all
    let bbox_a = brep_bbox(solid_a);
    let bbox_b = brep_bbox(solid_b);
    if !bbox_a.intersects(&bbox_b) {
        return EmptyReason::TrulyDisjoint;
    }

    // Bboxes overlap — check if SSI finds any traces
    let tolerance = 1e-6;
    let mut any_trace = false;
    let mut pair_count = 0;

    'outer: for fa in solid_a.outer_shell().faces() {
        for fb in solid_b.outer_shell().faces() {
            pair_count += 1;
            if pair_count > 50 { break 'outer; } // limit for speed
            if let Ok(traces) = intersect_surfaces(fa.surface(), fb.surface(), tolerance) {
                if traces.iter().any(|t| t.points.len() >= 2) {
                    any_trace = true;
                    break 'outer;
                }
            }
        }
    }

    if !any_trace && bbox_a.intersects(&bbox_b) {
        // Bboxes overlap but no SSI traces — either truly disjoint
        // (bbox overlap doesn't mean surface overlap) or missed intersection
        // Heuristic: check if centroids are inside each other
        EmptyReason::MissedSSI
    } else if any_trace {
        EmptyReason::AllSameClassification
    } else {
        EmptyReason::TrulyDisjoint
    }
}

fn brep_bbox(solid: &knot_topo::Solid) -> knot_core::Aabb3 {
    let pts: Vec<knot_geom::Point3> = solid.outer_shell().faces().iter()
        .flat_map(|f| f.outer_loop().half_edges().iter().map(|he| *he.start_vertex().point()))
        .collect();
    knot_core::Aabb3::from_points(&pts).unwrap()
}

fn record_surface_types(a: &knot_topo::BRep, b: &knot_topo::BRep, map: &mut HashMap<String, usize>) {
    let mut types = Vec::new();
    for solid in [a, b] {
        for face in solid.solids()[0].outer_shell().faces() {
            let t = match face.surface().as_ref() {
                Surface::Plane(_) => "Plane",
                Surface::Sphere(_) => "Sphere",
                Surface::Cylinder(_) => "Cylinder",
                Surface::Cone(_) => "Cone",
                Surface::Torus(_) => "Torus",
                Surface::Nurbs(_) => "NURBS",
            };
            if !types.contains(&t) { types.push(t); }
        }
    }
    types.sort();
    let key = types.join("+");
    *map.entry(key).or_insert(0) += 1;
}
