//! Diagnose specific edge-vs-vertex distance gaps. Find one failing
//! model, list every edge whose curve doesn't pass through its
//! vertices, dump the curve type and distances.

use std::path::PathBuf;
use knot_io::from_step;
use knot_geom::curve::{Curve, CurveParam};

fn find_step_files(max: usize) -> Vec<PathBuf> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("data").join("abc");
    if !base.exists() { return Vec::new(); }
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

fn curve_type(c: &Curve) -> &'static str {
    match c {
        Curve::Line(_) => "Line",
        Curve::CircularArc(_) => "CircularArc",
        Curve::EllipticalArc(_) => "EllipticalArc",
        Curve::Nurbs(_) => "Nurbs",
    }
}

#[test]
#[ignore]
fn dump_failing_edge_distances() {
    let files = find_step_files(20);
    for path in files.iter().take(20) {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let brep = match from_step(&content) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        // Walk every edge. Skip those that pass.
        let solid = match brep.solids().first() {
            Some(s) => s,
            None => continue,
        };
        let mut buggy = Vec::new();
        for (fi, face) in solid.outer_shell().faces().iter().enumerate() {
            for (ei, he) in face.outer_loop().half_edges().iter().enumerate() {
                let edge = he.edge();
                let cs = edge.curve().point_at(CurveParam(edge.t_start()));
                let ce = edge.curve().point_at(CurveParam(edge.t_end()));
                let vs = *edge.start().point();
                let ve = *edge.end().point();
                let ds = (cs - vs).norm();
                let de = (ce - ve).norm();
                if ds > 1e-6 || de > 1e-6 {
                    let domain = edge.curve().domain();
                    buggy.push(format!(
                        "f{}/e{}: type={} t=[{:.6},{:.6}] dom=[{:.6},{:.6}] d_start={:.4e} d_end={:.4e} vs=({:.4},{:.4},{:.4}) cs=({:.4},{:.4},{:.4})",
                        fi, ei, curve_type(edge.curve()),
                        edge.t_start(), edge.t_end(), domain.start, domain.end,
                        ds, de,
                        vs.x, vs.y, vs.z, cs.x, cs.y, cs.z,
                    ));
                }
            }
        }
        if !buggy.is_empty() {
            eprintln!("=== {} ({} buggy edges)", name, buggy.len());
            for b in buggy.iter().take(3) {
                eprintln!("  {}", b);
            }
            return; // stop after first failing model
        }
    }
}
