//! Detailed failure analysis for the reliability harness.

use knot_core::Aabb3;
use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::surface::Surface;
use knot_ops::primitives;
use knot_ops::boolean::{boolean, BooleanOp};
use knot_tessellate::{tessellate, TessellateOptions};

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self { Self(seed) }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        let t = (self.next() & 0xFFFFFFFF) as f64 / 0xFFFFFFFF_u64 as f64;
        lo + t * (hi - lo)
    }
    fn range(&mut self, n: usize) -> usize {
        (self.next() as usize) % n
    }
}

fn shape_name(idx: usize) -> &'static str {
    match idx { 0 => "box", 1 => "sphere", _ => "cylinder" }
}

fn op_name(op: BooleanOp) -> &'static str {
    match op {
        BooleanOp::Union => "union",
        BooleanOp::Intersection => "intersection",
        BooleanOp::Subtraction => "subtraction",
    }
}

#[test]
fn analyze_failures() {
    let mut rng = Rng::new(42);
    let n_pairs = 100;

    // Track failures by category
    let mut by_op = [0u32; 3]; // union, intersection, subtraction
    let mut by_op_total = [0u32; 3];
    let mut by_shape_pair = std::collections::HashMap::<String, (u32, u32)>::new(); // (fail, total)
    let mut failure_messages = Vec::new();
    let mut disjoint_fails = 0u32;
    let mut overlapping_fails = 0u32;
    let mut disjoint_total = 0u32;
    let mut overlapping_total = 0u32;

    for _ in 0..n_pairs {
        let shape_a = rng.range(3);
        let shape_b = rng.range(3);
        let ox_a = rng.uniform(-2.0, 2.0);
        let oy_a = rng.uniform(-2.0, 2.0);
        let oz_a = rng.uniform(-2.0, 2.0);
        let size_a = rng.uniform(0.5, 3.0);
        let ox_b = rng.uniform(-2.0, 2.0);
        let oy_b = rng.uniform(-2.0, 2.0);
        let oz_b = rng.uniform(-2.0, 2.0);
        let size_b = rng.uniform(0.5, 3.0);

        let a = make_solid(&mut rng, shape_a, ox_a, oy_a, oz_a, size_a);
        let b = make_solid(&mut rng, shape_b, ox_b, oy_b, oz_b, size_b);

        // Estimate if bounding boxes overlap (rough proxy for "overlapping")
        let dist = ((ox_a - ox_b).powi(2) + (oy_a - oy_b).powi(2) + (oz_a - oz_b).powi(2)).sqrt();
        let overlapping = dist < (size_a + size_b);

        let pair_key = format!("{}-{}", shape_name(shape_a), shape_name(shape_b));

        for (oi, &op) in [BooleanOp::Union, BooleanOp::Intersection, BooleanOp::Subtraction].iter().enumerate() {
            by_op_total[oi] += 1;
            let entry = by_shape_pair.entry(pair_key.clone()).or_insert((0, 0));
            entry.1 += 1;

            if overlapping { overlapping_total += 1; } else { disjoint_total += 1; }

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| boolean(&a, &b, op)));
            let failed = match result {
                Err(_) => true,
                Ok(Err(e)) => {
                    let msg = e.to_string();
                    if !msg.contains("empty") && !msg.contains("Empty") && !msg.contains("no faces") {
                        failure_messages.push(format!("{} {} {}: {}", pair_key, op_name(op),
                            if overlapping { "OVL" } else { "DIS" }, msg));
                        true
                    } else {
                        false // empty result is correct for disjoint intersection
                    }
                }
                Ok(Ok(brep)) => {
                    match tessellate(&brep, TessellateOptions::default()) {
                        Ok(m) if m.triangle_count() > 0 => false,
                        Ok(_) => { failure_messages.push(format!("{} {}: 0 triangles", pair_key, op_name(op))); true }
                        Err(e) => { failure_messages.push(format!("{} {}: tess {}", pair_key, op_name(op), e)); true }
                    }
                }
            };

            if failed {
                by_op[oi] += 1;
                by_shape_pair.get_mut(&pair_key).unwrap().0 += 1;
                if overlapping { overlapping_fails += 1; } else { disjoint_fails += 1; }
            }
        }
    }

    eprintln!("\n=== FAILURE BREAKDOWN ===\n");

    eprintln!("By operation:");
    for (i, name) in ["union", "intersection", "subtraction"].iter().enumerate() {
        eprintln!("  {:<14} {}/{} ({:.0}% fail)",
            name, by_op[i], by_op_total[i],
            by_op[i] as f64 / by_op_total[i] as f64 * 100.0);
    }

    eprintln!("\nBy overlap:");
    eprintln!("  overlapping:  {}/{} ({:.0}% fail)", overlapping_fails, overlapping_total,
        if overlapping_total > 0 { overlapping_fails as f64 / overlapping_total as f64 * 100.0 } else { 0.0 });
    eprintln!("  disjoint:     {}/{} ({:.0}% fail)", disjoint_fails, disjoint_total,
        if disjoint_total > 0 { disjoint_fails as f64 / disjoint_total as f64 * 100.0 } else { 0.0 });

    eprintln!("\nBy shape pair:");
    let mut pairs: Vec<_> = by_shape_pair.iter().collect();
    pairs.sort_by(|a, b| (b.1.0 as f64 / b.1.1 as f64).partial_cmp(&(a.1.0 as f64 / a.1.1 as f64)).unwrap());
    for (key, (fail, total)) in &pairs {
        eprintln!("  {:<20} {}/{} ({:.0}% fail)", key, fail, total,
            *fail as f64 / *total as f64 * 100.0);
    }

    eprintln!("\nFirst 20 failure messages:");
    for msg in failure_messages.iter().take(20) {
        eprintln!("  {}", msg);
    }
}

