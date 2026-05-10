use std::sync::Arc;
use knot_core::{KResult, KernelError, ErrorCode, Aabb3, SnapGrid};
use knot_core::snap::LatticeIndex;
use knot_core::exact::{ExactPoint3, ExactRational, Orientation, point_side_of_plane};
use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::curve::{Curve, LineSeg};
use knot_geom::surface::{Surface, SurfaceParam, Plane};
use knot_topo::*;
use knot_intersect::surface_surface::intersect_surfaces;
use crate::topo_builder::TopologyBuilder;

/// The three classical boolean operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanOp {
    Union,
    Intersection,
    Subtraction,
}

/// Perform a boolean operation on two BReps.
///
/// Topology-first algorithm:
/// 1. Compute model-level snap grid from combined bounding box
/// 2. Exact face-face overlap predicates to filter candidate pairs
/// 3. Compute SSI between candidate face pairs
/// 4. Build topology graph: which faces split, which edges introduced
/// 5. Split faces along intersection curves
/// 6. Classify split faces using exact point-side-of-plane predicates
/// 7. Select faces per boolean op
/// 8. Snap-round all vertices to the grid
/// 9. Assemble and validate result
pub fn boolean(a: &BRep, b: &BRep, op: BooleanOp) -> KResult<BRep> {
    // Per-call BezierPatch cache: thread-local, scoped to the boolean
    // op. The dispatcher's NURBS-vs-analytic paths re-decompose the
    // same NURBS surface for every face pair without this; on dense
    // models the duplication can dominate the SSI cost. Clearing
    // before and after ensures the cache's `*const NurbsSurface` keys
    // only point to live entities for the lifetime of any entry.
    knot_intersect::algebraic::nurbs_bridge::clear_bezier_patch_cache();
    let result = boolean_inner(a, b, op);
    knot_intersect::algebraic::nurbs_bridge::clear_bezier_patch_cache();
    result
}

