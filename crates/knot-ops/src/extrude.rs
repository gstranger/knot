//! Feature operations: extrude and revolve.

use std::f64::consts::TAU;
use std::sync::Arc;

use knot_core::{ErrorCode, KResult, KernelError};
use knot_geom::curve::{Curve, LineSeg};
use knot_geom::surface::{Plane, Surface};
use knot_geom::transform;
use knot_geom::{Point3, Vector3};
use knot_topo::*;
use crate::topo_builder::line_he;

/// Extrude a BRep profile along a direction vector.
///
/// The first face of the profile is swept by `direction * distance` to produce
/// a closed prismatic solid.  Inner loops (holes) in the profile produce
/// tunnels through the extruded solid.
pub fn extrude_linear(profile: &BRep, direction: Vector3, distance: f64) -> KResult<BRep> {
    if direction.norm() < 1e-15 || distance.abs() < 1e-15 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: "extrude direction or distance is near zero".into(),
        });
    }

    let dir = direction.normalize();
    let offset = dir * distance;

    let face = first_face(profile)?;
    let mut pts = loop_points(face.outer_loop());
    let nv = pts.len();
    if nv < 3 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: "profile must have at least 3 vertices".into(),
        });
    }

    // Ensure CCW winding when viewed from the extrusion direction.
    let ln = newell_normal(&pts);
    if ln.norm() < 1e-15 {
        return Err(KernelError::InvalidGeometry {
            code: ErrorCode::DegenerateCurve,
            detail: "profile is degenerate (collinear vertices)".into(),
        });
    }
    if ln.dot(&dir) < 0.0 {
        pts.reverse();
    }

    // Collect inner loops from the profile face.
    let reversed_outer = ln.dot(&dir) < 0.0;
    let inner_loops: Vec<Vec<Point3>> = face
        .inner_loops()
        .iter()
        .filter(|il| il.half_edges().len() >= 3)
        .map(|il| {
            let mut ip: Vec<Point3> =
                il.half_edges().iter().map(|he| *he.start_vertex().point()).collect();
            if reversed_outer {
                ip.reverse();
            }
            ip
        })
        .collect();

    let bot: Vec<Arc<Vertex>> = pts.iter().map(|p| Arc::new(Vertex::new(*p))).collect();
    let top: Vec<Arc<Vertex>> =
        pts.iter().map(|p| Arc::new(Vertex::new(*p + offset))).collect();

    let inner_bot: Vec<Vec<Arc<Vertex>>> = inner_loops
        .iter()
        .map(|il| il.iter().map(|p| Arc::new(Vertex::new(*p))).collect())
        .collect();
    let inner_top: Vec<Vec<Arc<Vertex>>> = inner_loops
        .iter()
        .map(|il| il.iter().map(|p| Arc::new(Vertex::new(*p + offset))).collect())
        .collect();

    let mut faces = Vec::new();

    // Outer side quads.
    for i in 0..nv {
        let j = (i + 1) % nv;
        let edge_dir = *bot[j].point() - *bot[i].point();
        let normal = safe_normalize(edge_dir.cross(&offset));
        let edges = vec![
            line_he(&bot[i], &bot[j]),
            line_he(&bot[j], &top[j]),
            line_he(&top[j], &top[i]),
            line_he(&top[i], &bot[i]),
        ];
        let lp = Loop::new(edges, true)?;
        let surf = Arc::new(Surface::Plane(Plane::new(*bot[i].point(), normal)));
        faces.push(Face::new(surf, lp, vec![], true)?);
    }

    // Inner side quads (tunnel walls).
    for (ib, it) in inner_bot.iter().zip(inner_top.iter()) {
        let m = ib.len();
        for i in 0..m {
            let j = (i + 1) % m;
            let edge_dir = *ib[j].point() - *ib[i].point();
            let normal = safe_normalize(edge_dir.cross(&offset));
            let edges = vec![
                line_he(&ib[i], &ib[j]),
                line_he(&ib[j], &it[j]),
                line_he(&it[j], &it[i]),
                line_he(&it[i], &ib[i]),
            ];
            let lp = Loop::new(edges, true)?;
            let surf = Arc::new(Surface::Plane(Plane::new(*ib[i].point(), normal)));
            faces.push(Face::new(surf, lp, vec![], true)?);
        }
    }

    // Bottom cap (reversed outer + reversed inner loops).
    let bot_outer_edges: Vec<HalfEdge> = (0..nv)
        .rev()
        .map(|i| {
            let j = if i == 0 { nv - 1 } else { i - 1 };
            line_he(&bot[i], &bot[j])
        })
        .collect();
    let bot_outer_loop = Loop::new(bot_outer_edges, true)?;
    let bot_inner_loops: Vec<Loop> = inner_bot
        .iter()
        .map(|ib| {
            let m = ib.len();
            let edges: Vec<HalfEdge> = (0..m)
                .rev()
                .map(|i| {
                    let j = if i == 0 { m - 1 } else { i - 1 };
                    line_he(&ib[i], &ib[j])
                })
                .collect();
            Loop::new(edges, false)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let bot_surf = Arc::new(Surface::Plane(Plane::new(*bot[0].point(), -dir)));
    faces.push(Face::new(bot_surf, bot_outer_loop, bot_inner_loops, true)?);

    // Top cap (forward outer + forward inner loops).
    let top_outer_edges: Vec<HalfEdge> =
        (0..nv).map(|i| line_he(&top[i], &top[(i + 1) % nv])).collect();
    let top_outer_loop = Loop::new(top_outer_edges, true)?;
    let top_inner_loops: Vec<Loop> = inner_top
        .iter()
        .map(|it| {
            let m = it.len();
            let edges: Vec<HalfEdge> =
                (0..m).map(|i| line_he(&it[i], &it[(i + 1) % m])).collect();
            Loop::new(edges, false)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let top_surf = Arc::new(Surface::Plane(Plane::new(*top[0].point(), dir)));
    faces.push(Face::new(top_surf, top_outer_loop, top_inner_loops, true)?);

    let shell = Shell::new(faces, true)?;
    let solid = Solid::new(shell, vec![])?;
    BRep::new(vec![solid])
}

/// Revolve a BRep profile around an axis.
///
/// The first face of the profile is rotated around the axis defined by
/// `axis_origin` and `axis_direction` through `angle` radians.  A positive
/// angle rotates counter-clockwise when viewed from the positive-axis side.
/// Pass `2π` for a closed solid of revolution; smaller angles produce a wedge
/// with planar cap faces.
///
/// Only the outer loop is used.  Vertices on the revolve axis produce
/// triangular (conical) faces rather than quads.
pub fn revolve(
    profile: &BRep,
    axis_origin: Point3,
    axis_direction: Vector3,
    angle: f64,
) -> KResult<BRep> {
    if axis_direction.norm() < 1e-15 || angle.abs() < 1e-15 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: "revolve axis or angle is near zero".into(),
        });
    }

    // Normalise so angle is always positive (flip axis for negative angle).
    let (axis, angle) = if angle < 0.0 {
        (-axis_direction.normalize(), -angle)
    } else {
        (axis_direction.normalize(), angle)
    };

    let face = first_face(profile)?;
    let mut pts = loop_points(face.outer_loop());
    let nv = pts.len();
    if nv < 3 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: "profile must have at least 3 vertices".into(),
        });
    }

    let ln = newell_normal(&pts);
    if ln.norm() < 1e-15 {
        return Err(KernelError::InvalidGeometry {
            code: ErrorCode::DegenerateCurve,
            detail: "profile is degenerate (collinear vertices)".into(),
        });
    }

    // Classify on-axis vertices.
    if pts.iter().all(|p| radial_dist(p, &axis_origin, &axis) < 1e-10) {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: "all profile vertices lie on the revolve axis".into(),
        });
    }

    // Reject profiles where an on-axis vertex is flanked by two off-axis
    // edges — this creates a non-manifold pinch point when revolved.
    {
        let oa: Vec<bool> =
            pts.iter().map(|p| radial_dist(p, &axis_origin, &axis) < 1e-10).collect();
        for i in 0..nv {
            if !oa[i] {
                continue;
            }
            let prev = if i == 0 { nv - 1 } else { i - 1 };
            let next = (i + 1) % nv;
            if !oa[prev] && !oa[next] {
                return Err(KernelError::InvalidInput {
                    code: ErrorCode::UnsupportedConfiguration,
                    detail: "on-axis vertex between two off-axis vertices \
                             creates a non-manifold pinch point"
                        .into(),
                });
            }
        }
    }

    // Determine correct winding so side quads get outward normals.
    let radial = first_radial(&pts, &axis_origin, &axis);
    let tangent = axis.cross(&radial);
    if ln.dot(&tangent) > 0.0 {
        pts.reverse();
    }

    let on_axis: Vec<bool> =
        pts.iter().map(|p| radial_dist(p, &axis_origin, &axis) < 1e-10).collect();

    // Angular segments.
    let full = (angle - TAU).abs() < 1e-10;
    let n_seg = if full { 24 } else { (24.0 * angle / TAU).ceil().max(1.0) as usize };
    let da = angle / n_seg as f64;

    // Build vertex rings.  On-axis vertices share a single Arc.
    let shared: Vec<Option<Arc<Vertex>>> = pts
        .iter()
        .zip(on_axis.iter())
        .map(|(p, &oa)| if oa { Some(Arc::new(Vertex::new(*p))) } else { None })
        .collect();

    let n_rings = if full { n_seg } else { n_seg + 1 };
    let rings: Vec<Vec<Arc<Vertex>>> = (0..n_rings)
        .map(|k| {
            let rot = transform::rotation(axis, k as f64 * da);
            pts.iter()
                .enumerate()
                .map(|(i, p)| {
                    if let Some(ref v) = shared[i] {
                        v.clone()
                    } else {
                        let rel = p - axis_origin;
                        let rotated = transform::transform_vector(&rot, &rel);
                        Arc::new(Vertex::new(axis_origin + rotated))
                    }
                })
                .collect()
        })
        .collect();

    // Approximate solid centre (on axis, at average profile height) for
    // outward-normal orientation checks.
    let avg_ax =
        pts.iter().map(|p| (p - axis_origin).dot(&axis)).sum::<f64>() / nv as f64;
    let solid_center = axis_origin + avg_ax * axis;

    let mut faces = Vec::new();

    // Side faces.
    for i in 0..nv {
        let j = (i + 1) % nv;
        for k in 0..n_seg {
            let kn = if full { (k + 1) % n_seg } else { k + 1 };

            if on_axis[i] && on_axis[j] {
                continue;
            } else if on_axis[i] {
                // Triangle: apex at i (on-axis), arc at j.
                let apex = &rings[k][i];
                let b0 = &rings[k][j];
                let b1 = &rings[kn][j];
                faces.push(make_tri_face(apex, b0, b1, &solid_center)?);
            } else if on_axis[j] {
                // Triangle: arc at i, apex at j (on-axis).
                let a0 = &rings[k][i];
                let a1 = &rings[kn][i];
                let apex = &rings[k][j];
                faces.push(make_tri_face(a0, a1, apex, &solid_center)?);
            } else {
                // Quad.
                let a0 = &rings[k][i];
                let a1 = &rings[kn][i];
                let b0 = &rings[k][j];
                let b1 = &rings[kn][j];
                faces.push(make_quad_face(a0, a1, b1, b0, &solid_center)?);
            }
        }
    }

    // Cap faces for partial revolution.
    if !full {
        // Start cap — profile at angle 0.
        let start = &rings[0];
        let sc: Vec<HalfEdge> =
            (0..nv).map(|i| line_he(&start[i], &start[(i + 1) % nv])).collect();
        let sn = newell_normal(
            &sc.iter().map(|h| *h.start_vertex().point()).collect::<Vec<_>>(),
        );
        let sl = Loop::new(sc, true)?;
        let ss = Arc::new(Surface::Plane(Plane::new(*start[0].point(), safe_normalize(sn))));
        faces.push(Face::new(ss, sl, vec![], true)?);

        // End cap — profile at final angle, reversed winding.
        let end = &rings[n_seg];
        let ec: Vec<HalfEdge> = (0..nv)
            .rev()
            .map(|i| {
                let j = if i == 0 { nv - 1 } else { i - 1 };
                line_he(&end[i], &end[j])
            })
            .collect();
        let en = newell_normal(
            &ec.iter().map(|h| *h.start_vertex().point()).collect::<Vec<_>>(),
        );
        let el = Loop::new(ec, true)?;
        let es = Arc::new(Surface::Plane(Plane::new(
            *end[0].point(),
            safe_normalize(en),
        )));
        faces.push(Face::new(es, el, vec![], true)?);
    }

    let shell = Shell::new(faces, true)?;
    let solid = Solid::new(shell, vec![])?;
    BRep::new(vec![solid])
}