fn make_solid(rng: &mut Rng, shape: usize, ox: f64, oy: f64, oz: f64, size: f64) -> knot_topo::BRep {
    match shape {
        0 => {
            let sx = rng.uniform(0.5, size);
            let sy = rng.uniform(0.5, size);
            let sz = rng.uniform(0.5, size);
            make_offset_box(ox, oy, oz, sx, sy, sz)
        }
        1 => {
            let r = rng.uniform(0.3, size * 0.6);
            let n_lon = 6 + rng.range(10) as u32;
            let n_lat = 3 + rng.range(5) as u32;
            primitives::make_sphere(Point3::new(ox, oy, oz), r, n_lon, n_lat).unwrap()
        }
        _ => {
            let r = rng.uniform(0.2, size * 0.4);
            let h = rng.uniform(0.5, size);
            let n = 6 + rng.range(12) as u32;
            primitives::make_cylinder(Point3::new(ox, oy, oz), r, h, n).unwrap()
        }
    }
}

/// Diagnose the first failure from the seed-42 reliability run.
///
/// Replays the same RNG sequence as `analyze_failures` to find the first
/// failing (shape pair, boolean op) combination, then prints detailed
/// topology diagnostics for that failure.
#[test]
fn diagnose_first_failure() {
    use std::sync::Arc;
    use std::collections::HashMap;
    use knot_geom::curve::{Curve, LineSeg};
    use knot_geom::surface::Surface;
    use knot_core::{Aabb3, SnapGrid};
    use knot_core::snap::LatticeIndex;
    use knot_intersect::surface_surface::intersect_surfaces;

    let mut rng = Rng::new(42);
    let n_pairs = 100;

    for pair_idx in 0..n_pairs {
        let shape_a = rng.range(3);
        let shape_b = rng.range(3);
        let ox_a = rng.uniform(-2.0, 2.0);
        let oy_a = rng.uniform(-2.0, 2.0);
        let oz_a = rng.uniform(-2.0, 2.0);
        let size_a = rng.uniform(0.5, 3.0);
        let ox_b = rng.uniform(-2.0, 2.0);
        let oy_b = rng.uniform(-2.0, 2.0);
        let oz_b = rng.uniform(-2.0, 2.0);
        let size_b = rng.uniform(0.5, 3.0);

        let a = make_solid(&mut rng, shape_a, ox_a, oy_a, oz_a, size_a);
        let b = make_solid(&mut rng, shape_b, ox_b, oy_b, oz_b, size_b);

        let dist = ((ox_a - ox_b).powi(2) + (oy_a - oy_b).powi(2) + (oz_a - oz_b).powi(2)).sqrt();
        let overlapping = dist < (size_a + size_b);

        for &op in [BooleanOp::Union, BooleanOp::Intersection, BooleanOp::Subtraction].iter() {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| boolean(&a, &b, op)));
            let (failed, err_msg) = match &result {
                Err(_) => (true, "PANIC".to_string()),
                Ok(Err(e)) => {
                    let msg = e.to_string();
                    if msg.contains("empty") || msg.contains("Empty") || msg.contains("no faces") {
                        (false, String::new())
                    } else {
                        (true, msg)
                    }
                }
                Ok(Ok(brep)) => {
                    match tessellate(brep, TessellateOptions::default()) {
                        Ok(m) if m.triangle_count() > 0 => (false, String::new()),
                        Ok(_) => (true, "0 triangles".to_string()),
                        Err(e) => (true, format!("tess: {}", e)),
                    }
                }
            };

            if !failed {
                continue;
            }

            // ─── Found the first failure! Print full diagnostics. ───

            eprintln!("\n============================================================");
            eprintln!("FIRST FAILURE: pair #{}, {} op", pair_idx, op_name(op));
            eprintln!("============================================================\n");

            // Shape types and parameters
            eprintln!("Shape A: {} | center=({:.4}, {:.4}, {:.4}) size={:.4}",
                shape_name(shape_a), ox_a, oy_a, oz_a, size_a);
            eprintln!("Shape B: {} | center=({:.4}, {:.4}, {:.4}) size={:.4}",
                shape_name(shape_b), ox_b, oy_b, oz_b, size_b);
            eprintln!("Overlap: {} (dist={:.4}, sum_size={:.4})",
                overlapping, dist, size_a + size_b);

            // Print input shape details
            let solid_a = a.single_solid().unwrap();
            let solid_b = b.single_solid().unwrap();
            eprintln!("\nInput A: {} faces, surfaces: {:?}",
                solid_a.outer_shell().face_count(),
                solid_a.outer_shell().faces().iter()
                    .map(|f| surface_type_name(f.surface()))
                    .collect::<Vec<_>>());
            eprintln!("Input B: {} faces, surfaces: {:?}",
                solid_b.outer_shell().face_count(),
                solid_b.outer_shell().faces().iter()
                    .map(|f| surface_type_name(f.surface()))
                    .collect::<Vec<_>>());

            // Which boolean op fails
            eprintln!("\nBoolean op: {}", op_name(op));
            eprintln!("Error: {}", err_msg);

            // ─── Replicate the boolean pipeline to get pre-validation topology ───

            // Step 1: grid
            let bbox = {
                let mut pts: Vec<Point3> = Vec::new();
                for face in solid_a.outer_shell().faces() {
                    for he in face.outer_loop().half_edges() {
                        pts.push(*he.start_vertex().point());
                    }
                }
                for face in solid_b.outer_shell().faces() {
                    for he in face.outer_loop().half_edges() {
                        pts.push(*he.start_vertex().point());
                    }
                }
                Aabb3::from_points(&pts).unwrap()
            };
            let grid = SnapGrid::from_bbox_diagonal(bbox.diagonal_length(), 1e-9);
            let tolerance = grid.cell_size * 100.0;

            let faces_a: Vec<&knot_topo::Face> = solid_a.outer_shell().faces().iter().collect();
            let faces_b: Vec<&knot_topo::Face> = solid_b.outer_shell().faces().iter().collect();

            // Step 2: candidate pairs via bbox
            let mut candidate_pairs = Vec::new();
            for (ia, fa) in faces_a.iter().enumerate() {
                let ba = face_bbox_diag(fa, tolerance);
                for (ib, fb) in faces_b.iter().enumerate() {
                    let bb = face_bbox_diag(fb, tolerance);
                    if ba.intersects(&bb) {
                        candidate_pairs.push((ia, ib));
                    }
                }
            }

            eprintln!("\nCandidate face pairs (bbox overlap): {}", candidate_pairs.len());

            // Step 3: SSI
            struct FaceIx {
                face_a_idx: usize,
                face_b_idx: usize,
                trace: knot_intersect::SurfaceSurfaceTrace,
            }
            let mut intersections: Vec<FaceIx> = Vec::new();
            let mut ssi_errors = 0;
            for &(ia, ib) in &candidate_pairs {
                match intersect_surfaces(faces_a[ia].surface(), faces_b[ib].surface(), tolerance) {
                    Ok(traces) => {
                        for trace in traces {
                            if trace.points.len() >= 2 {
                                intersections.push(FaceIx {
                                    face_a_idx: ia,
                                    face_b_idx: ib,
                                    trace,
                                });
                            }
                        }
                    }
                    Err(e) => {
                        ssi_errors += 1;
                        eprintln!("  SSI error on face pair ({}, {}): {}", ia, ib, e);
                    }
                }
            }
            eprintln!("SSI traces found: {} (errors: {})", intersections.len(), ssi_errors);

            // Step 4-5: Split faces (replicate split_faces logic locally)
            // Use a local TopologyBuilder equivalent
            let mut vertex_cache: HashMap<LatticeIndex, Arc<knot_topo::Vertex>> = HashMap::new();
            let mut edge_cache: HashMap<(LatticeIndex, LatticeIndex), Arc<knot_topo::Edge>> = HashMap::new();
            let mut dropped_faces_a = 0usize;
            let mut dropped_faces_b = 0usize;
            let mut dropped_reasons_a: Vec<String> = Vec::new();
            let mut dropped_reasons_b: Vec<String> = Vec::new();

            let get_vertex = |cache: &mut HashMap<LatticeIndex, Arc<knot_topo::Vertex>>, grid: &SnapGrid, point: Point3| -> Arc<knot_topo::Vertex> {
                let li = grid.lattice_index(point);
                cache.entry(li)
                    .or_insert_with(|| Arc::new(knot_topo::Vertex::new(grid.lattice_to_point(li))))
                    .clone()
            };

            let get_edge = |ecache: &mut HashMap<(LatticeIndex, LatticeIndex), Arc<knot_topo::Edge>>,
                            _vcache: &mut HashMap<LatticeIndex, Arc<knot_topo::Vertex>>,
                            grid: &SnapGrid,
                            start: &Arc<knot_topo::Vertex>,
                            end: &Arc<knot_topo::Vertex>| -> (Arc<knot_topo::Edge>, bool) {
                let start_li = grid.lattice_index(*start.point());
                let end_li = grid.lattice_index(*end.point());
                let (key, same_sense) = if start_li <= end_li {
                    ((start_li, end_li), true)
                } else {
                    ((end_li, start_li), false)
                };
                let edge = ecache.entry(key).or_insert_with(|| {
                    let (cs, ce) = if same_sense {
                        (start.clone(), end.clone())
                    } else {
                        (end.clone(), start.clone())
                    };
                    let curve = Arc::new(Curve::Line(LineSeg::new(*cs.point(), *ce.point())));
                    Arc::new(knot_topo::Edge::new(cs, ce, curve, 0.0, 1.0))
                }).clone();
                (edge, same_sense)
            };

            let try_polygon_to_face = |polygon: &[Point3],
                                        surface: Arc<Surface>,
                                        same_sense: bool,
                                        vcache: &mut HashMap<LatticeIndex, Arc<knot_topo::Vertex>>,
                                        ecache: &mut HashMap<(LatticeIndex, LatticeIndex), Arc<knot_topo::Edge>>,
                                        grid: &SnapGrid|
                -> Result<knot_topo::Face, String> {
                if polygon.len() < 3 {
                    return Err("polygon < 3 vertices".into());
                }
                let verts: Vec<Arc<knot_topo::Vertex>> = polygon.iter()
                    .map(|p| get_vertex(vcache, grid, *p))
                    .collect();
                let mut deduped: Vec<Arc<knot_topo::Vertex>> = Vec::new();
                for v in &verts {
                    if deduped.last().map_or(true, |last: &Arc<knot_topo::Vertex>| last.id() != v.id()) {
                        deduped.push(v.clone());
                    }
                }
                if deduped.len() >= 2 && deduped.first().unwrap().id() == deduped.last().unwrap().id() {
                    deduped.pop();
                }
                let n = deduped.len();
                if n < 3 {
                    return Err(format!("degenerate after snap: {} -> {} verts", polygon.len(), n));
                }
                let mut half_edges = Vec::new();
                for i in 0..n {
                    let j = (i + 1) % n;
                    let (edge, fwd) = get_edge(ecache, vcache, grid, &deduped[i], &deduped[j]);
                    half_edges.push(knot_topo::HalfEdge::new(edge, fwd));
                }
                let loop_ = knot_topo::Loop::new(half_edges, true)
                    .map_err(|e| format!("loop: {}", e))?;
                knot_topo::Face::new(surface, loop_, vec![], same_sense)
                    .map_err(|e| format!("face: {}", e))
            };

            // Replicate split_faces for side A
            let split_side = |faces: &[&knot_topo::Face],
                              intersections: &[FaceIx],
                              is_a_side: bool,
                              tolerance: f64,
                              vcache: &mut HashMap<LatticeIndex, Arc<knot_topo::Vertex>>,
                              ecache: &mut HashMap<(LatticeIndex, LatticeIndex), Arc<knot_topo::Edge>>,
                              grid: &SnapGrid,
                              dropped: &mut usize,
                              dropped_reasons: &mut Vec<String>|
                -> Vec<knot_topo::Face> {
                let mut result = Vec::new();
                for (fi, face) in faces.iter().enumerate() {
                    let relevant: Vec<&knot_intersect::SurfaceSurfaceTrace> = intersections.iter()
                        .filter(|ix| if is_a_side { ix.face_a_idx == fi } else { ix.face_b_idx == fi })
                        .map(|ix| &ix.trace)
                        .collect();

                    let boundary: Vec<Point3> = face.outer_loop().half_edges().iter()
                        .map(|he| *he.start_vertex().point())
                        .collect();

                    if relevant.is_empty() || boundary.len() < 3 {
                        match try_polygon_to_face(&boundary, face.surface().clone(), face.same_sense(),
                                                   vcache, ecache, grid) {
                            Ok(f) => result.push(f),
                            Err(reason) => {
                                *dropped += 1;
                                dropped_reasons.push(format!("face {}: unsplit: {}", fi, reason));
                            }
                        }
                        continue;
                    }

                    let face_normal = compute_polygon_normal_local(&boundary);
                    let mut current_polygons = vec![boundary];

                    for trace in &relevant {
                        if trace.points.len() < 2 { continue; }
                        let cut_start = trace.points.first().unwrap();
                        let cut_end = trace.points.last().unwrap();
                        let cut_dir = cut_end - cut_start;
                        if cut_dir.norm() < tolerance { continue; }

                        let mut new_polygons = Vec::new();
                        for poly in &current_polygons {
                            let (left, right) = split_polygon_local(poly, cut_start, &cut_dir, &face_normal, tolerance);
                            if left.len() >= 3 { new_polygons.push(left); }
                            if right.len() >= 3 { new_polygons.push(right); }
                            if new_polygons.is_empty() { new_polygons.push(poly.clone()); }
                        }
                        current_polygons = new_polygons;
                    }

                    for (pi, poly) in current_polygons.iter().enumerate() {
                        match try_polygon_to_face(poly, face.surface().clone(), face.same_sense(),
                                                   vcache, ecache, grid) {
                            Ok(f) => result.push(f),
                            Err(reason) => {
                                *dropped += 1;
                                dropped_reasons.push(format!("face {} sub-polygon {}: {}", fi, pi, reason));
                            }
                        }
                    }
                }
                result
            };

            let split_a = split_side(&faces_a, &intersections, true, tolerance,
                &mut vertex_cache, &mut edge_cache, &grid,
                &mut dropped_faces_a, &mut dropped_reasons_a);
            let split_b = split_side(&faces_b, &intersections, false, tolerance,
                &mut vertex_cache, &mut edge_cache, &grid,
                &mut dropped_faces_b, &mut dropped_reasons_b);

            eprintln!("\nSplit faces A: {} (from {} originals, {} dropped)",
                split_a.len(), faces_a.len(), dropped_faces_a);
            eprintln!("Split faces B: {} (from {} originals, {} dropped)",
                split_b.len(), faces_b.len(), dropped_faces_b);

            if !dropped_reasons_a.is_empty() {
                eprintln!("\nDropped faces (A side):");
                for r in &dropped_reasons_a {
                    eprintln!("  {}", r);
                }
            }
            if !dropped_reasons_b.is_empty() {
                eprintln!("\nDropped faces (B side):");
                for r in &dropped_reasons_b {
                    eprintln!("  {}", r);
                }
            }

            // Collect ALL split faces (as if Union selected everything) to inspect topology
            let all_faces: Vec<knot_topo::Face> = split_a.iter().chain(split_b.iter()).cloned().collect();

            // Compute V, E, F using lattice indices (same method as validate.rs)
            let val_grid = SnapGrid::new(1e-10);
            let mut edge_use_count: HashMap<(LatticeIndex, LatticeIndex), usize> = HashMap::new();
            let mut vertex_set: HashMap<LatticeIndex, ()> = HashMap::new();
            let mut face_vertex_counts: Vec<usize> = Vec::new();

            for face in &all_faces {
                let hes = face.outer_loop().half_edges();
                face_vertex_counts.push(hes.len());
                for he in hes {
                    let start = val_grid.lattice_index(*he.start_vertex().point());
                    let end = val_grid.lattice_index(*he.end_vertex().point());
                    let key = if start <= end { (start, end) } else { (end, start) };
                    *edge_use_count.entry(key).or_insert(0) += 1;
                    vertex_set.entry(start).or_insert(());
                    vertex_set.entry(end).or_insert(());
                }
            }

            let v = vertex_set.len();
            let e = edge_use_count.len();
            let f = all_faces.len();
            let euler = v as i64 - e as i64 + f as i64;

            eprintln!("\n--- Euler Characteristic (all split faces, no classification) ---");
            eprintln!("V={}, E={}, F={}, V-E+F={}", v, e, f, euler);

            // Edge-use count distribution
            let mut use_dist: HashMap<usize, usize> = HashMap::new();
            for &count in edge_use_count.values() {
                *use_dist.entry(count).or_insert(0) += 1;
            }
            let mut use_dist_sorted: Vec<(usize, usize)> = use_dist.into_iter().collect();
            use_dist_sorted.sort();
            eprintln!("\n--- Edge-Use Count Distribution (all faces) ---");
            for (use_count, num_edges) in &use_dist_sorted {
                eprintln!("  use_count={}: {} edges", use_count, num_edges);
            }

            // Face vertex counts
            let mut fvc_dist: HashMap<usize, usize> = HashMap::new();
            for &c in &face_vertex_counts {
                *fvc_dist.entry(c).or_insert(0) += 1;
            }
            let mut fvc_sorted: Vec<(usize, usize)> = fvc_dist.into_iter().collect();
            fvc_sorted.sort();
            eprintln!("\n--- Face Vertex Counts ---");
            for (nverts, nfaces) in &fvc_sorted {
                eprintln!("  {} vertices: {} faces", nverts, nfaces);
            }

            // Now try to see what happens with just the boolean op's selected faces
            // We can't easily replicate classification without the private functions,
            // but we can report the pre-classification topology above and the error below.
            eprintln!("\n--- Boolean Result Error ---");
            eprintln!("{}", err_msg);

            // Also try the other two ops for context
            eprintln!("\n--- All Ops for This Pair ---");
            for &test_op in &[BooleanOp::Union, BooleanOp::Intersection, BooleanOp::Subtraction] {
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| boolean(&a, &b, test_op)));
                let status = match &res {
                    Err(_) => "PANIC".to_string(),
                    Ok(Err(e)) => {
                        let msg = e.to_string();
                        if msg.contains("empty") || msg.contains("Empty") || msg.contains("no faces") {
                            "OK (empty/disjoint)".to_string()
                        } else {
                            format!("FAIL: {}", msg)
                        }
                    }
                    Ok(Ok(brep)) => {
                        match tessellate(brep, TessellateOptions::default()) {
                            Ok(m) if m.triangle_count() > 0 =>
                                format!("OK ({} tris)", m.triangle_count()),
                            Ok(_) => "FAIL: 0 triangles".to_string(),
                            Err(e) => format!("FAIL: tess {}", e),
                        }
                    }
                };
                eprintln!("  {:<14} {}", op_name(test_op), status);
            }

            eprintln!("\n============================================================\n");
            return; // Stop after first failure
        }
    }

    eprintln!("No failures found in {} pairs with seed 42", n_pairs);
}