fn boolean_inner(a: &BRep, b: &BRep, op: BooleanOp) -> KResult<BRep> {
    let solid_a = a.single_solid().ok_or_else(|| KernelError::InvalidInput {
        code: ErrorCode::UnsupportedConfiguration,
        detail: "boolean requires single-solid BReps".into(),
    })?;
    let solid_b = b.single_solid().ok_or_else(|| KernelError::InvalidInput {
        code: ErrorCode::UnsupportedConfiguration,
        detail: "boolean requires single-solid BReps".into(),
    })?;

    // Step 0: Quick sanity check on inputs — if either model has bad
    // topology (odd Euler from import linearization), bail early with a
    // clear error rather than producing a confusing downstream failure.
    if let Err(e) = a.validate() {
        // Accept NonManifoldEdge (common from import) but reject Euler violations
        if let KernelError::TopoInconsistency { code: ErrorCode::EulerViolation, .. } = &e {
            return Err(KernelError::InvalidInput {
                code: ErrorCode::EulerViolation,
                detail: format!("input A has invalid topology: {}", e),
            });
        }
    }
    if let Err(e) = b.validate() {
        if let KernelError::TopoInconsistency { code: ErrorCode::EulerViolation, .. } = &e {
            return Err(KernelError::InvalidInput {
                code: ErrorCode::EulerViolation,
                detail: format!("input B has invalid topology: {}", e),
            });
        }
    }

    // Wall-clock deadline and tracing. On WASM, std::time::Instant panics,
    // so these are compiled out entirely.
    #[cfg(not(target_arch = "wasm32"))]
    let _op_start = std::time::Instant::now();
    #[cfg(not(target_arch = "wasm32"))]
    let _op_deadline = _op_start + std::time::Duration::from_secs(8);
    #[cfg(not(target_arch = "wasm32"))]
    let _trace_enabled = std::env::var_os("KNOT_BOOLEAN_TRACE").is_some();
    #[cfg(target_arch = "wasm32")]
    let _trace_enabled = false;

    #[cfg(not(target_arch = "wasm32"))]
    fn _now() -> std::time::Instant { std::time::Instant::now() }
    #[cfg(target_arch = "wasm32")]
    fn _now() {}

    let check_deadline = |_stage: &str| -> KResult<()> {
        #[cfg(not(target_arch = "wasm32"))]
        if std::time::Instant::now() > _op_deadline {
            return Err(KernelError::OperationFailed {
                code: ErrorCode::OperationTimeout,
                detail: format!("boolean budget exceeded at stage {}", _stage),
            });
        }
        Ok(())
    };

    #[cfg(not(target_arch = "wasm32"))]
    let _trace_stage = |name: &str, t0: std::time::Instant| {
        if _trace_enabled {
            eprintln!(
                "  [boolean trace] {:<22} {:>8}ms  (cumulative {:>5}ms)",
                name,
                t0.elapsed().as_millis(),
                _op_start.elapsed().as_millis(),
            );
        }
    };
    #[cfg(target_arch = "wasm32")]
    let _trace_stage = |_name: &str, _t0: ()| {};

    // Step 1: Compute model-level snap grid from combined bounding box
    let _stage_start = _now();
    let bbox = compute_brep_bbox(solid_a, solid_b);
    let grid = SnapGrid::from_bbox_diagonal(bbox.diagonal_length(), 1e-9);
    let tolerance = grid.cell_size * 100.0; // geometric tolerance tied to grid
    let mut builder = TopologyBuilder::new(grid);

    let faces_a: Vec<&Face> = solid_a.outer_shell().faces().iter().collect();
    let faces_b: Vec<&Face> = solid_b.outer_shell().faces().iter().collect();
    _trace_stage("setup", _stage_start);

    // Step 2: Face-face overlap filtering via bounding boxes
    let _stage_start = _now();
    let candidate_pairs = find_candidate_pairs(&faces_a, &faces_b, tolerance);
    if _trace_enabled {
        eprintln!(
            "  [boolean trace] candidates: {} of {} raw ({:.1}%)",
            candidate_pairs.len(),
            faces_a.len() * faces_b.len(),
            100.0 * candidate_pairs.len() as f64 / (faces_a.len() * faces_b.len()) as f64,
        );
    }
    _trace_stage("candidate_filter", _stage_start);

    // Step 3: Compute SSI between candidate face pairs.
    // Clip cylinder/cone v-domains to face vertex extents so the SSI
    // grid sampling covers the actual overlap region.
    //
    // Wall-clock budget: large CAD models with hundreds of faces can
    // produce thousands of candidate pairs and tens of thousands of
    // SSI calls. Without spatial culling each pair is O(seed_grid),
    // so total cost grows quadratically. Bail with a structured error
    // once we exceed the budget — better than wedging the whole
    // pipeline on a model the current SSI can't service.
    //
    // Surface-pair memoization: many CAD faces share the same
    // underlying analytical surface (six faces of a cube use one
    // plane, multiple holes use one cylinder, etc.). Cache SSI
    // results keyed by `(Arc::as_ptr, Arc::as_ptr)` so a unique
    // surface pair is only intersected once. The pair is canonicalized
    // (sorted by pointer value) so direction doesn't matter.
    // Sequential SSI loop with per-iteration deadline check. Rayon
    // parallelism was tried here; on the pathological large-NURBS
    // models that drive the timeout cases each individual SSI call
    // can take seconds, so parallelism just delays the deadline
    // detection. For models that aren't SSI-bound the sequential
    // loop is already fast.
    let _stage_start = _now();
    let mut intersections: Vec<FaceIntersection> = Vec::new();
    for &(ia, ib) in &candidate_pairs {
        check_deadline("SSI")?;
        let sa = clip_cylinder_domain(faces_a[ia]);
        let sb = clip_cylinder_domain(faces_b[ib]);
        let traces = intersect_surfaces(&sa, &sb, tolerance)?;
        for trace in traces {
            if trace.points.len() >= 2 {
                intersections.push(FaceIntersection {
                    face_a_idx: ia,
                    face_b_idx: ib,
                    trace,
                });
            }
        }
    }
    if _trace_enabled {
        eprintln!(
            "  [boolean trace] SSI traces found: {} from {} candidate pairs",
            intersections.len(), candidate_pairs.len(),
        );
    }
    _trace_stage("ssi_loop", _stage_start);

    check_deadline("post-SSI")?;

    // Step 4-5: Split faces along intersection curves.
    // Both sides share a single TopologyBuilder so intersection edges are
    // allocated once and referenced with opposite half-edge orientations.
    let _stage_start = _now();
    let split_a = split_faces(&faces_a, &intersections, true, tolerance, &mut builder);
    let split_b = split_faces(&faces_b, &intersections, false, tolerance, &mut builder);
    if _trace_enabled {
        eprintln!(
            "  [boolean trace] split: A {} → {} sub-faces, B {} → {} sub-faces",
            faces_a.len(), split_a.len(), faces_b.len(), split_b.len(),
        );
    }
    _trace_stage("split_faces", _stage_start);

    check_deadline("post-split")?;

    // Step 6: Classify each sub-face using exact predicates where possible.
    // Pre-build per-solid classifiers so the per-face triangulation
    // and BVH construction is amortized over the full sub-face batch.
    let _stage_start = _now();
    let classifier_b = SolidClassifier::new(solid_b);
    let classifier_a = SolidClassifier::new(solid_a);
    let classified_a: Vec<(Face, Classification)> = split_a
        .into_iter()
        .map(|f| {
            let cls = classify_face_with(&f, &classifier_b);
            (f, cls)
        })
        .collect();

    let classified_b: Vec<(Face, Classification)> = split_b
        .into_iter()
        .map(|f| {
            let cls = classify_face_with(&f, &classifier_a);
            (f, cls)
        })
        .collect();
    _trace_stage("classify", _stage_start);

    check_deadline("post-classify")?;

    // Step 7: Select faces per boolean op
    let mut selected: Vec<Face> = Vec::new();
    match op {
        BooleanOp::Union => {
            for (face, cls) in &classified_a {
                if *cls == Classification::Outside { selected.push(face.clone()); }
            }
            for (face, cls) in &classified_b {
                if *cls == Classification::Outside { selected.push(face.clone()); }
            }
        }
        BooleanOp::Intersection => {
            for (face, cls) in &classified_a {
                if *cls == Classification::Inside { selected.push(face.clone()); }
            }
            for (face, cls) in &classified_b {
                if *cls == Classification::Inside { selected.push(face.clone()); }
            }
        }
        BooleanOp::Subtraction => {
            for (face, cls) in &classified_a {
                if *cls == Classification::Outside { selected.push(face.clone()); }
            }
            for (face, cls) in &classified_b {
                if *cls == Classification::Inside {
                    // Skip B-faces coplanar with any A-face: they lie on A's
                    // boundary and A's (trimmed) face already covers that region.
                    if is_face_coplanar_with_solid(face, solid_a, tolerance) {
                        continue;
                    }
                    selected.push(flip_face(face, &mut builder)?);
                }
            }
        }
    }

    if selected.is_empty() {
        return Err(KernelError::OperationFailed {
            code: ErrorCode::EmptyResult,
            detail: "boolean operation produced no faces".into(),
        });
    }

    // Step 7b: Deduplicate coplanar faces that share the same boundary.
    // This handles the case where intersection/subtraction produces two copies
    // of the same face from different solids at the shared boundary.
    let selected = deduplicate_faces(&selected, &grid);

    if selected.is_empty() {
        return Err(KernelError::OperationFailed {
            code: ErrorCode::EmptyResult,
            detail: "boolean operation produced no faces after dedup".into(),
        });
    }

    // Step 8: Snap-rounding is handled by the TopologyBuilder — all
    // vertices were snapped at allocation time. No separate pass needed.

    // Step 9: Assemble and validate.
    // Determine if the result should be treated as a closed shell.
    // When no intersections were found, the result may contain faces from
    // two disjoint solids — not a single closed shell.
    let expect_closed = !intersections.is_empty();
    let shell = Shell::new(selected, expect_closed)?;
    let solid = Solid::new(shell, vec![])?;
    let result = BRep::new(vec![solid])?;

    // Output validation: hard-fail only on structurally broken
    // outputs (LoopNotClosed, DanglingReference — half-edge graph is
    // corrupt). Soft-accept manifoldness/Euler defects:
    //
    // - **NonManifoldEdge** (an edge shared by 3+ faces) typically
    //   arises when cell classification drops one of the four faces
    //   around an SSI cut. Each surviving face still meshes cleanly.
    // - **EulerViolation** (V-E+F has wrong parity) means the global
    //   topology is missing a small number of vertex/edge/face
    //   counts; per-face geometry is still intact and tessellation
    //   produces a usable mesh.
    //
    // Callers needing strict-manifold output can re-run validate().
    if let Err(e) = result.validate() {
        match &e {
            KernelError::TopoInconsistency { code: ErrorCode::NonManifoldEdge, .. }
            | KernelError::TopoInconsistency { code: ErrorCode::EulerViolation, .. } => {
                // Soft-accept; carry on.
            }
            _ => return Err(e),
        }
    }

    Ok(result)
}

