use std::sync::Arc;

use knot_geom::curve::{Curve, LineSeg};
use knot_geom::surface::{Plane, Surface};
use knot_geom::{Point3, Vector3};
use knot_ops::loft::loft;
use knot_topo::*;

fn make_profile(pts: &[Point3], normal: Vector3) -> BRep {
    let n = pts.len();
    let verts: Vec<Arc<Vertex>> = pts.iter().map(|p| Arc::new(Vertex::new(*p))).collect();
    let edges: Vec<HalfEdge> = (0..n)
        .map(|i| {
            let j = (i + 1) % n;
            let curve = Arc::new(Curve::Line(LineSeg::new(pts[i], pts[j])));
            let edge = Arc::new(Edge::new(verts[i].clone(), verts[j].clone(), curve, 0.0, 1.0));
            HalfEdge::new(edge, true)
        })
        .collect();
    let lp = Loop::new(edges, true).unwrap();
    let surface = Arc::new(Surface::Plane(Plane::new(pts[0], normal)));
    let face = Face::new(surface, lp, vec![], true).unwrap();
    let shell = Shell::new(vec![face], false).unwrap();
    let solid = Solid::new(shell, vec![]).unwrap();
    BRep::new(vec![solid]).unwrap()
}

fn square(z: f64, half: f64) -> BRep {
    make_profile(
        &[
            Point3::new(-half, -half, z),
            Point3::new( half, -half, z),
            Point3::new( half,  half, z),
            Point3::new(-half,  half, z),
        ],
        Vector3::z(),
    )
}

#[test]
fn loft_two_squares_makes_a_prism() {
    let bottom = square(0.0, 0.5);
    let top    = square(2.0, 0.5);
    let result = loft(&[bottom, top]).expect("loft should succeed");
    let solid = &result.solids()[0];
    let faces = solid.outer_shell().faces();
    // 4 side quads + 2 caps
    assert_eq!(faces.len(), 6, "expected 6 faces, got {}", faces.len());
    result.validate().expect("loft result must validate");
}

#[test]
fn loft_two_different_size_squares_makes_a_frustum() {
    let bottom = square(0.0, 1.0);
    let top    = square(1.5, 0.4);
    let result = loft(&[bottom, top]).expect("loft should succeed");
    assert_eq!(result.solids()[0].outer_shell().faces().len(), 6);
    result.validate().expect("frustum loft must validate");
}

#[test]
fn loft_three_profiles_chains_two_ruled_strips() {
    let a = square(0.0, 0.5);
    let b = square(1.0, 1.0);
    let c = square(2.0, 0.3);
    let result = loft(&[a, b, c]).expect("3-profile loft should succeed");
    // 4 quads × 2 strips + 2 caps = 10
    let face_count = result.solids()[0].outer_shell().faces().len();
    assert_eq!(face_count, 10, "expected 10 faces, got {face_count}");
    result.validate().expect("3-profile loft must validate");
}

#[test]
fn loft_rejects_single_profile() {
    let one = square(0.0, 0.5);
    let err = loft(&[one]).expect_err("single profile must fail");
    let msg = format!("{err}");
    assert!(msg.contains("at least two"), "unexpected error: {msg}");
}

#[test]
fn loft_rejects_mismatched_vertex_counts() {
    let sq = square(0.0, 0.5);
    let tri = make_profile(
        &[
            Point3::new(-0.5, -0.5, 1.0),
            Point3::new( 0.5, -0.5, 1.0),
            Point3::new( 0.0,  0.5, 1.0),
        ],
        Vector3::z(),
    );
    let err = loft(&[sq, tri]).expect_err("vertex-count mismatch must fail");
    let msg = format!("{err}");
    assert!(msg.contains("vertex count"), "unexpected error: {msg}");
}
