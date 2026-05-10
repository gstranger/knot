use std::sync::Arc;

use knot_geom::curve::{CircularArc, Curve, LineSeg};
use knot_geom::surface::{Plane, Surface};
use knot_geom::{Point3, Vector3};
use knot_ops::sweep::sweep_1rail;
use knot_topo::*;

/// Create a single-face planar profile BRep from a list of points.
fn make_profile(pts: &[Point3], normal: Vector3) -> BRep {
    let n = pts.len();
    let verts: Vec<Arc<Vertex>> = pts.iter().map(|p| Arc::new(Vertex::new(*p))).collect();
    let edges: Vec<HalfEdge> = (0..n)
        .map(|i| {
            let j = (i + 1) % n;
            let curve = Arc::new(Curve::Line(LineSeg::new(pts[i], pts[j])));
            let edge =
                Arc::new(Edge::new(verts[i].clone(), verts[j].clone(), curve, 0.0, 1.0));
            HalfEdge::new(edge, true)
        })
        .collect();
    let loop_ = Loop::new(edges, true).unwrap();
    let surface = Arc::new(Surface::Plane(Plane::new(pts[0], normal)));
    let face = Face::new(surface, loop_, vec![], true).unwrap();
    let shell = Shell::new(vec![face], false).unwrap();
    let solid = Solid::new(shell, vec![]).unwrap();
    BRep::new(vec![solid]).unwrap()
}

fn square_profile() -> BRep {
    make_profile(
        &[
            Point3::new(-0.5, -0.5, 0.0),
            Point3::new(0.5, -0.5, 0.0),
            Point3::new(0.5, 0.5, 0.0),
            Point3::new(-0.5, 0.5, 0.0),
        ],
        Vector3::z(),
    )
}

fn triangle_profile() -> BRep {
    make_profile(
        &[
            Point3::new(0.0, -0.5, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 0.5, 0.0),
        ],
        Vector3::z(),
    )
}

// ── line rail (produces a prism, like extrude) ───────────────────────────────

#[test]
fn sweep_line_rail_square() {
    let profile = square_profile();
    let rail = Curve::Line(LineSeg::new(
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(0.0, 0.0, 3.0),
    ));
    let brep = sweep_1rail(&profile, &rail).unwrap();
    let solid = brep.single_solid().unwrap();
    let shell = solid.outer_shell();
    // 1 segment: 4 side quads + 2 caps = 6 faces
    assert_eq!(shell.face_count(), 6);
    assert!(shell.is_closed());
    assert!(brep.validate().is_ok());
}

#[test]
fn sweep_line_rail_triangle() {
    let profile = triangle_profile();
    let rail = Curve::Line(LineSeg::new(
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(5.0, 0.0, 0.0),
    ));
    let brep = sweep_1rail(&profile, &rail).unwrap();
    let solid = brep.single_solid().unwrap();
    // 3 side quads + 2 caps = 5
    assert_eq!(solid.outer_shell().face_count(), 5);
    assert!(brep.validate().is_ok());
}

// ── circular arc rail (bent tube) ────────────────────────────────────────────

#[test]
fn sweep_arc_rail() {
    let profile = square_profile();
    // 90° arc in the XZ plane, radius 5
    let rail = Curve::CircularArc(CircularArc {
        center: Point3::new(0.0, 0.0, 0.0),
        normal: Vector3::y(),
        radius: 5.0,
        ref_direction: Vector3::x(),
        start_angle: 0.0,
        end_angle: std::f64::consts::FRAC_PI_2,
    });
    let brep = sweep_1rail(&profile, &rail).unwrap();
    let solid = brep.single_solid().unwrap();
    let shell = solid.outer_shell();
    // n_seg = ceil(24 * 0.25) = 6
    // 4 profile edges × 6 segments = 24 quads + 2 caps = 26
    assert_eq!(shell.face_count(), 26);
    assert!(shell.is_closed());
    assert!(brep.validate().is_ok());
}

// ── closed circular rail (torus-like) ────────────────────────────────────────

#[test]
fn sweep_closed_circle_rail() {
    let profile = square_profile();
    // Full circle in XZ plane, radius 5
    let rail = Curve::CircularArc(CircularArc {
        center: Point3::new(0.0, 0.0, 0.0),
        normal: Vector3::y(),
        radius: 5.0,
        ref_direction: Vector3::x(),
        start_angle: 0.0,
        end_angle: std::f64::consts::TAU,
    });
    let brep = sweep_1rail(&profile, &rail).unwrap();
    let solid = brep.single_solid().unwrap();
    let shell = solid.outer_shell();
    // 24 segments × 4 edges = 96 quads, no caps
    assert_eq!(shell.face_count(), 96);
    assert!(shell.is_closed());
    assert!(brep.validate().is_ok());
}

// ── oblique line rail ────────────────────────────────────────────────────────

#[test]
fn sweep_oblique_line() {
    let profile = square_profile();
    let rail = Curve::Line(LineSeg::new(
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(3.0, 4.0, 5.0),
    ));
    let brep = sweep_1rail(&profile, &rail).unwrap();
    assert_eq!(brep.single_solid().unwrap().outer_shell().face_count(), 6);
    assert!(brep.validate().is_ok());
}

// ── determinism ──────────────────────────────────────────────────────────────

#[test]
fn sweep_deterministic() {
    let profile = square_profile();
    let rail = Curve::CircularArc(CircularArc {
        center: Point3::origin(),
        normal: Vector3::y(),
        radius: 3.0,
        ref_direction: Vector3::x(),
        start_angle: 0.0,
        end_angle: std::f64::consts::FRAC_PI_2,
    });
    let a = sweep_1rail(&profile, &rail).unwrap();
    let b = sweep_1rail(&profile, &rail).unwrap();
    assert_eq!(a.id(), b.id());
}

// ── concave profile along arc ────────────────────────────────────────────────

#[test]
fn sweep_concave_profile() {
    // L-shaped profile swept along an arc
    let profile = make_profile(
        &[
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 0.5, 0.0),
            Point3::new(0.5, 0.5, 0.0),
            Point3::new(0.5, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ],
        Vector3::z(),
    );
    let rail = Curve::CircularArc(CircularArc {
        center: Point3::new(0.0, 0.0, 0.0),
        normal: Vector3::z(),
        radius: 5.0,
        ref_direction: Vector3::x(),
        start_angle: 0.0,
        end_angle: std::f64::consts::FRAC_PI_4,
    });
    let brep = sweep_1rail(&profile, &rail).unwrap();
    let solid = brep.single_solid().unwrap();
    assert!(solid.outer_shell().is_closed());
    assert!(brep.validate().is_ok());
}