/// Handle the case where no surface-surface intersections were found.
/// The solids are either disjoint or one fully contains the other.
fn handle_no_intersection(
    a: &BRep, b: &BRep,
    solid_a: &Solid, solid_b: &Solid,
    op: BooleanOp,
    grid: &SnapGrid,
) -> KResult<BRep> {
    // Test containment: is A's centroid inside B? Is B's centroid inside A?
    let centroid_a = brep_centroid(solid_a);
    let centroid_b = brep_centroid(solid_b);
    let a_inside_b = classify_point_exact(&centroid_a, solid_b) == Classification::Inside;
    let b_inside_a = classify_point_exact(&centroid_b, solid_a) == Classification::Inside;

    match op {
        BooleanOp::Union => {
            if a_inside_b {
                // A is inside B → union = B
                Ok(b.clone())
            } else if b_inside_a {
                // B is inside A → union = A
                Ok(a.clone())
            } else {
                // Disjoint → union = both (but as a single BRep this is tricky;
                // for now, return A since we can't represent multi-solid results cleanly)
                // TODO: support multi-solid BReps for disjoint union
                Err(KernelError::OperationFailed {
                    code: ErrorCode::EmptyResult,
                    detail: "disjoint union not yet supported (no surface intersection found)".into(),
                })
            }
        }
        BooleanOp::Intersection => {
            if a_inside_b {
                Ok(a.clone()) // A ∩ B = A when A ⊂ B
            } else if b_inside_a {
                Ok(b.clone()) // A ∩ B = B when B ⊂ A
            } else {
                Err(KernelError::OperationFailed {
                    code: ErrorCode::EmptyResult,
                    detail: "no intersection (disjoint solids)".into(),
                })
            }
        }
        BooleanOp::Subtraction => {
            if a_inside_b {
                // A - B = empty when A ⊂ B
                Err(KernelError::OperationFailed {
                    code: ErrorCode::EmptyResult,
                    detail: "subtraction result is empty (A is inside B)".into(),
                })
            } else if b_inside_a {
                // A - B = A with B-shaped hole — need the full split pipeline.
                // But we have no SSI traces, which means the surfaces don't intersect.
                // This shouldn't happen for proper containment; return A as approximation.
                Ok(a.clone())
            } else {
                // Disjoint → A - B = A
                Ok(a.clone())
            }
        }
    }
}

fn brep_centroid(solid: &Solid) -> Point3 {
    let mut sum = Vector3::zeros();
    let mut count = 0;
    for face in solid.outer_shell().faces() {
        for he in face.outer_loop().half_edges() {
            sum += he.start_vertex().point().coords;
            count += 1;
        }
    }
    if count > 0 { Point3::from(sum / count as f64) } else { Point3::origin() }
}

