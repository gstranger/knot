//! Rigid transforms (translation, rotation) and scaling for BRep solids.
//!
//! Reconstructs the full topology tree with transformed geometry.
//! - `Isometry3` — rigid body transform (rotation + translation, no scale).
//! - `scale_brep` — uniform or non-uniform scaling.

use std::sync::Arc;

use knot_core::{ErrorCode, KResult, KernelError};
use knot_geom::Point3;
use knot_geom::point::Isometry3;
use knot_geom::transform::{transform_point, transform_vector};
use knot_geom::curve::{Curve, NurbsCurve, LineSeg, CircularArc, EllipticalArc};
use knot_geom::surface::{Surface, NurbsSurface, Plane, Sphere, Cylinder, Cone, Torus};
use knot_topo::{BRep, Solid, Shell, Face, Loop, Edge, HalfEdge, Vertex};

/// Apply a rigid transform to a BRep, returning a new BRep.
pub fn transform_brep(brep: &BRep, iso: &Isometry3) -> KResult<BRep> {
    let solids: Vec<Solid> = brep
        .solids()
        .iter()
        .map(|s| transform_solid(s, iso))
        .collect::<KResult<Vec<_>>>()?;
    BRep::new(solids)
}

fn transform_solid(solid: &Solid, iso: &Isometry3) -> KResult<Solid> {
    let outer = transform_shell(solid.outer_shell(), iso)?;
    let voids: Vec<Shell> = solid
        .void_shells()
        .iter()
        .map(|s| transform_shell(s, iso))
        .collect::<KResult<Vec<_>>>()?;
    Solid::new(outer, voids)
}

fn transform_shell(shell: &Shell, iso: &Isometry3) -> KResult<Shell> {
    let faces: Vec<Face> = shell
        .faces()
        .iter()
        .map(|f| transform_face(f, iso))
        .collect::<KResult<Vec<_>>>()?;
    Shell::new(faces, shell.is_closed())
}

fn transform_face(face: &Face, iso: &Isometry3) -> KResult<Face> {
    let surface = Arc::new(transform_surface(&face.surface(), iso));
    let outer = transform_loop(face.outer_loop(), iso)?;
    let inners: Vec<Loop> = face
        .inner_loops()
        .iter()
        .map(|l| transform_loop(l, iso))
        .collect::<KResult<Vec<_>>>()?;
    Face::new(surface, outer, inners, face.same_sense())
}

fn transform_loop(lp: &Loop, iso: &Isometry3) -> KResult<Loop> {
    let half_edges: Vec<HalfEdge> = lp
        .half_edges()
        .iter()
        .map(|he| transform_half_edge(he, iso))
        .collect();
    Loop::new(half_edges, lp.is_outer())
}

fn transform_half_edge(he: &HalfEdge, iso: &Isometry3) -> HalfEdge {
    let edge = he.edge();
    let start = Arc::new(Vertex::new(transform_point(iso, edge.start().point())));
    let end = Arc::new(Vertex::new(transform_point(iso, edge.end().point())));
    let curve = Arc::new(transform_curve(&edge.curve(), iso));
    let new_edge = Arc::new(Edge::new(start, end, curve, edge.t_start(), edge.t_end()));
    HalfEdge::new(new_edge, he.same_sense())
}

// ── Curve transforms ──

