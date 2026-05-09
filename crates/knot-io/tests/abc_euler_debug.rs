//! Deep diagnosis of Euler violations: what does the result topology look like?

use std::path::PathBuf;
use std::collections::HashMap;
use knot_io::from_step;
use knot_ops::boolean::{boolean, BooleanOp};
use knot_core::{SnapGrid, snap::LatticeIndex};

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
            let e = e.to_string_lossy().to_lowercase(); e == "step" || e == "stp"
        }) { files.push(p); }
    }
}

#[test]
#[ignore]
fn euler_violation_details() {
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

    let grid = SnapGrid::new(1e-10);
    let mut euler_cases = 0;
    let n_pairs = models.len().min(30);

    for i in 0..n_pairs {
        let j = (i + 1) % models.len();
        if i == j { continue; }

        for &op in &[BooleanOp::Union, BooleanOp::Intersection, BooleanOp::Subtraction] {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                boolean(&models[i], &models[j], op)
            }));

            // We want cases that succeed (NonManifoldEdge accepted) but have bad Euler
            let brep = match result {
                Ok(Ok(b)) => b,
                _ => continue,
            };

            // Check Euler ourselves
            let solid = match brep.single_solid() {
                Some(s) => s,
                None => continue,
            };
            let shell = solid.outer_shell();

            let mut edge_uses: HashMap<(LatticeIndex, LatticeIndex), usize> = HashMap::new();
            let mut verts: HashMap<LatticeIndex, ()> = HashMap::new();

            let to_li = |p: &knot_geom::Point3| grid.lattice_index(*p);

            for face in shell.faces() {
                for he in face.outer_loop().half_edges() {
                    let s = to_li(he.start_vertex().point());
                    let e = to_li(he.end_vertex().point());
                    let key = if s <= e { (s, e) } else { (e, s) };
                    *edge_uses.entry(key).or_insert(0) += 1;
                    verts.entry(s).or_insert(());
                    verts.entry(e).or_insert(());
                }
            }

            let v = verts.len() as i64;
            let e = edge_uses.len() as i64;
            let f = shell.face_count() as i64;
            let euler = v - e + f;

            if euler % 2 != 0 {
                euler_cases += 1;
                if euler_cases > 5 { continue; } // limit output

                eprintln!("\n=== Euler violation #{} (pair {}, {:?}) ===", euler_cases, i, op);
                eprintln!("V={} E={} F={} → V-E+F={}", v, e, f, euler);

                // Edge use distribution
                let mut use_dist: HashMap<usize, usize> = HashMap::new();
                for (_, count) in &edge_uses {
                    *use_dist.entry(*count).or_insert(0) += 1;
                }
                eprintln!("Edge use distribution: {:?}", use_dist);

                // Face sizes (number of edges per face)
                let mut face_sizes: HashMap<usize, usize> = HashMap::new();
                for face in shell.faces() {
                    let n = face.outer_loop().half_edges().len();
                    *face_sizes.entry(n).or_insert(0) += 1;
                }
                eprintln!("Face sizes: {:?}", face_sizes);

                // Count boundary edges (used only once)
                let boundary = edge_uses.values().filter(|&&c| c == 1).count();
                let manifold = edge_uses.values().filter(|&&c| c == 2).count();
                let nonmanifold = edge_uses.values().filter(|&&c| c > 2).count();
                eprintln!("Boundary edges (1 use): {}", boundary);
                eprintln!("Manifold edges (2 uses): {}", manifold);
                eprintln!("Non-manifold edges (>2 uses): {}", nonmanifold);

                // Check: are there faces with all same-oriented edges?
                // (indicating a face was not properly split)
                let mut unsplit_count = 0;
                for face in shell.faces() {
                    let hes = face.outer_loop().half_edges();
                    if hes.len() > 6 {
                        unsplit_count += 1;
                    }
                }
                if unsplit_count > 0 {
                    eprintln!("Faces with >6 edges (likely unsplit): {}", unsplit_count);
                }
            }
        }
    }

    eprintln!("\nTotal Euler violations found: {}", euler_cases);
}