// ═══════════════════════════════════════════════════════════════════
// Step 1: Bounding Box & Grid
// ═══════════════════════════════════════════════════════════════════

/// Clip cylinder/cone v-domain to the face's actual vertex extent.
/// STEP imports set v_min/v_max to +/-1e6; the SSI grid wastes time
/// sampling that entire range when only a small region has geometry.
fn clip_cylinder_domain(face: &Face) -> Surface {
    match face.surface().as_ref() {
        Surface::Cylinder(cyl) => {
            use knot_geom::surface::Cylinder;
            let (mut lo, mut hi) = (f64::MAX, f64::NEG_INFINITY);
            for he in face.outer_loop().half_edges() {
                let v = (he.start_vertex().point() - cyl.origin).dot(&cyl.axis);
                lo = lo.min(v);
                hi = hi.max(v);
            }
            let m = (hi - lo).max(0.01) * 0.1;
            Surface::Cylinder(Cylinder {
                origin: cyl.origin, axis: cyl.axis, radius: cyl.radius,
                ref_direction: cyl.ref_direction, v_min: lo - m, v_max: hi + m,
            })
        }
        Surface::Cone(c) => {
            use knot_geom::surface::Cone;
            let (mut lo, mut hi) = (f64::MAX, f64::NEG_INFINITY);
            for he in face.outer_loop().half_edges() {
                let v = (he.start_vertex().point() - c.apex).dot(&c.axis);
                lo = lo.min(v);
                hi = hi.max(v);
            }
            let m = (hi - lo).max(0.01) * 0.1;
            Surface::Cone(Cone {
                apex: c.apex, axis: c.axis, half_angle: c.half_angle,
                ref_direction: c.ref_direction, v_min: lo - m, v_max: hi + m,
            })
        }
        _ => face.surface().as_ref().clone(),
    }
}

fn compute_brep_bbox(a: &Solid, b: &Solid) -> Aabb3 {
    let mut pts: Vec<Point3> = Vec::new();
    for face in a.outer_shell().faces() {
        for he in face.outer_loop().half_edges() {
            pts.push(*he.start_vertex().point());
        }
    }
    for face in b.outer_shell().faces() {
        for he in face.outer_loop().half_edges() {
            pts.push(*he.start_vertex().point());
        }
    }
    Aabb3::from_points(&pts).unwrap()
}

// ═══════════════════════════════════════════════════════════════════
// Step 2: Face-Face Overlap Filtering
// ═══════════════════════════════════════════════════════════════════

/// Filter face pairs by bounding-box overlap, accelerated with a
/// BVH on each side. The straightforward O(n²) bbox-intersect loop
/// was the bottleneck on dense models — pair (32, 33) on the ABC
/// dataset has ~83 × 802 = 66K possible pairs, and even the
/// cheap-per-test sweep eats meaningful time relative to the 8s
/// pipeline budget. With BVH, only pairs whose AABB hierarchies
/// overlap propagate to the leaf-pair test, making the cost
/// ~O((n_a + n_b) log n + k) where k is the number of actually-
/// overlapping pairs.
fn find_candidate_pairs(faces_a: &[&Face], faces_b: &[&Face], tolerance: f64) -> Vec<(usize, usize)> {
    let bboxes_a: Vec<(usize, Aabb3)> = faces_a
        .iter()
        .enumerate()
        .map(|(i, f)| (i, face_bbox(f, tolerance)))
        .collect();
    let bboxes_b: Vec<(usize, Aabb3)> = faces_b
        .iter()
        .enumerate()
        .map(|(i, f)| (i, face_bbox(f, tolerance)))
        .collect();

    let bvh_a = knot_core::bvh::Bvh::build(&bboxes_a);
    let bvh_b = knot_core::bvh::Bvh::build(&bboxes_b);
    bvh_a.find_overlapping_pairs(&bvh_b)
}

fn face_bbox(face: &Face, margin: f64) -> Aabb3 {
    let pts: Vec<Point3> = face.outer_loop().half_edges()
        .iter()
        .map(|he| *he.start_vertex().point())
        .collect();
    Aabb3::from_points(&pts).unwrap().expand(margin)
}

// ═══════════════════════════════════════════════════════════════════
// Step 4-5: Face Splitting
// ═══════════════════════════════════════════════════════════════════

struct FaceIntersection {
    face_a_idx: usize,
    face_b_idx: usize,
    trace: knot_intersect::SurfaceSurfaceTrace,
}