/// Helper: compute polygon normal (Newell's method) -- local copy for the diagnostic test.
fn compute_polygon_normal_local(verts: &[Point3]) -> Vector3 {
    let mut normal = Vector3::zeros();
    let n = verts.len();
    for i in 0..n {
        let curr = &verts[i];
        let next = &verts[(i + 1) % n];
        normal.x += (curr.y - next.y) * (curr.z + next.z);
        normal.y += (curr.z - next.z) * (curr.x + next.x);
        normal.z += (curr.x - next.x) * (curr.y + next.y);
    }
    let len = normal.norm();
    if len > 1e-30 { normal / len } else { Vector3::z() }
}

/// Helper: split polygon by a line (local copy for the diagnostic test).
fn split_polygon_local(
    polygon: &[Point3],
    line_point: &Point3,
    line_dir: &Vector3,
    face_normal: &Vector3,
    tolerance: f64,
) -> (Vec<Point3>, Vec<Point3>) {
    let n = polygon.len();
    if n < 3 { return (polygon.to_vec(), Vec::new()); }

    let split_normal = line_dir.cross(face_normal);
    let split_len = split_normal.norm();
    if split_len < 1e-12 { return (polygon.to_vec(), Vec::new()); }
    let split_normal = split_normal / split_len;

    let signs: Vec<f64> = polygon.iter()
        .map(|p| (p - line_point).dot(&split_normal))
        .collect();

    let mut left = Vec::new();
    let mut right = Vec::new();

    for i in 0..n {
        let j = (i + 1) % n;
        if signs[i] >= -tolerance { left.push(polygon[i]); }
        if signs[i] <= tolerance { right.push(polygon[i]); }

        if (signs[i] > tolerance && signs[j] < -tolerance)
            || (signs[i] < -tolerance && signs[j] > tolerance)
        {
            let t = signs[i] / (signs[i] - signs[j]);
            let pt = Point3::new(
                polygon[i].x + t * (polygon[j].x - polygon[i].x),
                polygon[i].y + t * (polygon[j].y - polygon[i].y),
                polygon[i].z + t * (polygon[j].z - polygon[i].z),
            );
            left.push(pt);
            right.push(pt);
        }
    }

    (left, right)
}

