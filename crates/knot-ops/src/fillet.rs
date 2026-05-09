//! Fillet and chamfer operations on BRep edges.
//!
//! V1 scope: constant-radius fillet and constant-distance chamfer on
//! straight edges between two planar faces.

use std::collections::HashMap;
use std::sync::Arc;

use knot_core::snap::LatticeIndex;
use knot_core::{Aabb3, ErrorCode, KResult, KernelError, SnapGrid};
use knot_geom::curve::{Curve, CircularArc, LineSeg};
use knot_geom::surface::{Cylinder, Plane, Surface};
use knot_geom::{Point3, Vector3};
use knot_topo::*;

use crate::topo_builder::{line_he, TopologyBuilder};

// ═══════════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════════

/// Apply a constant-radius fillet to the specified edges.
///
/// Each edge is identified by its two endpoint positions. Both adjacent
/// faces must be planar and the edge must be a straight line.
pub fn fillet(brep: &BRep, edge_points: &[(Point3, Point3)], radius: f64) -> KResult<BRep> {
    if radius <= 0.0 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: "fillet radius must be positive".into(),
        });
    }
    blend_edges(brep, edge_points, BlendKind::Fillet { radius })
}

/// Apply a constant-distance chamfer to the specified edges.
///
/// Each edge is identified by its two endpoint positions. Both adjacent
/// faces must be planar and the edge must be a straight line.
pub fn chamfer(brep: &BRep, edge_points: &[(Point3, Point3)], distance: f64) -> KResult<BRep> {
    if distance <= 0.0 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: "chamfer distance must be positive".into(),
        });
    }
    blend_edges(brep, edge_points, BlendKind::Chamfer { distance })
}

// ═══════════════════════════════════════════════════════════════════
// Core algorithm
// ═══════════════════════════════════════════════════════════════════

#[derive(Clone, Copy)]
enum BlendKind {
    Fillet { radius: f64 },
    Chamfer { distance: f64 },
}

/// Sorted lattice-index pair identifying an edge.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct EdgeKey(LatticeIndex, LatticeIndex);

impl EdgeKey {
    fn new(a: LatticeIndex, b: LatticeIndex) -> Self {
        if a <= b { Self(a, b) } else { Self(b, a) }
    }
}

/// Two faces sharing an edge.
struct EdgeAdj {
    face_a: usize,
    face_b: usize,
}

/// Result of computing the blend geometry for one edge.
struct BlendResult {
    key: EdgeKey,
    /// Trim line on face A: (start-side point, end-side point)
    trim_a: (Point3, Point3),
    /// Trim line on face B: (start-side point, end-side point)
    trim_b: (Point3, Point3),
    face_a: usize,
    face_b: usize,
    /// For fillet: cylinder center at p0 end
    center_start: Point3,
    /// For fillet: cylinder center at p1 end
    center_end: Point3,
    /// Sweep angle (only used for fillet)
    sweep: f64,
    /// The fillet radius (only used for fillet)
    radius: f64,
    /// Edge direction (p0 → p1), normalized
    edge_dir: Vector3,
    /// Edge start point (lower lattice)
    p0: Point3,
    /// Edge end point (higher lattice)
    p1: Point3,
    /// Offset direction into face A
    offset_a: Vector3,
}