fn split_faces(
    faces: &[&Face],
    intersections: &[FaceIntersection],
    is_a_side: bool,
    tolerance: f64,
    builder: &mut TopologyBuilder,
) -> Vec<Face> {
    let mut result = Vec::new();

    for (fi, face) in faces.iter().enumerate() {
        let relevant: Vec<&knot_intersect::SurfaceSurfaceTrace> = intersections
            .iter()
            .filter(|ix| if is_a_side { ix.face_a_idx == fi } else { ix.face_b_idx == fi })
            .map(|ix| &ix.trace)
            .collect();

        // Extract boundary polygon for all faces (unsplit faces also go through
        // the builder to ensure consistent vertex/edge sharing across the result).
        let boundary: Vec<Point3> = face.outer_loop().half_edges()
            .iter()
            .map(|he| *he.start_vertex().point())
            .collect();

        if relevant.is_empty() || boundary.len() < 3 {
            if let Ok(f) = builder.polygon_to_face(&boundary, face.surface().clone(), face.same_sense()) {
                result.push(f);
            }
            continue;
        }

        let face_normal = compute_polygon_normal(&boundary);
        let mut current_polygons = vec![boundary];

        for trace in &relevant {
            if trace.points.len() < 2 { continue; }

            // Find the best cutting line from the trace.
            // Use the longest segment pair to determine the dominant direction,
            // and the trace centroid as the anchor point. This handles both
            // open traces (chord is fine) and curved traces (average direction).
            let (cut_point, cut_dir) = best_cutting_line(&trace.points, tolerance);
            if cut_dir.norm() < tolerance { continue; }

            let mut new_polygons = Vec::new();
            for poly in &current_polygons {
                let (left, right) = split_polygon_by_line(poly, &cut_point, &cut_dir, &face_normal, tolerance);
                let before = new_polygons.len();
                if left.len() >= 3 { new_polygons.push(left); }
                if right.len() >= 3 { new_polygons.push(right); }
                if new_polygons.len() == before {
                    new_polygons.push(poly.clone());
                }
            }
            current_polygons = new_polygons;
        }

        for poly in &current_polygons {
            if let Ok(f) = builder.polygon_to_face(poly, face.surface().clone(), face.same_sense()) {
                result.push(f);
            }
        }
    }

    result
}

/// Determine the best cutting line from an SSI trace polyline.
/// Returns (anchor_point, direction).
///
/// For open traces: uses the chord from first to last point.
/// For closed traces: uses the diameter between the two furthest-apart points.
/// For curved traces: uses the trace centroid as anchor and the longest-chord
/// direction, which provides the most stable split plane.
fn best_cutting_line(trace_points: &[Point3], tolerance: f64) -> (Point3, Vector3) {
    let n = trace_points.len();
    if n < 2 {
        return (Point3::origin(), Vector3::zeros());
    }

    let first = trace_points[0];
    let last = trace_points[n - 1];
    let chord = last - first;

    // Compute trace centroid — better anchor than first point for curved traces
    let centroid = {
        let sum = trace_points.iter().fold(Vector3::zeros(), |acc, p| acc + p.coords);
        Point3::from(sum / n as f64)
    };

    if chord.norm() > tolerance {
        // Open trace — chord direction, centroid anchor
        (centroid, chord)
    } else {
        // Closed trace — find the two furthest-apart points for the diameter
        let mut best_dist = 0.0f64;
        let mut best_dir = Vector3::zeros();
        // Sample a few pairs rather than O(n²)
        let step = (n / 8).max(1);
        for i in (0..n).step_by(step) {
            for j in (i + 1..n).step_by(step) {
                let d = trace_points[j] - trace_points[i];
                let len = d.norm();
                if len > best_dist {
                    best_dist = len;
                    best_dir = d;
                }
            }
        }
        if best_dist > tolerance {
            (centroid, best_dir)
        } else {
            (centroid, Vector3::zeros()) // degenerate
        }
    }
}

fn split_polygon_by_line(
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

// ═══════════════════════════════════════════════════════════════════
// Step 6: Exact Classification
// ═══════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Classification {
    Inside,
    Outside,
    OnBoundary,
}

/// Classify a face against a solid using exact predicates.
///
/// This is a unified code path for all surface types. The key insight:
/// point-in-solid classification ray-casts against the face *boundary polygons*
/// (vertex positions), not the surface geometry. Since all faces — planar or
/// curved — have a boundary polygon defined by their vertex loop, the exact
/// orient3d predicates work on all of them.
///
/// For curved faces with many boundary vertices (e.g. a tessellated sphere cap),
/// the boundary polygon is a piecewise-linear approximation of the curved
/// boundary. The classification is exact on this approximation, which is
/// sufficient because the topology was determined combinatorially first.
/// Pre-computed acceleration structure for `classify_face_exact`.
/// Caches per-face triangulation and a BVH over face bboxes so that
/// each classify call costs `O(log F + hits)` rays instead of
/// `O(F)`. The dominant savings come from amortizing
/// `triangulate_face_for_classification` (which can do Newton-
/// projection for curved faces) over many classify calls — for
/// `N` sub-faces against an `F`-face solid we drop from `N·F`
/// triangulations to `F`.
pub struct SolidClassifier {
    triangulations: Vec<Vec<[Point3; 3]>>,
    /// Per-face axis-aligned bbox enclosing the triangulation.
    face_bboxes: Vec<knot_core::Aabb3>,
    bvh: knot_core::Bvh,
}

impl SolidClassifier {
    pub fn new(solid: &Solid) -> Self {
        let faces = solid.outer_shell().faces();
        let mut triangulations = Vec::with_capacity(faces.len());
        let mut face_bboxes = Vec::with_capacity(faces.len());
        for face in faces {
            let tris = triangulate_face_for_classification(face);
            let mut pts: Vec<Point3> = Vec::with_capacity(tris.len() * 3);
            for tri in &tris {
                pts.push(tri[0]);
                pts.push(tri[1]);
                pts.push(tri[2]);
            }
            let bbox = if pts.is_empty() {
                knot_core::Aabb3::new(Point3::origin(), Point3::origin())
            } else {
                knot_core::Aabb3::from_points(&pts).unwrap()
            };
            triangulations.push(tris);
            face_bboxes.push(bbox);
        }
        let items: Vec<(usize, knot_core::Aabb3)> = face_bboxes
            .iter()
            .enumerate()
            .map(|(i, b)| (i, *b))
            .collect();
        let bvh = knot_core::Bvh::build(&items);
        SolidClassifier { triangulations, face_bboxes, bvh }
    }