// ── helpers ──────────────────────────────────────────────────────────────────

pub(crate) fn first_face(brep: &BRep) -> KResult<&Face> {
    for solid in brep.solids() {
        let faces = solid.outer_shell().faces();
        if !faces.is_empty() {
            return Ok(&faces[0]);
        }
    }
    Err(KernelError::InvalidInput {
        code: ErrorCode::MalformedInput,
        detail: "profile BRep has no faces".into(),
    })
}

pub(crate) fn loop_points(lp: &Loop) -> Vec<Point3> {
    lp.half_edges().iter().map(|he| *he.start_vertex().point()).collect()
}

pub(crate) fn newell_normal(pts: &[Point3]) -> Vector3 {
    let n = pts.len();
    let (mut nx, mut ny, mut nz) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let c = pts[i];
        let next = pts[(i + 1) % n];
        nx += (c.y - next.y) * (c.z + next.z);
        ny += (c.z - next.z) * (c.x + next.x);
        nz += (c.x - next.x) * (c.y + next.y);
    }
    Vector3::new(nx, ny, nz)
}

pub(crate) fn safe_normalize(v: Vector3) -> Vector3 {
    let len = v.norm();
    if len > 1e-30 { v / len } else { Vector3::z() }
}

fn tri_normal(a: &Point3, b: &Point3, c: &Point3) -> Vector3 {
    safe_normalize((b - a).cross(&(c - a)))
}

