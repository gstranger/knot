//! Diagnostic: for the consistently-failing pair (11, 12), tabulate
//! the surface-pair types among the surviving candidates. If mostly
//! NURBS-vs-NURBS we need Phase 2B (algebraic); if mostly NURBS-vs-
//! analytic the normal-cone prune is the right move.

use std::path::PathBuf;
use knot_geom::surface::Surface;
use knot_io::from_step;
use knot_topo::Face;

fn surface_kind(face: &Face) -> &'static str {
    match face.surface().as_ref() {
        Surface::Plane(_) => "Plane",
        Surface::Sphere(_) => "Sphere",
        Surface::Cylinder(_) => "Cylinder",
        Surface::Cone(_) => "Cone",
        Surface::Torus(_) => "Torus",
        Surface::Nurbs(_) => "Nurbs",
    }
}

#[test]
#[ignore]
fn pair_11_12_surface_breakdown() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("data").join("abc");
    let a_path = base.join("0000/00000011/00000011_e909f412cda24521865fac0f_step_000.step");
    let b_path = base.join("0000/00000012/00000012_f16882934f314832b639ffc0_step_000.step");

    let a = from_step(&std::fs::read_to_string(&a_path).unwrap()).unwrap();
    let b = from_step(&std::fs::read_to_string(&b_path).unwrap()).unwrap();

    let faces_a: Vec<&Face> = a.solids()[0].outer_shell().faces().iter().collect();
    let faces_b: Vec<&Face> = b.solids()[0].outer_shell().faces().iter().collect();

    eprintln!("\n=== pair (11, 12) ===");
    eprintln!("Model A: {} faces", faces_a.len());
    eprintln!("Model B: {} faces", faces_b.len());

    // Model bboxes computed by hand — are these the same shape?
    let mut bb_a_min = [f64::INFINITY; 3];
    let mut bb_a_max = [f64::NEG_INFINITY; 3];
    for f in &faces_a {
        for he in f.outer_loop().half_edges().iter() {
            let p = he.start_vertex().point();
            bb_a_min[0] = bb_a_min[0].min(p.x); bb_a_max[0] = bb_a_max[0].max(p.x);
            bb_a_min[1] = bb_a_min[1].min(p.y); bb_a_max[1] = bb_a_max[1].max(p.y);
            bb_a_min[2] = bb_a_min[2].min(p.z); bb_a_max[2] = bb_a_max[2].max(p.z);
        }
    }
    let mut bb_b_min = [f64::INFINITY; 3];
    let mut bb_b_max = [f64::NEG_INFINITY; 3];
    for f in &faces_b {
        for he in f.outer_loop().half_edges().iter() {
            let p = he.start_vertex().point();
            bb_b_min[0] = bb_b_min[0].min(p.x); bb_b_max[0] = bb_b_max[0].max(p.x);
            bb_b_min[1] = bb_b_min[1].min(p.y); bb_b_max[1] = bb_b_max[1].max(p.y);
            bb_b_min[2] = bb_b_min[2].min(p.z); bb_b_max[2] = bb_b_max[2].max(p.z);
        }
    }
    eprintln!("A bbox: min=({:.2},{:.2},{:.2}) max=({:.2},{:.2},{:.2})",
        bb_a_min[0], bb_a_min[1], bb_a_min[2], bb_a_max[0], bb_a_max[1], bb_a_max[2]);
    eprintln!("B bbox: min=({:.2},{:.2},{:.2}) max=({:.2},{:.2},{:.2})",
        bb_b_min[0], bb_b_min[1], bb_b_min[2], bb_b_max[0], bb_b_max[1], bb_b_max[2]);
    let dx = (bb_a_min[0] + bb_a_max[0]) * 0.5 - (bb_b_min[0] + bb_b_max[0]) * 0.5;
    let dy = (bb_a_min[1] + bb_a_max[1]) * 0.5 - (bb_b_min[1] + bb_b_max[1]) * 0.5;
    let dz = (bb_a_min[2] + bb_a_max[2]) * 0.5 - (bb_b_min[2] + bb_b_max[2]) * 0.5;
    eprintln!("Centroid offset: ({:.4}, {:.4}, {:.4})  norm={:.4}",
        dx, dy, dz, (dx*dx + dy*dy + dz*dz).sqrt());

    // Face-type histogram
    let mut hist_a: std::collections::BTreeMap<&str, usize> = Default::default();
    for f in &faces_a { *hist_a.entry(surface_kind(f)).or_insert(0) += 1; }
    let mut hist_b: std::collections::BTreeMap<&str, usize> = Default::default();
    for f in &faces_b { *hist_b.entry(surface_kind(f)).or_insert(0) += 1; }
    eprintln!("\nFace types in A: {:?}", hist_a);
    eprintln!("Face types in B: {:?}", hist_b);

    // Pair-type histogram of ALL pairs (not yet filtered)
    let mut pair_hist: std::collections::BTreeMap<String, usize> = Default::default();
    for fa in &faces_a {
        let ka = surface_kind(fa);
        for fb in &faces_b {
            let kb = surface_kind(fb);
            // Canonicalize ordering
            let (k1, k2) = if ka <= kb { (ka, kb) } else { (kb, ka) };
            let label = format!("{}-vs-{}", k1, k2);
            *pair_hist.entry(label).or_insert(0) += 1;
        }
    }
    eprintln!("\nRaw face-pair-type distribution (top 10):");
    let mut sorted: Vec<_> = pair_hist.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (k, v) in sorted.iter().take(10) {
        eprintln!("  {:30} {:>6}", k, v);
    }
}