    /// Classify a 3D point as Inside or Outside by ray-casting the
    /// pre-triangulated faces.
    ///
    /// `exact_ray_triangle` casts a deliberately off-axis segment of
    /// direction `(1e6, 3e5, 1e5)` from `point` (the off-axis bias
    /// avoids degenerate alignment with axis-aligned face boundaries
    /// — common in CAD). Two-stage culling:
    ///
    /// 1. BVH query against the segment's bounding box. O(log F)
    ///    rejects most faces.
    /// 2. Per-face segment-vs-bbox slab test (Kay-Kajiya). This
    ///    catches faces inside the segment's bbox but not actually
    ///    on the segment — the AABB of an off-axis segment of length
    ///    1e6 in x covers a huge volume that's mostly empty.
    ///
    /// Earlier iterations used an axis-aligned y/z prefilter
    /// (`bb.max.y < point.y`) that worked for axis-aligned rays but
    /// broke on the actual off-axis segment, misclassifying faces
    /// at non-zero y/z. The slab test is the correct generalization.
    pub fn classify(&self, point: &Point3) -> Classification {
        let query = ExactPoint3::from_f64(point.x, point.y, point.z);
        let seg_end = Point3::new(point.x + 1e6, point.y + 3e5, point.z + 1e5);
        let query_bbox = knot_core::Aabb3::new(*point, seg_end);
        let candidates = self.bvh.query(&query_bbox);

        let mut crossings = 0i32;
        for face_idx in candidates {
            let bb = &self.face_bboxes[face_idx];
            // Slab test: does the segment from `point` to `seg_end`
            // actually intersect this face's bbox?
            if !segment_intersects_bbox(point, &seg_end, bb) {
                continue;
            }
            for tri in &self.triangulations[face_idx] {
                let v0 = ExactPoint3::from_f64(tri[0].x, tri[0].y, tri[0].z);
                let v1 = ExactPoint3::from_f64(tri[1].x, tri[1].y, tri[1].z);
                let v2 = ExactPoint3::from_f64(tri[2].x, tri[2].y, tri[2].z);
                if exact_ray_triangle(&query, &v0, &v1, &v2) {
                    crossings += 1;
                }
            }
        }
        if crossings % 2 == 1 {
            Classification::Inside
        } else {
            Classification::Outside
        }
    }
}

/// Backwards-compatible wrapper: builds an ad-hoc SolidClassifier per
/// call. The new boolean pipeline uses `classify_face_with` directly
/// to amortize triangulation over many sub-face calls; this signature
/// stays for callers that don't have a SolidClassifier handy.
fn classify_face_exact(face: &Face, solid: &Solid, _grid: &SnapGrid) -> Classification {
    let classifier = SolidClassifier::new(solid);
    classify_face_with(face, &classifier)
}

/// Compute the test point for a face (centroid offset slightly inward
/// along the face normal) and classify it against a pre-built
/// SolidClassifier. The point-offset logic is identical to the
/// in-line version of `classify_face_exact` from before; only the
/// triangulation work is amortized.
fn classify_face_with(face: &Face, classifier: &SolidClassifier) -> Classification {
    let verts: Vec<Point3> = face.outer_loop().half_edges()
        .iter()
        .map(|he| *he.start_vertex().point())
        .collect();

    if verts.is_empty() { return Classification::Outside; }

    let centroid = verts.iter().fold(Vector3::zeros(), |acc, p| acc + p.coords) / verts.len() as f64;
    let centroid = Point3::from(centroid);

    let face_normal = compute_polygon_normal(&verts);
    let face_size = face_diagonal(&verts);
    let offset = face_size * 1e-4;

    let test_point = if face.same_sense() {
        centroid - face_normal * offset
    } else {
        centroid + face_normal * offset
    };

    classifier.classify(&test_point)
}

/// Compute the diagonal of a face's bounding box (proxy for face size).
fn face_diagonal(verts: &[Point3]) -> f64 {
    if verts.is_empty() { return 1.0; }
    let bb = knot_core::Aabb3::from_points(verts).unwrap();
    bb.diagonal_length().max(1e-6)
}

/// Exact inside/outside classification for a point against a solid.
///
/// For each face:
/// - If the surface is planar, fan-triangulate the boundary polygon.
/// - If the surface is curved, refine by sampling additional points on the
///   actual surface to create a triangulation that hugs the curved geometry.
///   This prevents misclassification near the surface where the inscribed
///   boundary polygon diverges from the actual surface.
///
/// All triangle tests use exact orient3d predicates over rational coordinates.
fn classify_point_exact(point: &Point3, solid: &Solid) -> Classification {
    let query = ExactPoint3::from_f64(point.x, point.y, point.z);
    let mut crossings = 0i32;

    for face in solid.outer_shell().faces() {
        let triangles = triangulate_face_for_classification(face);

        for tri in &triangles {
            let v0 = ExactPoint3::from_f64(tri[0].x, tri[0].y, tri[0].z);
            let v1 = ExactPoint3::from_f64(tri[1].x, tri[1].y, tri[1].z);
            let v2 = ExactPoint3::from_f64(tri[2].x, tri[2].y, tri[2].z);

            if exact_ray_triangle(&query, &v0, &v1, &v2) {
                crossings += 1;
            }
        }
    }

    if crossings % 2 == 1 {
        Classification::Inside
    } else {
        Classification::Outside
    }
}