/// Helper: face bbox for candidate pair filtering.
fn face_bbox_diag(face: &knot_topo::Face, margin: f64) -> Aabb3 {
    let pts: Vec<Point3> = face.outer_loop().half_edges()
        .iter()
        .map(|he| *he.start_vertex().point())
        .collect();
    Aabb3::from_points(&pts).unwrap().expand(margin)
}

/// Helper: surface type name for diagnostics.
fn surface_type_name(surface: &Surface) -> &'static str {
    match surface {
        Surface::Plane(_) => "Plane",
        Surface::Sphere(_) => "Sphere",
        Surface::Cylinder(_) => "Cylinder",
        Surface::Cone(_) => "Cone",
        Surface::Torus(_) => "Torus",
        Surface::Nurbs(_) => "Nurbs",
    }
}

fn make_offset_box(ox: f64, oy: f64, oz: f64, sx: f64, sy: f64, sz: f64) -> knot_topo::BRep {
    use std::sync::Arc;
    use knot_geom::curve::{Curve, LineSeg};
    use knot_geom::surface::{Surface, Plane};
    use knot_topo::*;
    let hx = sx/2.0; let hy = sy/2.0; let hz = sz/2.0;
    let v = [
        Arc::new(Vertex::new(Point3::new(ox-hx, oy-hy, oz-hz))),
        Arc::new(Vertex::new(Point3::new(ox+hx, oy-hy, oz-hz))),
        Arc::new(Vertex::new(Point3::new(ox+hx, oy+hy, oz-hz))),
        Arc::new(Vertex::new(Point3::new(ox-hx, oy+hy, oz-hz))),
        Arc::new(Vertex::new(Point3::new(ox-hx, oy-hy, oz+hz))),
        Arc::new(Vertex::new(Point3::new(ox+hx, oy-hy, oz+hz))),
        Arc::new(Vertex::new(Point3::new(ox+hx, oy+hy, oz+hz))),
        Arc::new(Vertex::new(Point3::new(ox-hx, oy+hy, oz+hz))),
    ];
    let make_face = |vi: [usize; 4], origin: Point3, normal: Vector3| -> Face {
        let mut edges = Vec::new();
        for i in 0..4 { let j = (i+1)%4;
            let s = v[vi[i]].clone(); let e = v[vi[j]].clone();
            let c = Arc::new(Curve::Line(LineSeg::new(*s.point(), *e.point())));
            edges.push(HalfEdge::new(Arc::new(Edge::new(s, e, c, 0.0, 1.0)), true));
        }
        Face::new(Arc::new(Surface::Plane(Plane::new(origin, normal))), Loop::new(edges, true).unwrap(), vec![], true).unwrap()
    };
    let faces = vec![
        make_face([0,3,2,1], Point3::new(ox,oy,oz-hz), -Vector3::z()),
        make_face([4,5,6,7], Point3::new(ox,oy,oz+hz), Vector3::z()),
        make_face([0,1,5,4], Point3::new(ox,oy-hy,oz), -Vector3::y()),
        make_face([2,3,7,6], Point3::new(ox,oy+hy,oz), Vector3::y()),
        make_face([0,4,7,3], Point3::new(ox-hx,oy,oz), -Vector3::x()),
        make_face([1,2,6,5], Point3::new(ox+hx,oy,oz), Vector3::x()),
    ];
    let shell = Shell::new(faces, true).unwrap();
    BRep::new(vec![Solid::new(shell, vec![]).unwrap()]).unwrap()
}
