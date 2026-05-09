//! Diagnose the 7 remaining real topology failures.

use std::path::PathBuf;
use knot_io::from_step;
use knot_ops::boolean::{boolean, BooleanOp};
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
        else if p.extension().map_or(false, |ext| {
            let ext = ext.to_string_lossy().to_lowercase();
            ext == "step" || ext == "stp"
        }) { files.push(p); }
    }
}

fn surface_name(s: &Surface) -> &'static str {
    match s {
        Surface::Plane(_) => "Plane",
        Surface::Sphere(_) => "Sphere",
        Surface::Cylinder(_) => "Cyl",
        Surface::Cone(_) => "Cone",
        Surface::Torus(_) => "Torus",
        Surface::Nurbs(_) => "NURBS",
    }
}

fn op_name(op: BooleanOp) -> &'static str {
    match op { BooleanOp::Union => "union", BooleanOp::Intersection => "inter", BooleanOp::Subtraction => "sub" }
}

#[test]
#[ignore]
fn diagnose_7_topo_failures() {
    let files = find_step_files(100);
    if files.is_empty() { eprintln!("No STEP files"); return; }

    let mut models: Vec<knot_topo::BRep> = Vec::new();
    for f in &files {
        if models.len() >= 50 { break; }
        if let Ok(c) = std::fs::read_to_string(f) {
            if let Ok(b) = from_step(&c) { models.push(b); }
        }
    }
    eprintln!("Loaded {} models", models.len());

    let mut found = 0;
    let n_pairs = models.len().min(30);

    for i in 0..n_pairs {
        let j = (i + 1) % models.len();
        if i == j { continue; }

        for &op in &[BooleanOp::Union, BooleanOp::Intersection, BooleanOp::Subtraction] {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                boolean(&models[i], &models[j], op)
            }));

            let is_topo_fail = match &result {
                Ok(Err(e)) => {
                    let msg = e.to_string();
                    !msg.contains("no faces") && !msg.contains("empty")
                        && !msg.contains("input A has invalid") && !msg.contains("input B has invalid")
                }
                _ => false,
            };

            if !is_topo_fail { continue; }
            found += 1;

            let err = result.unwrap().unwrap_err();
            let sa = models[i].single_solid().unwrap();
            let sb = models[j].single_solid().unwrap();

            // Collect surface types for each model
            let types_a: Vec<&str> = sa.outer_shell().faces().iter()
                .map(|f| surface_name(f.surface())).collect();
            let types_b: Vec<&str> = sb.outer_shell().faces().iter()
                .map(|f| surface_name(f.surface())).collect();

            // Count unique surface types
            let mut uniq_a: Vec<&str> = types_a.clone();
            uniq_a.sort(); uniq_a.dedup();
            let mut uniq_b: Vec<&str> = types_b.clone();
            uniq_b.sort(); uniq_b.dedup();

            eprintln!("\n--- Failure #{} (pair {},{} {}) ---", found, i, j, op_name(op));
            eprintln!("  A: {} faces, types: {:?}", sa.outer_shell().face_count(), uniq_a);
            eprintln!("  B: {} faces, types: {:?}", sb.outer_shell().face_count(), uniq_b);
            eprintln!("  Error: {}", err);

            // Check if SSI between face pairs finds anything
            let tolerance = 1e-6;
            let mut ssi_pairs = 0;
            let mut ssi_traces = 0;
            for fa in sa.outer_shell().faces() {
                for fb in sb.outer_shell().faces() {
                    let ba = face_bbox(fa);
                    let bb = face_bbox(fb);
                    if !ba.intersects(&bb) { continue; }
                    ssi_pairs += 1;
                    if let Ok(traces) = knot_intersect::surface_surface::intersect_surfaces(
                        fa.surface(), fb.surface(), tolerance
                    ) {
                        ssi_traces += traces.iter().filter(|t| t.points.len() >= 2).count();
                    }
                    if ssi_pairs > 30 { break; } // limit
                }
                if ssi_pairs > 30 { break; }
            }
            eprintln!("  SSI: {} candidate pairs checked, {} traces found", ssi_pairs, ssi_traces);
        }
    }

    eprintln!("\nTotal real topology failures: {}", found);
}

fn face_bbox(face: &knot_topo::Face) -> knot_core::Aabb3 {
    let pts: Vec<knot_geom::Point3> = face.outer_loop().half_edges().iter()
        .map(|he| *he.start_vertex().point()).collect();
    knot_core::Aabb3::from_points(&pts).unwrap().expand(1e-6)
}