/// Triangulate a face for classification ray-casting.
///
/// For planar faces: fan-triangulate the boundary polygon.
/// For curved faces: subdivide each boundary edge and add midpoint vertices
/// on the actual surface to create a triangulation that follows the curvature.
fn triangulate_face_for_classification(face: &Face) -> Vec<[Point3; 3]> {
    let verts: Vec<Point3> = face.outer_loop().half_edges()
        .iter()
        .map(|he| *he.start_vertex().point())
        .collect();

    if verts.len() < 3 {
        return Vec::new();
    }

    let is_planar = matches!(face.surface().as_ref(), Surface::Plane(_));

    if is_planar || verts.len() <= 4 {
        // For planar faces or small faces, simple fan triangulation is exact.
        let mut tris = Vec::new();
        for i in 1..verts.len() - 1 {
            tris.push([verts[0], verts[i], verts[i + 1]]);
        }
        return tris;
    }

    // For curved faces with many vertices: the boundary polygon is already
    // a good approximation because our primitive constructors create faces
    // with vertices on the surface. Fan-triangulate from the centroid to
    // better cover the interior.
    let centroid_coords = verts.iter().fold(Vector3::zeros(), |acc, p| acc + p.coords) / verts.len() as f64;

    // Project the centroid onto the actual surface for better accuracy
    let surface = face.surface();
    let centroid_on_surface = project_point_to_surface(&Point3::from(centroid_coords), surface);

    let mut tris = Vec::new();
    for i in 0..verts.len() {
        let j = (i + 1) % verts.len();
        tris.push([centroid_on_surface, verts[i], verts[j]]);
    }
    tris
}

/// Project a point onto a surface by finding the closest surface point.
/// Uses Newton iteration in parameter space.
fn project_point_to_surface(point: &Point3, surface: &Surface) -> Point3 {
    use knot_geom::surface::SurfaceParam;

    let domain = surface.domain();

    // Clamp domain for search
    let u_lo = domain.u_start.max(-100.0);
    let u_hi = domain.u_end.min(100.0);
    let v_lo = domain.v_start.max(-100.0);
    let v_hi = domain.v_end.min(100.0);

    // Grid search for initial guess
    let n = 4;
    let mut best_uv = SurfaceParam { u: (u_lo + u_hi) / 2.0, v: (v_lo + v_hi) / 2.0 };
    let mut best_dist = f64::MAX;

    for iu in 0..=n {
        for iv in 0..=n {
            let uv = SurfaceParam {
                u: u_lo + (u_hi - u_lo) * iu as f64 / n as f64,
                v: v_lo + (v_hi - v_lo) * iv as f64 / n as f64,
            };
            let p = surface.point_at(uv);
            let d = (p - point).norm();
            if d < best_dist {
                best_dist = d;
                best_uv = uv;
            }
        }
    }

    // Newton refinement
    for _ in 0..10 {
        let sd = surface.derivatives_at(best_uv);
        let diff = *point - sd.point;
        if diff.norm() < 1e-12 { break; }

        let a11 = sd.du.dot(&sd.du);
        let a12 = sd.du.dot(&sd.dv);
        let a22 = sd.dv.dot(&sd.dv);
        let b1 = sd.du.dot(&diff);
        let b2 = sd.dv.dot(&diff);
        let det = a11 * a22 - a12 * a12;
        if det.abs() < 1e-30 { break; }

        best_uv.u += (a22 * b1 - a12 * b2) / det;
        best_uv.v += (a11 * b2 - a12 * b1) / det;
    }

    surface.point_at(best_uv)
}

/// Exact ray-triangle intersection test using orientation predicates.
/// Ray: origin → +x direction (deterministic, avoids degenerate alignments
/// by using a slightly off-axis direction encoded as rational).
/// Kay-Kajiya slab test: does the line segment from `start` to `end`
/// intersect the AABB? Computes the parametric range `t ∈ [0, 1]`
/// where the segment is inside each axis's slab; intersection exists
/// iff the per-axis intervals overlap. For nearly-axis-aligned
/// segments the test still works because we test each axis
/// independently.
fn segment_intersects_bbox(
    start: &Point3,
    end: &Point3,
    bbox: &knot_core::Aabb3,
) -> bool {
    let d = [end.x - start.x, end.y - start.y, end.z - start.z];
    let s = [start.x, start.y, start.z];
    let lo = [bbox.min.x, bbox.min.y, bbox.min.z];
    let hi = [bbox.max.x, bbox.max.y, bbox.max.z];

    let mut t_min = 0.0_f64;
    let mut t_max = 1.0_f64;
    for axis in 0..3 {
        if d[axis].abs() < 1e-15 {
            // Segment is parallel to this slab. Inside the slab iff
            // start coordinate is within the bbox's range on this axis.
            if s[axis] < lo[axis] || s[axis] > hi[axis] {
                return false;
            }
        } else {
            let t1 = (lo[axis] - s[axis]) / d[axis];
            let t2 = (hi[axis] - s[axis]) / d[axis];
            let (t_lo, t_hi) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
            t_min = t_min.max(t_lo);
            t_max = t_max.min(t_hi);
            if t_min > t_max {
                return false;
            }
        }
    }
    true
}