fn transform_curve(curve: &Curve, iso: &Isometry3) -> Curve {
    match curve {
        Curve::Line(l) => Curve::Line(LineSeg::new(
            transform_point(iso, &l.start),
            transform_point(iso, &l.end),
        )),
        Curve::CircularArc(a) => Curve::CircularArc(CircularArc {
            center: transform_point(iso, &a.center),
            normal: transform_vector(iso, &a.normal),
            radius: a.radius,
            ref_direction: transform_vector(iso, &a.ref_direction),
            start_angle: a.start_angle,
            end_angle: a.end_angle,
        }),
        Curve::EllipticalArc(a) => Curve::EllipticalArc(EllipticalArc {
            center: transform_point(iso, &a.center),
            normal: transform_vector(iso, &a.normal),
            major_axis: transform_vector(iso, &a.major_axis),
            major_radius: a.major_radius,
            minor_radius: a.minor_radius,
            start_angle: a.start_angle,
            end_angle: a.end_angle,
        }),
        Curve::Nurbs(n) => {
            let pts: Vec<_> = n
                .control_points()
                .iter()
                .map(|p| transform_point(iso, p))
                .collect();
            // NURBS constructor validates — unwrap is safe since inputs were already valid.
            Curve::Nurbs(
                NurbsCurve::new(pts, n.weights().to_vec(), n.knots().to_vec(), n.degree())
                    .expect("transformed NURBS curve should remain valid"),
            )
        }
    }
}

// ── Surface transforms ──