fn blend_edges(brep: &BRep, edge_points: &[(Point3, Point3)], kind: BlendKind) -> KResult<BRep> {
    let solid = brep.single_solid().ok_or_else(|| KernelError::InvalidInput {
        code: ErrorCode::UnsupportedConfiguration,
        detail: "fillet/chamfer requires single-solid BRep".into(),
    })?;
    let shell = solid.outer_shell();

    // Model-level snap grid
    let bbox = shell_bbox(shell);
    let grid = SnapGrid::from_bbox_diagonal(bbox.diagonal_length(), 1e-9);

    // Step 1: Build adjacency map
    let adjacency = build_adjacency(shell, &grid)?;

    // Step 2: Resolve selected edges and compute blend geometry
    let mut blends = Vec::new();
    for (pa, pb) in edge_points {
        let key = EdgeKey::new(grid.lattice_index(*pa), grid.lattice_index(*pb));
        let adj = adjacency.get(&key).ok_or_else(|| KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: format!("edge ({:?} → {:?}) not found in shell", pa, pb),
        })?;
        blends.push(compute_blend(shell, &key, adj, kind, &grid)?);
    }

    // Step 3: Build vertex replacement map
    // Key: (face_idx, vertex_lattice, is_start_of_next_he) → replacement point
    // When a filleted edge is removed from a face's loop, the vertices at its
    // endpoints shift to the trim points. For the predecessor HE, its end vertex
    // moves. For the successor HE, its start vertex moves.
    let filleted_keys: HashMap<EdgeKey, usize> = blends.iter()
        .enumerate()
        .map(|(i, b)| (b.key, i))
        .collect();

    // Step 4: Reconstruct faces
    let mut builder = TopologyBuilder::new(grid);
    let mut new_faces: Vec<Face> = Vec::new();

    for (fi, face) in shell.faces().iter().enumerate() {
        let hes = face.outer_loop().half_edges();
        let mut new_hes: Vec<HalfEdge> = Vec::new();

        for (hi, he) in hes.iter().enumerate() {
            let s_li = grid.lattice_index(*he.start_vertex().point());
            let e_li = grid.lattice_index(*he.end_vertex().point());
            let he_key = EdgeKey::new(s_li, e_li);

            if let Some(&bi) = filleted_keys.get(&he_key) {
                // This edge is being filleted — replace with trim line
                let b = &blends[bi];
                let (ts, te) = if fi == b.face_a { b.trim_a } else { b.trim_b };
                let vs = builder.vertex(ts);
                let ve = builder.vertex(te);
                new_hes.push(line_he(&vs, &ve));
            } else {
                // Check if start/end vertices need adjustment from adjacent fillets
                let prev_idx = if hi == 0 { hes.len() - 1 } else { hi - 1 };
                let next_idx = (hi + 1) % hes.len();

                let prev_s = grid.lattice_index(*hes[prev_idx].start_vertex().point());
                let prev_e = grid.lattice_index(*hes[prev_idx].end_vertex().point());
                let prev_key = EdgeKey::new(prev_s, prev_e);

                let next_s = grid.lattice_index(*hes[next_idx].start_vertex().point());
                let next_e = grid.lattice_index(*hes[next_idx].end_vertex().point());
                let next_key = EdgeKey::new(next_s, next_e);

                // Start vertex: if the PREVIOUS edge was filleted, this edge's
                // start vertex moves to that fillet's trim end on this face.
                let start_pt = if let Some(&bi) = filleted_keys.get(&prev_key) {
                    let b = &blends[bi];
                    let (_, te) = if fi == b.face_a { b.trim_a } else { b.trim_b };
                    te
                } else {
                    *he.start_vertex().point()
                };

                // End vertex: if the NEXT edge was filleted, this edge's
                // end vertex moves to that fillet's trim start on this face.
                let end_pt = if let Some(&bi) = filleted_keys.get(&next_key) {
                    let b = &blends[bi];
                    let (ts, _) = if fi == b.face_a { b.trim_a } else { b.trim_b };
                    ts
                } else {
                    *he.end_vertex().point()
                };

                let vs = builder.vertex(start_pt);
                let ve = builder.vertex(end_pt);
                new_hes.push(line_he(&vs, &ve));
            }
        }

        let loop_ = Loop::new(new_hes, true)?;
        new_faces.push(Face::new(face.surface().clone(), loop_, vec![], face.same_sense())?);
    }

    // Step 5: Add blend faces
    for b in &blends {
        new_faces.push(build_blend_face(b, kind, &mut builder)?);
    }

    // Step 6: Assemble and validate
    let shell = Shell::new(new_faces, true)?;
    let solid = Solid::new(shell, vec![])?;
    let result = BRep::new(vec![solid])?;
    result.validate()?;
    Ok(result)
}

// ═══════════════════════════════════════════════════════════════════
// Adjacency
// ═══════════════════════════════════════════════════════════════════

fn build_adjacency(shell: &Shell, grid: &SnapGrid) -> KResult<HashMap<EdgeKey, EdgeAdj>> {
    let mut map: HashMap<EdgeKey, Vec<usize>> = HashMap::new();

    for (fi, face) in shell.faces().iter().enumerate() {
        for he in face.outer_loop().half_edges() {
            let s = grid.lattice_index(*he.start_vertex().point());
            let e = grid.lattice_index(*he.end_vertex().point());
            let key = EdgeKey::new(s, e);
            map.entry(key).or_default().push(fi);
        }
    }

    let mut result = HashMap::new();
    for (key, faces) in map {
        if faces.len() == 2 {
            result.insert(key, EdgeAdj { face_a: faces[0], face_b: faces[1] });
        }
        // edges with != 2 uses are boundary or non-manifold — skip silently
    }
    Ok(result)
}

// ═══════════════════════════════════════════════════════════════════
// Blend geometry computation
// ═══════════════════════════════════════════════════════════════════