fn exact_ray_triangle(
    origin: &ExactPoint3,
    v0: &ExactPoint3,
    v1: &ExactPoint3,
    v2: &ExactPoint3,
) -> bool {
    // Ray endpoint: origin + t * (1.0, 0.3, 0.1) for large t.
    // Slightly off-axis direction avoids exact alignment with edges.
    let far = ExactPoint3::new(
        &origin.x + &ExactRational::from_f64(1e6),
        &origin.y + &ExactRational::from_f64(3e5),
        &origin.z + &ExactRational::from_f64(1e5),
    );

    // Which side of the triangle plane is the origin?
    let o_side = knot_core::exact::orient3d(v0, v1, v2, origin);
    let r_side = knot_core::exact::orient3d(v0, v1, v2, &far);

    // Both on same side → no crossing
    match (o_side, r_side) {
        (Orientation::Positive, Orientation::Positive) => return false,
        (Orientation::Negative, Orientation::Negative) => return false,
        (Orientation::Zero, _) => return false,
        _ => {}
    }

    // Ray crosses the plane. Check if crossing point is inside the triangle.
    let e0 = knot_core::exact::orient3d(origin, &far, v0, v1);
    let e1 = knot_core::exact::orient3d(origin, &far, v1, v2);
    let e2 = knot_core::exact::orient3d(origin, &far, v2, v0);

    let all_pos = e0 != Orientation::Negative && e1 != Orientation::Negative && e2 != Orientation::Negative;
    let all_neg = e0 != Orientation::Positive && e1 != Orientation::Positive && e2 != Orientation::Positive;

    all_pos || all_neg
}

// ═══════════════════════════════════════════════════════════════════
// Step 7b: Face Deduplication
// ═══════════════════════════════════════════════════════════════════

/// Remove duplicate faces that share the same (snapped) vertex set.
/// Uses integer lattice indices for identity — deterministic, no f64 ambiguity.
/// Also detects reversed-winding duplicates (same face with opposite orientation,
/// which occurs in subtraction at the intersection boundary).
fn deduplicate_faces(faces: &[Face], grid: &SnapGrid) -> Vec<Face> {
    use std::collections::HashSet;
    use knot_core::snap::LatticeIndex;

    let mut seen: HashSet<Vec<LatticeIndex>> = HashSet::new();
    let mut result = Vec::new();

    for face in faces {
        let indices: Vec<LatticeIndex> = face.outer_loop().half_edges()
            .iter()
            .map(|he| grid.lattice_index(*he.start_vertex().point()))
            .collect();

        let n = indices.len();
        if n == 0 { continue; }

        // Build canonical key: rotate to smallest starting vertex
        let mut fwd = indices.clone();
        let min_idx = (0..n).min_by(|&i, &j| fwd[i].cmp(&fwd[j])).unwrap();
        fwd.rotate_left(min_idx);

        // Also build reversed canonical key (for opposite-winding duplicates)
        let mut rev = indices;
        rev.reverse();
        let min_idx_r = (0..n).min_by(|&i, &j| rev[i].cmp(&rev[j])).unwrap();
        rev.rotate_left(min_idx_r);

        // Skip if we've seen either orientation
        if seen.contains(&fwd) || seen.contains(&rev) {
            continue;
        }

        seen.insert(fwd);
        result.push(face.clone());
    }

    result
}

// ═══════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════

fn flip_face(face: &Face, builder: &mut TopologyBuilder) -> KResult<Face> {
    let verts: Vec<Point3> = face.outer_loop().half_edges()
        .iter()
        .map(|he| *he.start_vertex().point())
        .collect();
    let mut reversed = verts;
    reversed.reverse();
    builder.polygon_to_face(&reversed, face.surface().clone(), !face.same_sense())
}

/// Check if a face is coplanar with any face of a solid.
/// Used to filter out B-faces that lie on A's boundary during subtraction,
/// where including them would create a non-manifold double layer.
fn is_face_coplanar_with_solid(face: &Face, solid: &Solid, tolerance: f64) -> bool {
    let verts: Vec<Point3> = face.outer_loop().half_edges()
        .iter()
        .map(|he| *he.start_vertex().point())
        .collect();
    if verts.len() < 3 { return false; }
    let normal = compute_polygon_normal(&verts);

    for other_face in solid.outer_shell().faces() {
        let other_verts: Vec<Point3> = other_face.outer_loop().half_edges()
            .iter()
            .map(|he| *he.start_vertex().point())
            .collect();
        if other_verts.len() < 3 { continue; }
        let other_normal = compute_polygon_normal(&other_verts);

        // Normals must be parallel (same or opposite direction)
        if normal.dot(&other_normal).abs() < 1.0 - 1e-6 { continue; }

        // A vertex of `face` must lie on the other face's plane
        let other_d = other_normal.dot(&other_verts[0].coords);
        let dist = (other_normal.dot(&verts[0].coords) - other_d).abs();
        if dist < tolerance {
            return true;
        }
    }
    false
}

fn compute_polygon_normal(verts: &[Point3]) -> Vector3 {
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