fn transform_surface(surface: &Surface, iso: &Isometry3) -> Surface {
    match surface {
        Surface::Plane(p) => Surface::Plane(Plane {
            origin: transform_point(iso, &p.origin),
            normal: transform_vector(iso, &p.normal),
            u_axis: transform_vector(iso, &p.u_axis),
            v_axis: transform_vector(iso, &p.v_axis),
        }),
        Surface::Sphere(s) => Surface::Sphere(Sphere::new(
            transform_point(iso, &s.center),
            s.radius,
        )),
        Surface::Cylinder(c) => Surface::Cylinder(Cylinder {
            origin: transform_point(iso, &c.origin),
            axis: transform_vector(iso, &c.axis),
            radius: c.radius,
            ref_direction: transform_vector(iso, &c.ref_direction),
            v_min: c.v_min,
            v_max: c.v_max,
        }),
        Surface::Cone(c) => Surface::Cone(Cone {
            apex: transform_point(iso, &c.apex),
            axis: transform_vector(iso, &c.axis),
            half_angle: c.half_angle,
            ref_direction: transform_vector(iso, &c.ref_direction),
            v_min: c.v_min,
            v_max: c.v_max,
        }),
        Surface::Torus(t) => Surface::Torus(Torus {
            center: transform_point(iso, &t.center),
            axis: transform_vector(iso, &t.axis),
            major_radius: t.major_radius,
            minor_radius: t.minor_radius,
            ref_direction: transform_vector(iso, &t.ref_direction),
        }),
        Surface::Nurbs(n) => {
            let pts: Vec<_> = n
                .control_points()
                .iter()
                .map(|p| transform_point(iso, p))
                .collect();
            Surface::Nurbs(
                NurbsSurface::new(
                    pts,
                    n.weights().to_vec(),
                    n.knots_u().to_vec(),
                    n.knots_v().to_vec(),
                    n.degree_u(),
                    n.degree_v(),
                    n.count_u(),
                    n.count_v(),
                )
                .expect("transformed NURBS surface should remain valid"),
            )
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// Scaling
// ═══════════════════════════════════════════════════════════════

/// Apply a scaling transform to a BRep, returning a new BRep.
///
/// Supports non-uniform scaling `(sx, sy, sz)`.  For analytical curved
/// geometry (arcs, spheres, cylinders, cones, tori), non-uniform scaling
/// is rejected because it changes the surface type (e.g. sphere → ellipsoid).
/// Planes, lines, and NURBS always work.
pub fn scale_brep(brep: &BRep, sx: f64, sy: f64, sz: f64) -> KResult<BRep> {
    let s = Scale { sx, sy, sz };
    let solids: Vec<Solid> = brep
        .solids()
        .iter()
        .map(|solid| scale_solid(solid, &s))
        .collect::<KResult<Vec<_>>>()?;
    BRep::new(solids)
}

#[derive(Clone, Copy)]
struct Scale {
    sx: f64,
    sy: f64,
    sz: f64,
}

impl Scale {
    fn is_uniform(&self) -> bool {
        let tol = 1e-12;
        (self.sx - self.sy).abs() < tol && (self.sy - self.sz).abs() < tol
    }

    /// The uniform scale factor (only valid when is_uniform() is true).
    fn factor(&self) -> f64 {
        self.sx
    }

    fn apply_point(&self, p: &Point3) -> Point3 {
        Point3::new(p.x * self.sx, p.y * self.sy, p.z * self.sz)
    }

    fn apply_vector(&self, v: &knot_geom::Vector3) -> knot_geom::Vector3 {
        knot_geom::Vector3::new(v.x * self.sx, v.y * self.sy, v.z * self.sz)
    }

    /// Scale a direction vector and re-normalize.  Returns the original
    /// vector if the scaled result is degenerate.
    fn apply_direction(&self, v: &knot_geom::Vector3) -> knot_geom::Vector3 {
        let scaled = self.apply_vector(v);
        let len = scaled.norm();
        if len > 1e-30 { scaled / len } else { *v }
    }
}

fn non_uniform_error(what: &str) -> KernelError {
    KernelError::OperationFailed {
        code: ErrorCode::UnsupportedConfiguration,
        detail: format!(
            "non-uniform scaling of {} requires conversion to NURBS (not yet supported)",
            what,
        ),
    }
}

fn scale_solid(solid: &Solid, s: &Scale) -> KResult<Solid> {
    let outer = scale_shell(solid.outer_shell(), s)?;
    let voids: Vec<Shell> = solid
        .void_shells()
        .iter()
        .map(|sh| scale_shell(sh, s))
        .collect::<KResult<Vec<_>>>()?;
    Solid::new(outer, voids)
}

fn scale_shell(shell: &Shell, s: &Scale) -> KResult<Shell> {
    let faces: Vec<Face> = shell
        .faces()
        .iter()
        .map(|f| scale_face(f, s))
        .collect::<KResult<Vec<_>>>()?;
    Shell::new(faces, shell.is_closed())
}

fn scale_face(face: &Face, s: &Scale) -> KResult<Face> {
    let surface = Arc::new(scale_surface(face.surface(), s)?);
    let outer = scale_loop(face.outer_loop(), s)?;
    let inners: Vec<Loop> = face
        .inner_loops()
        .iter()
        .map(|l| scale_loop(l, s))
        .collect::<KResult<Vec<_>>>()?;
    // Non-uniform scaling with negative determinant flips orientation.
    let det = s.sx * s.sy * s.sz;
    let same_sense = if det < 0.0 { !face.same_sense() } else { face.same_sense() };
    Face::new(surface, outer, inners, same_sense)
}

fn scale_loop(lp: &Loop, s: &Scale) -> KResult<Loop> {
    let half_edges: Vec<HalfEdge> = lp
        .half_edges()
        .iter()
        .map(|he| scale_half_edge(he, s))
        .collect::<KResult<Vec<_>>>()?;
    Loop::new(half_edges, lp.is_outer())
}

fn scale_half_edge(he: &HalfEdge, s: &Scale) -> KResult<HalfEdge> {
    let edge = he.edge();
    let start = Arc::new(Vertex::new(s.apply_point(edge.start().point())));
    let end = Arc::new(Vertex::new(s.apply_point(edge.end().point())));
    let curve = Arc::new(scale_curve(edge.curve(), s)?);
    let new_edge = Arc::new(Edge::new(start, end, curve, edge.t_start(), edge.t_end()));
    Ok(HalfEdge::new(new_edge, he.same_sense()))
}

// ── Curve scaling ──

fn scale_curve(curve: &Curve, s: &Scale) -> KResult<Curve> {
    match curve {
        Curve::Line(l) => Ok(Curve::Line(LineSeg::new(
            s.apply_point(&l.start),
            s.apply_point(&l.end),
        ))),
        Curve::CircularArc(a) => {
            if s.is_uniform() {
                let f = s.factor().abs();
                Ok(Curve::CircularArc(CircularArc {
                    center: s.apply_point(&a.center),
                    normal: a.normal, // direction unchanged under uniform scale
                    radius: a.radius * f,
                    ref_direction: a.ref_direction,
                    start_angle: a.start_angle,
                    end_angle: a.end_angle,
                }))
            } else {
                Err(non_uniform_error("CircularArc"))
            }
        }
        Curve::EllipticalArc(a) => {
            if s.is_uniform() {
                let f = s.factor().abs();
                Ok(Curve::EllipticalArc(EllipticalArc {
                    center: s.apply_point(&a.center),
                    normal: a.normal,
                    major_axis: a.major_axis,
                    major_radius: a.major_radius * f,
                    minor_radius: a.minor_radius * f,
                    start_angle: a.start_angle,
                    end_angle: a.end_angle,
                }))
            } else {
                Err(non_uniform_error("EllipticalArc"))
            }
        }
        Curve::Nurbs(n) => {
            let pts: Vec<_> = n.control_points().iter().map(|p| s.apply_point(p)).collect();
            Ok(Curve::Nurbs(
                NurbsCurve::new(pts, n.weights().to_vec(), n.knots().to_vec(), n.degree())
                    .expect("scaled NURBS curve should remain valid"),
            ))
        }
    }
}

// ── Surface scaling ──

fn scale_surface(surface: &Surface, s: &Scale) -> KResult<Surface> {
    match surface {
        Surface::Plane(p) => {
            let u = s.apply_vector(&p.u_axis);
            let v = s.apply_vector(&p.v_axis);
            let normal = u.cross(&v);
            let len = normal.norm();
            let normal = if len > 1e-30 { normal / len } else { p.normal };
            Ok(Surface::Plane(Plane {
                origin: s.apply_point(&p.origin),
                normal,
                u_axis: u,
                v_axis: v,
            }))
        }
        Surface::Sphere(sp) => {
            if s.is_uniform() {
                let f = s.factor().abs();
                Ok(Surface::Sphere(Sphere::new(
                    s.apply_point(&sp.center),
                    sp.radius * f,
                )))
            } else {
                Err(non_uniform_error("Sphere"))
            }
        }
        Surface::Cylinder(c) => {
            if s.is_uniform() {
                let f = s.factor().abs();
                Ok(Surface::Cylinder(Cylinder {
                    origin: s.apply_point(&c.origin),
                    axis: s.apply_direction(&c.axis),
                    radius: c.radius * f,
                    ref_direction: s.apply_direction(&c.ref_direction),
                    v_min: c.v_min * f,
                    v_max: c.v_max * f,
                }))
            } else {
                Err(non_uniform_error("Cylinder"))
            }
        }
        Surface::Cone(c) => {
            if s.is_uniform() {
                let f = s.factor().abs();
                Ok(Surface::Cone(Cone {
                    apex: s.apply_point(&c.apex),
                    axis: s.apply_direction(&c.axis),
                    half_angle: c.half_angle,
                    ref_direction: s.apply_direction(&c.ref_direction),
                    v_min: c.v_min * f,
                    v_max: c.v_max * f,
                }))
            } else {
                Err(non_uniform_error("Cone"))
            }
        }
        Surface::Torus(t) => {
            if s.is_uniform() {
                let f = s.factor().abs();
                Ok(Surface::Torus(Torus {
                    center: s.apply_point(&t.center),
                    axis: s.apply_direction(&t.axis),
                    major_radius: t.major_radius * f,
                    minor_radius: t.minor_radius * f,
                    ref_direction: s.apply_direction(&t.ref_direction),
                }))
            } else {
                Err(non_uniform_error("Torus"))
            }
        }
        Surface::Nurbs(n) => {
            let pts: Vec<_> = n.control_points().iter().map(|p| s.apply_point(p)).collect();
            Ok(Surface::Nurbs(
                NurbsSurface::new(
                    pts,
                    n.weights().to_vec(),
                    n.knots_u().to_vec(),
                    n.knots_v().to_vec(),
                    n.degree_u(),
                    n.degree_v(),
                    n.count_u(),
                    n.count_v(),
                )
                .expect("scaled NURBS surface should remain valid"),
            ))
        }
    }
}