fn radial_dist(p: &Point3, origin: &Point3, axis: &Vector3) -> f64 {
    let rel = p - origin;
    (rel - rel.dot(axis) * axis).norm()
}

fn first_radial(pts: &[Point3], origin: &Point3, axis: &Vector3) -> Vector3 {
    // Try centroid first, fall back to first off-axis vertex.
    let n = pts.len() as f64;
    let sum = pts.iter().fold(Vector3::zeros(), |acc, p| acc + p.coords);
    let centroid = Point3::from(sum / n);
    let rel = centroid - origin;
    let radial = rel - rel.dot(axis) * axis;
    if radial.norm() > 1e-10 {
        return radial;
    }
    for p in pts {
        let rel = p - origin;
        let r = rel - rel.dot(axis) * axis;
        if r.norm() > 1e-10 {
            return r;
        }
    }
    Vector3::x()
}

/// Build a triangle face with outward-normal orientation check.
fn make_tri_face(
    v0: &Arc<Vertex>,
    v1: &Arc<Vertex>,
    v2: &Arc<Vertex>,
    solid_center: &Point3,
) -> KResult<Face> {
    let wn = tri_normal(v0.point(), v1.point(), v2.point());
    let fc = Point3::from((v0.point().coords + v1.point().coords + v2.point().coords) / 3.0);
    let diff = fc - solid_center;

    // If winding normal points toward solid centre, flip the triangle.
    let (norm, edges) = if diff.norm() > 1e-10 && wn.dot(&diff) < 0.0 {
        (
            tri_normal(v0.point(), v2.point(), v1.point()),
            vec![line_he(v0, v2), line_he(v2, v1), line_he(v1, v0)],
        )
    } else {
        (wn, vec![line_he(v0, v1), line_he(v1, v2), line_he(v2, v0)])
    };

    let lp = Loop::new(edges, true)?;
    let s = Arc::new(Surface::Plane(Plane::new(*v0.point(), norm)));
    Face::new(s, lp, vec![], true)
}

/// Build a quad face with outward-normal orientation check.
pub(crate) fn make_quad_face(
    a0: &Arc<Vertex>,
    a1: &Arc<Vertex>,
    b1: &Arc<Vertex>,
    b0: &Arc<Vertex>,
    solid_center: &Point3,
) -> KResult<Face> {
    let u = *a1.point() - *a0.point();
    let v = *b0.point() - *a0.point();
    let wn = safe_normalize(u.cross(&v));

    let fc = Point3::from(
        (a0.point().coords + a1.point().coords + b1.point().coords + b0.point().coords) / 4.0,
    );
    let diff = fc - solid_center;

    let (norm, edges) = if diff.norm() > 1e-10 && wn.dot(&diff) < 0.0 {
        (
            safe_normalize(v.cross(&u)),
            vec![line_he(a0, b0), line_he(b0, b1), line_he(b1, a1), line_he(a1, a0)],
        )
    } else {
        (
            wn,
            vec![line_he(a0, a1), line_he(a1, b1), line_he(b1, b0), line_he(b0, a0)],
        )
    };

    let lp = Loop::new(edges, true)?;
    let s = Arc::new(Surface::Plane(Plane::new(*a0.point(), norm)));
    Face::new(s, lp, vec![], true)
}
