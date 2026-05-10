//! Pair (11, 12) bbox-filter diagnostic. For each model pair, report:
//! - total raw face-pair count (n_a × n_b)
//! - count after vertex-bbox overlap filter
//! - filter survival rate
//! - distribution of face-bbox diagonals (median, p90, p99) on each model
//!
//! Tells us whether the bbox filter is "barely doing anything" (loose
//! bboxes are the bottleneck — surface-aware bboxes would help) or
//! whether the models genuinely overlap heavily across most pairs
//! (brute-force per-pair work is unavoidable and Phase 2B.4 / faster
//! per-pair is the only lever).

use std::path::PathBuf;
use knot_io::from_step;
use knot_core::{Aabb3, bbox::Aabb3 as Aabb3Type};
use knot_geom::Point3;

fn pair_paths() -> Vec<(PathBuf, PathBuf)> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("data").join("abc");
    vec![
        (
            base.join("0000/00000011/00000011_e909f412cda24521865fac0f_step_000.step"),
            base.join("0000/00000012/00000012_f16882934f314832b639ffc0_step_000.step"),
        ),
        (
            base.join("0000/00000032/00000032_ad34a3f60c4a4caa99646600_step_012.step"),
            base.join("0000/00000033/00000033_ad34a3f60c4a4caa99646600_step_013.step"),
        ),
        (
            base.join("0000/00000024/00000024_ad34a3f60c4a4caa99646600_step_014.step"),
            base.join("0000/00000025/00000025_ad34a3f60c4a4caa99646600_step_015.step"),
        ),
    ]
}

fn face_bbox_vertex_only(face: &knot_topo::Face, margin: f64) -> Aabb3Type {
    let pts: Vec<Point3> = face.outer_loop().half_edges()
        .iter()
        .map(|he| *he.start_vertex().point())
        .collect();
    Aabb3::from_points(&pts).unwrap().expand(margin)
}

#[test]
#[ignore]
fn pathological_pair_filter_survival() {
    for (path_a, path_b) in pair_paths() {
        if !path_a.exists() || !path_b.exists() {
            eprintln!("skip pair: {} or {} missing", path_a.display(), path_b.display());
            continue;
        }
        let a_content = std::fs::read_to_string(&path_a).unwrap();
        let b_content = std::fs::read_to_string(&path_b).unwrap();
        let a_brep = match from_step(&a_content) { Ok(b) => b, _ => continue };
        let b_brep = match from_step(&b_content) { Ok(b) => b, _ => continue };
        let a_solid = a_brep.solids().first().unwrap();
        let b_solid = b_brep.solids().first().unwrap();
        let a_faces = a_solid.outer_shell().faces();
        let b_faces = b_solid.outer_shell().faces();

        // Combined bbox to derive tolerance.
        let mut all_pts: Vec<Point3> = Vec::new();
        for f in a_faces {
            for he in f.outer_loop().half_edges() {
                all_pts.push(*he.start_vertex().point());
            }
        }
        for f in b_faces {
            for he in f.outer_loop().half_edges() {
                all_pts.push(*he.start_vertex().point());
            }
        }
        let combined = Aabb3::from_points(&all_pts).unwrap();
        let tolerance = combined.diagonal_length() * 1e-7;

        let bboxes_a: Vec<Aabb3Type> =
            a_faces.iter().map(|f| face_bbox_vertex_only(f, tolerance)).collect();
        let bboxes_b: Vec<Aabb3Type> =
            b_faces.iter().map(|f| face_bbox_vertex_only(f, tolerance)).collect();

        let n_a = bboxes_a.len();
        let n_b = bboxes_b.len();
        let raw = n_a * n_b;

        let mut survivors = 0usize;
        for ba in &bboxes_a {
            for bb in &bboxes_b {
                if ba.intersects(bb) {
                    survivors += 1;
                }
            }
        }

        // Diagonal distribution.
        let mut diag_a: Vec<f64> = bboxes_a.iter().map(|b| b.diagonal_length()).collect();
        let mut diag_b: Vec<f64> = bboxes_b.iter().map(|b| b.diagonal_length()).collect();
        diag_a.sort_by(|a, b| a.partial_cmp(b).unwrap());
        diag_b.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median_a = diag_a[n_a / 2];
        let median_b = diag_b[n_b / 2];
        let p90_a = diag_a[(n_a as f64 * 0.9) as usize];
        let p90_b = diag_b[(n_b as f64 * 0.9) as usize];

        let combined_diag = combined.diagonal_length();

        eprintln!(
            "\n=== {} × {} ===",
            path_a.file_name().unwrap().to_string_lossy(),
            path_b.file_name().unwrap().to_string_lossy(),
        );
        eprintln!("  faces: {} × {} = {} raw pairs", n_a, n_b, raw);
        eprintln!("  bbox-filter survivors: {} ({:.1}% of raw)",
            survivors, 100.0 * survivors as f64 / raw as f64);
        eprintln!("  combined model diag: {:.4e}", combined_diag);
        eprintln!("  face bbox diag median: a={:.4e}, b={:.4e}", median_a, median_b);
        eprintln!("  face bbox diag p90:    a={:.4e}, b={:.4e}", p90_a, p90_b);
        eprintln!("  median face/model ratio: a={:.3}, b={:.3}",
            median_a / combined_diag, median_b / combined_diag);

        // Surface-type breakdown of model A and B faces.
        let surface_types = |faces: &[knot_topo::Face]| -> std::collections::BTreeMap<&'static str, usize> {
            let mut map = std::collections::BTreeMap::new();
            for f in faces {
                let kind = match &**f.surface() {
                    knot_geom::surface::Surface::Plane(_) => "Plane",
                    knot_geom::surface::Surface::Sphere(_) => "Sphere",
                    knot_geom::surface::Surface::Cylinder(_) => "Cylinder",
                    knot_geom::surface::Surface::Cone(_) => "Cone",
                    knot_geom::surface::Surface::Torus(_) => "Torus",
                    knot_geom::surface::Surface::Nurbs(_) => "Nurbs",
                };
                *map.entry(kind).or_insert(0) += 1;
            }
            map
        };
        eprintln!("  A surface types: {:?}", surface_types(a_faces));
        eprintln!("  B surface types: {:?}", surface_types(b_faces));
    }
}
