use std::f64::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;

use knot_geom::curve::{Curve, LineSeg};
use knot_geom::surface::{Plane, Surface};
use knot_geom::{Point3, Vector3};
use knot_ops::extrude::{extrude_linear, revolve};
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

// ── extrude tests ────────────────────────────────────────────────────────────

#[test]
fn extrude_square() {
    let profile = make_profile(
        &[
            Point3::new(-1.0, -1.0, 0.0),
            Point3::new(1.0, -1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(-1.0, 1.0, 0.0),
        ],
        Vector3::z(),
    );
    let brep = extrude_linear(&profile, Vector3::z(), 3.0).unwrap();
    let solid = brep.single_solid().unwrap();
    let shell = solid.outer_shell();
    assert_eq!(shell.face_count(), 6); // 4 sides + top + bottom
    assert!(shell.is_closed());
    assert!(brep.validate().is_ok());
}

#[test]
fn extrude_triangle() {
    let profile = make_profile(
        &[
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(1.0, 1.5, 0.0),
        ],
        Vector3::z(),
    );
    let brep = extrude_linear(&profile, Vector3::z(), 5.0).unwrap();
    let solid = brep.single_solid().unwrap();
    assert_eq!(solid.outer_shell().face_count(), 5); // 3 sides + top + bottom
    assert!(solid.outer_shell().is_closed());
    assert!(brep.validate().is_ok());
}

#[test]
fn extrude_oblique_direction() {
    let profile = make_profile(
        &[
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ],
        Vector3::z(),
    );
    let brep = extrude_linear(&profile, Vector3::new(1.0, 1.0, 2.0), 1.0).unwrap();
    assert_eq!(brep.single_solid().unwrap().outer_shell().face_count(), 6);
    assert!(brep.validate().is_ok());
}

#[test]
fn extrude_reversed_profile() {
    // Profile wound opposite to extrusion direction — should still work.
    let profile = make_profile(
        &[
            Point3::new(-1.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(1.0, -1.0, 0.0),
            Point3::new(-1.0, -1.0, 0.0),
        ],
        -Vector3::z(),
    );
    let brep = extrude_linear(&profile, Vector3::z(), 2.0).unwrap();
    assert_eq!(brep.single_solid().unwrap().outer_shell().face_count(), 6);
    assert!(brep.validate().is_ok());
}

#[test]
fn extrude_deterministic() {
    let profile = make_profile(
        &[
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
        ],
        Vector3::z(),
    );
    let a = extrude_linear(&profile, Vector3::z(), 1.0).unwrap();
    let b = extrude_linear(&profile, Vector3::z(), 1.0).unwrap();
    assert_eq!(a.id(), b.id());
}

#[test]
fn extrude_pentagon() {
    let pts: Vec<Point3> = (0..5)
        .map(|i| {
            let a = TAU * i as f64 / 5.0;
            Point3::new(a.cos(), a.sin(), 0.0)
        })
        .collect();
    let profile = make_profile(&pts, Vector3::z());
    let brep = extrude_linear(&profile, Vector3::z(), 1.0).unwrap();
    assert_eq!(brep.single_solid().unwrap().outer_shell().face_count(), 7); // 5 sides + 2 caps
    assert!(brep.validate().is_ok());
}

// ── revolve tests ────────────────────────────────────────────────────────────

#[test]
fn revolve_rectangle_full() {
    // Rectangle at x=1..2, z=0..1, revolved 360° around z-axis → torus-like.
    let profile = make_profile(
        &[
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 0.0),
        ],
        -Vector3::y(),
    );
    let brep = revolve(&profile, Point3::origin(), Vector3::z(), TAU).unwrap();
    let solid = brep.single_solid().unwrap();
    assert_eq!(solid.outer_shell().face_count(), 4 * 24); // 4 edges × 24 segs
    assert!(solid.outer_shell().is_closed());
    assert!(brep.validate().is_ok());
}

#[test]
fn revolve_triangle_with_axis_vertices() {
    // Right triangle with two vertices on the z-axis → cone-like.
    let profile = make_profile(
        &[
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
        ],
        -Vector3::y(),
    );
    let brep = revolve(&profile, Point3::origin(), Vector3::z(), TAU).unwrap();
    let solid = brep.single_solid().unwrap();
    // 2 triangle fans × 24 = 48, on-axis edge skipped
    assert_eq!(solid.outer_shell().face_count(), 48);
    assert!(solid.outer_shell().is_closed());
    assert!(brep.validate().is_ok());
}

#[test]
fn revolve_partial_90() {
    let profile = make_profile(
        &[
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
        ],
        -Vector3::y(),
    );
    let brep = revolve(&profile, Point3::origin(), Vector3::z(), FRAC_PI_2).unwrap();
    let solid = brep.single_solid().unwrap();
    let shell = solid.outer_shell();
    // n_seg = ceil(24 * 0.25) = 6
    // 4 edges × 6 segs = 24 side faces + 2 caps = 26
    assert_eq!(shell.face_count(), 26);
    assert!(shell.is_closed());
    assert!(brep.validate().is_ok());
}

#[test]
fn revolve_deterministic() {
    let profile = make_profile(
        &[
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(1.5, 0.0, 1.0),
        ],
        -Vector3::y(),
    );
    let a = revolve(&profile, Point3::origin(), Vector3::z(), TAU).unwrap();
    let b = revolve(&profile, Point3::origin(), Vector3::z(), TAU).unwrap();
    assert_eq!(a.id(), b.id());
}

#[test]
fn revolve_negative_angle() {
    let profile = make_profile(
        &[
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
        ],
        -Vector3::y(),
    );
    let brep = revolve(&profile, Point3::origin(), Vector3::z(), -FRAC_PI_2).unwrap();
    assert!(brep.validate().is_ok());
    // Same face count as positive 90° (6 segs × 4 edges + 2 caps = 26)
    assert_eq!(brep.single_solid().unwrap().outer_shell().face_count(), 26);
}

#[test]
fn revolve_single_axis_vertex_rejected() {
    // A single on-axis vertex flanked by two off-axis edges creates a
    // non-manifold pinch point and should be rejected.
    let profile = make_profile(
        &[
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 1.0),
        ],
        -Vector3::y(),
    );
    assert!(revolve(&profile, Point3::origin(), Vector3::z(), TAU).is_err());
}

#[test]
fn revolve_two_adjacent_axis_vertices() {
    // Rectangle with two vertices on z-axis (adjacent) → valid solid.
    let profile = make_profile(
        &[
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 1.0),
            Point3::new(0.0, 0.0, 1.0),
        ],
        -Vector3::y(),
    );
    let brep = revolve(&profile, Point3::origin(), Vector3::z(), TAU).unwrap();
    let solid = brep.single_solid().unwrap();
    // Edge 0→1 (on→off): 24 tris, 1→2 (off→off): 24 quads, 2→3 (off→on): 24 tris, 3→0 (on→on): skip
    assert_eq!(solid.outer_shell().face_count(), 72);
    assert!(solid.outer_shell().is_closed());
    assert!(brep.validate().is_ok());
}