fn compute_blend(
    shell: &Shell,
    key: &EdgeKey,
    adj: &EdgeAdj,
    kind: BlendKind,
    grid: &SnapGrid,
) -> KResult<BlendResult> {
    let face_a = &shell.faces()[adj.face_a];
    let face_b = &shell.faces()[adj.face_b];

    let plane_a = extract_plane(face_a, "face A")?;
    let plane_b = extract_plane(face_b, "face B")?;

    let p0 = grid.lattice_to_point(key.0);
    let p1 = grid.lattice_to_point(key.1);
    let edge_vec = p1 - p0;
    let edge_len = edge_vec.norm();
    if edge_len < 1e-12 {
        return Err(KernelError::InvalidGeometry {
            code: ErrorCode::DegenerateCurve,
            detail: "zero-length edge cannot be filleted".into(),
        });
    }
    let edge_dir = edge_vec / edge_len;

    // Face outward normals (accounting for same_sense)
    let n_a = if face_a.same_sense() { plane_a.normal } else { -plane_a.normal };
    let n_b = if face_b.same_sense() { plane_b.normal } else { -plane_b.normal };

    // Offset directions: perpendicular to edge, in face plane, pointing inward
    let mut offset_a = edge_dir.cross(&n_a);
    let mut offset_b = edge_dir.cross(&n_b);

    let edge_mid = Point3::from((p0.coords + p1.coords) / 2.0);
    let centroid_a = face_centroid(face_a);
    if offset_a.dot(&(centroid_a - edge_mid)) < 0.0 { offset_a = -offset_a; }
    let centroid_b = face_centroid(face_b);
    if offset_b.dot(&(centroid_b - edge_mid)) < 0.0 { offset_b = -offset_b; }

    let oa_len = offset_a.norm();
    let ob_len = offset_b.norm();
    if oa_len < 1e-12 || ob_len < 1e-12 {
        return Err(KernelError::InvalidGeometry {
            code: ErrorCode::DegenerateSurface,
            detail: "edge is parallel to face normal (degenerate)".into(),
        });
    }
    offset_a /= oa_len;
    offset_b /= ob_len;

    // Angle between the two offset directions (the dihedral opening)
    let cos_angle = offset_a.dot(&offset_b).clamp(-1.0, 1.0);
    let angle_between = cos_angle.acos();
    let half = angle_between / 2.0;

    if half.abs() < 1e-10 || (std::f64::consts::PI - half).abs() < 1e-10 {
        return Err(KernelError::InvalidGeometry {
            code: ErrorCode::DegenerateSurface,
            detail: "coplanar faces cannot be filleted".into(),
        });
    }

    match kind {
        BlendKind::Chamfer { distance } => {
            let v0 = p0 + distance * offset_a;
            let v1 = p1 + distance * offset_a;
            let v2 = p1 + distance * offset_b;
            let v3 = p0 + distance * offset_b;

            Ok(BlendResult {
                key: *key,
                trim_a: (v0, v1),
                trim_b: (v3, v2),
                face_a: adj.face_a,
                face_b: adj.face_b,
                center_start: edge_mid, // unused for chamfer
                center_end: edge_mid,
                sweep: 0.0,
                radius: 0.0,
                edge_dir,
                p0, p1,
                offset_a,
            })
        }
        BlendKind::Fillet { radius } => {
            let trim_dist = radius / half.tan();
            let center_offset = radius / half.sin();

            let v0 = p0 + trim_dist * offset_a;
            let v1 = p1 + trim_dist * offset_a;
            let v2 = p1 + trim_dist * offset_b;
            let v3 = p0 + trim_dist * offset_b;

            let center_dir = (offset_a + offset_b).normalize();
            let c0 = p0 + center_offset * center_dir;
            let c1 = p1 + center_offset * center_dir;

            let sweep = std::f64::consts::PI - angle_between;

            Ok(BlendResult {
                key: *key,
                trim_a: (v0, v1),
                trim_b: (v3, v2),
                face_a: adj.face_a,
                face_b: adj.face_b,
                center_start: c0,
                center_end: c1,
                sweep,
                radius,
                edge_dir,
                p0, p1,
                offset_a,
            })
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Blend face construction
// ═══════════════════════════════════════════════════════════════════

fn build_blend_face(
    b: &BlendResult,
    kind: BlendKind,
    builder: &mut TopologyBuilder,
) -> KResult<Face> {
    let (ta0, ta1) = b.trim_a; // trim on face A
    let (tb0, tb1) = b.trim_b; // trim on face B

    let va0 = builder.vertex(ta0);
    let va1 = builder.vertex(ta1);
    let vb0 = builder.vertex(tb0);
    let vb1 = builder.vertex(tb1);

    match kind {
        BlendKind::Chamfer { .. } => {
            // Flat bevel — all line edges
            let he0 = line_he(&va0, &va1);
            let he1 = line_he(&va1, &vb1);
            let he2 = line_he(&vb1, &vb0);
            let he3 = line_he(&vb0, &va0);

            let loop_ = Loop::new(vec![he0, he1, he2, he3], true)?;

            let u = (ta1 - ta0).normalize();
            let v = (tb0 - ta0).normalize();
            let normal = u.cross(&v);
            let normal = if normal.norm() > 1e-12 { normal.normalize() } else { Vector3::z() };

            let surface = Arc::new(Surface::Plane(Plane::new(ta0, normal)));
            Face::new(surface, loop_, vec![], true)
        }
        BlendKind::Fillet { .. } => {
            // Cylindrical fillet — line edges along trim, arc edges at cross-sections.
            // Use SNAPPED vertex positions to compute arc parameters so that
            // arc.point_at(t_start) matches the vertex exactly after snap-rounding.
            let va0_pt = *va0.point();
            let va1_pt = *va1.point();
            let vb0_pt = *vb0.point();
            let vb1_pt = *vb1.point();

            // Edge 0: trim line on face A (va0 → va1)
            let he0 = line_he(&va0, &va1);

            // Edge 1: arc at p1 end (va1 → vb1)
            let (arc1, sweep1) = make_arc_between(
                b.center_end, b.edge_dir, va1_pt, vb1_pt,
            );
            let curve1 = Arc::new(Curve::CircularArc(arc1));
            let edge1 = Arc::new(Edge::new(va1.clone(), vb1.clone(), curve1, 0.0, sweep1));
            let he1 = HalfEdge::new(edge1, true);

            // Edge 2: trim line on face B reversed (vb1 → vb0)
            let he2 = line_he(&vb1, &vb0);

            // Edge 3: arc at p0 end (vb0 → va0)
            let (arc3, sweep3) = make_arc_between(
                b.center_start, -b.edge_dir, vb0_pt, va0_pt,
            );
            let curve3 = Arc::new(Curve::CircularArc(arc3));
            let edge3 = Arc::new(Edge::new(vb0.clone(), va0.clone(), curve3, 0.0, sweep3));
            let he3 = HalfEdge::new(edge3, true);

            let loop_ = Loop::new(vec![he0, he1, he2, he3], true)?;

            // Cylinder surface — use snapped positions for consistency
            let cyl_ref = (va0_pt - b.center_start).normalize();
            let cyl_r = (va0_pt - b.center_start).norm();
            let cyl = Cylinder {
                origin: b.center_start,
                axis: b.edge_dir,
                radius: cyl_r,
                ref_direction: cyl_ref,
                v_min: 0.0,
                v_max: (b.center_end - b.center_start).norm(),
            };
            let surface = Arc::new(Surface::Cylinder(cyl));
            // Cylinder normal points outward from axis. For a convex fillet,
            // the fillet surface is concave (curves toward the solid), so its
            // outward face normal opposes the cylinder's geometric normal.
            Face::new(surface, loop_, vec![], false)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════

/// Construct a CircularArc from center, normal, and the two endpoint positions.
/// Computes ref_direction, radius, and sweep angle from the actual positions
/// so that `arc.point_at(0) == p_start` and `arc.point_at(sweep) == p_end` exactly.
fn make_arc_between(
    center: Point3,
    normal: Vector3,
    p_start: Point3,
    p_end: Point3,
) -> (CircularArc, f64) {
    let r_vec = p_start - center;
    let radius = r_vec.norm();
    let ref_dir = if radius > 1e-15 { r_vec / radius } else { Vector3::x() };
    let binormal = normal.cross(&ref_dir);

    let end_vec = p_end - center;
    let cos_a = end_vec.dot(&ref_dir) / radius.max(1e-15);
    let sin_a = end_vec.dot(&binormal) / radius.max(1e-15);
    let mut sweep = sin_a.atan2(cos_a);
    if sweep < 0.0 { sweep += std::f64::consts::TAU; }

    let arc = CircularArc {
        center,
        normal,
        radius,
        ref_direction: ref_dir,
        start_angle: 0.0,
        end_angle: sweep,
    };
    (arc, sweep)
}

fn extract_plane<'a>(face: &'a Face, label: &str) -> KResult<&'a Plane> {
    match face.surface().as_ref() {
        Surface::Plane(p) => Ok(p),
        _ => Err(KernelError::OperationFailed {
            code: ErrorCode::UnsupportedConfiguration,
            detail: format!("{} is not planar — only planar faces supported for fillet/chamfer", label),
        }),
    }
}

fn face_centroid(face: &Face) -> Point3 {
    let pts: Vec<Point3> = face.outer_loop().half_edges()
        .iter()
        .map(|he| *he.start_vertex().point())
        .collect();
    let n = pts.len() as f64;
    let sum = pts.iter().fold(Vector3::zeros(), |acc, p| acc + p.coords);
    Point3::from(sum / n)
}

fn shell_bbox(shell: &Shell) -> Aabb3 {
    let pts: Vec<Point3> = shell.faces().iter()
        .flat_map(|f| f.outer_loop().half_edges().iter().map(|he| *he.start_vertex().point()))
        .collect();
    Aabb3::from_points(&pts).unwrap()
}
