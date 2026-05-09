//! Diagnose WHY face splitting produces bad Euler characteristics.

use std::path::PathBuf;
use knot_io::from_step;
use knot_ops::boolean::{boolean, BooleanOp};

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
fn euler_root_cause() {
    let files = find_step_files(100);
    if files.is_empty() { return; }

    let mut models: Vec<knot_topo::BRep> = Vec::new();
    for f in &files {
        if models.len() >= 50 { break; }
        if let Ok(c) = std::fs::read_to_string(f) {
            if let Ok(b) = from_step(&c) { models.push(b); }
        }
    }

    let mut found = 0;
    let n_pairs = models.len().min(30);

    for i in 0..n_pairs {
        let j = (i + 1) % models.len();
        if i == j { continue; }

        for &op in &[BooleanOp::Union, BooleanOp::Intersection, BooleanOp::Subtraction] {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                boolean(&models[i], &models[j], op)
            }));

            let is_euler = match &result {
                Ok(Err(e)) => e.to_string().contains("Euler"),
                _ => false,
            };

            if !is_euler { continue; }
            found += 1;
            if found > 3 { continue; }

            eprintln!("\n=== Euler failure #{} (pair {}, {:?}) ===", found, i, op);

            let sa = models[i].single_solid().unwrap();
            let sb = models[j].single_solid().unwrap();

            eprintln!("Model A: {} faces", sa.outer_shell().face_count());
            eprintln!("Model B: {} faces", sb.outer_shell().face_count());

            // Check how many faces have 1-2 edges (seam edges)
            let seam_a = sa.outer_shell().faces().iter()
                .filter(|f| f.outer_loop().half_edges().len() <= 2).count();
            let seam_b = sb.outer_shell().faces().iter()
                .filter(|f| f.outer_loop().half_edges().len() <= 2).count();
            eprintln!("Seam faces (1-2 edges): A={}, B={}", seam_a, seam_b);

            // Check face edge count distribution
            let mut a_sizes: Vec<usize> = sa.outer_shell().faces().iter()
                .map(|f| f.outer_loop().half_edges().len()).collect();
            let mut b_sizes: Vec<usize> = sb.outer_shell().faces().iter()
                .map(|f| f.outer_loop().half_edges().len()).collect();
            a_sizes.sort();
            b_sizes.sort();
            eprintln!("A edge counts: {:?}", a_sizes);
            eprintln!("B edge counts: {:?}", b_sizes);

            // Check if the models themselves have valid Euler
            let euler_a = compute_euler(sa.outer_shell());
            let euler_b = compute_euler(sb.outer_shell());
            eprintln!("Input Euler: A={}, B={}", euler_a, euler_b);

            eprintln!("Error: {}", result.unwrap().unwrap_err());
        }
    }

    eprintln!("\nTotal Euler failures: {}", found);
}

fn compute_euler(shell: &knot_topo::Shell) -> i64 {
    use std::collections::HashMap;
    use knot_core::{SnapGrid, snap::LatticeIndex};
    let grid = SnapGrid::new(1e-10);

    let mut edges: HashMap<(LatticeIndex, LatticeIndex), ()> = HashMap::new();
    let mut verts: HashMap<LatticeIndex, ()> = HashMap::new();

    for face in shell.faces() {
        for he in face.outer_loop().half_edges() {
            let s = grid.lattice_index(*he.start_vertex().point());
            let e = grid.lattice_index(*he.end_vertex().point());
            let key = if s <= e { (s, e) } else { (e, s) };
            edges.entry(key).or_insert(());
            verts.entry(s).or_insert(());
            verts.entry(e).or_insert(());
        }
    }

    verts.len() as i64 - edges.len() as i64 + shell.face_count() as i64
}
