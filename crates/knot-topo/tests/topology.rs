use std::sync::Arc;
use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::curve::{Curve, LineSeg};
use knot_geom::surface::{Surface, Plane};
use knot_topo::*;

/// Build a triangular face from 3 vertices to test topology construction.
fn make_triangle() -> Face {
    let v0 = Arc::new(Vertex::new(Point3::new(0.0, 0.0, 0.0)));
    let v1 = Arc::new(Vertex::new(Point3::new(1.0, 0.0, 0.0)));
    let v2 = Arc::new(Vertex::new(Point3::new(0.0, 1.0, 0.0)));

    let c01 = Arc::new(Curve::Line(LineSeg::new(*v0.point(), *v1.point())));
    let c12 = Arc::new(Curve::Line(LineSeg::new(*v1.point(), *v2.point())));
    let c20 = Arc::new(Curve::Line(LineSeg::new(*v2.point(), *v0.point())));

    let e01 = Arc::new(Edge::new(v0.clone(), v1.clone(), c01, 0.0, 1.0));
    let e12 = Arc::new(Edge::new(v1.clone(), v2.clone(), c12, 0.0, 1.0));
    let e20 = Arc::new(Edge::new(v2.clone(), v0.clone(), c20, 0.0, 1.0));

    let he0 = HalfEdge::new(e01, true);
    let he1 = HalfEdge::new(e12, true);
    let he2 = HalfEdge::new(e20, true);

    let loop_ = Loop::new(vec![he0, he1, he2], true).unwrap();

    let surface = Arc::new(Surface::Plane(Plane::new(Point3::origin(), Vector3::z())));
    Face::new(surface, loop_, vec![], true).unwrap()
}

#[test]
fn vertex_deterministic_id() {
    let v1 = Vertex::new(Point3::new(1.0, 2.0, 3.0));
    let v2 = Vertex::new(Point3::new(1.0, 2.0, 3.0));
    assert_eq!(v1.id(), v2.id(), "same coordinates should produce same id");
}

#[test]
fn vertex_different_points_different_ids() {
    let v1 = Vertex::new(Point3::new(1.0, 2.0, 3.0));
    let v2 = Vertex::new(Point3::new(1.0, 2.0, 4.0));
    assert_ne!(v1.id(), v2.id());
}

#[test]
fn edge_closed_detection() {
    let v = Arc::new(Vertex::new(Point3::origin()));
    let c = Arc::new(Curve::Line(LineSeg::new(Point3::origin(), Point3::origin())));
    let e = Edge::new(v.clone(), v.clone(), c, 0.0, 1.0);
    assert!(e.is_closed());
}

#[test]
fn edge_open() {
    let v0 = Arc::new(Vertex::new(Point3::new(0.0, 0.0, 0.0)));
    let v1 = Arc::new(Vertex::new(Point3::new(1.0, 0.0, 0.0)));
    let c = Arc::new(Curve::Line(LineSeg::new(*v0.point(), *v1.point())));
    let e = Edge::new(v0, v1, c, 0.0, 1.0);
    assert!(!e.is_closed());
}

#[test]
fn halfedge_direction() {
    let v0 = Arc::new(Vertex::new(Point3::new(0.0, 0.0, 0.0)));
    let v1 = Arc::new(Vertex::new(Point3::new(1.0, 0.0, 0.0)));
    let c = Arc::new(Curve::Line(LineSeg::new(*v0.point(), *v1.point())));
    let e = Arc::new(Edge::new(v0.clone(), v1.clone(), c, 0.0, 1.0));

    let fwd = HalfEdge::new(e.clone(), true);
    assert_eq!(fwd.start_vertex().id(), v0.id());
    assert_eq!(fwd.end_vertex().id(), v1.id());

    let rev = HalfEdge::new(e, false);
    assert_eq!(rev.start_vertex().id(), v1.id());
    assert_eq!(rev.end_vertex().id(), v0.id());
}

#[test]
fn loop_validates_closure() {
    let v0 = Arc::new(Vertex::new(Point3::new(0.0, 0.0, 0.0)));
    let v1 = Arc::new(Vertex::new(Point3::new(1.0, 0.0, 0.0)));
    let v2 = Arc::new(Vertex::new(Point3::new(0.0, 1.0, 0.0)));

    let c01 = Arc::new(Curve::Line(LineSeg::new(*v0.point(), *v1.point())));
    let c12 = Arc::new(Curve::Line(LineSeg::new(*v1.point(), *v2.point())));

    let e01 = Arc::new(Edge::new(v0.clone(), v1.clone(), c01, 0.0, 1.0));
    let e12 = Arc::new(Edge::new(v1.clone(), v2.clone(), c12, 0.0, 1.0));

    // Open loop (v0 → v1 → v2, but no edge back to v0)
    let result = Loop::new(vec![HalfEdge::new(e01, true), HalfEdge::new(e12, true)], true);
    assert!(result.is_err(), "open loop should fail validation");
}

#[test]
fn triangle_face_construction() {
    let face = make_triangle();
    assert_eq!(face.outer_loop().vertex_count(), 3);
    assert!(face.inner_loops().is_empty());
    assert!(face.same_sense());
}

#[test]
fn shell_and_solid_construction() {
    let face = make_triangle();
    let shell = Shell::new(vec![face], false).unwrap();
    assert_eq!(shell.face_count(), 1);
    assert!(!shell.is_closed());

    let solid = Solid::new(shell, vec![]).unwrap();
    let brep = BRep::new(vec![solid]).unwrap();
    assert!(brep.single_solid().is_some());
    assert!(brep.validate().is_ok());
}

#[test]
fn brep_deterministic_id() {
    let f1 = make_triangle();
    let f2 = make_triangle();

    let s1 = Shell::new(vec![f1], false).unwrap();
    let s2 = Shell::new(vec![f2], false).unwrap();
    let sol1 = Solid::new(s1, vec![]).unwrap();
    let sol2 = Solid::new(s2, vec![]).unwrap();
    let b1 = BRep::new(vec![sol1]).unwrap();
    let b2 = BRep::new(vec![sol2]).unwrap();

    assert_eq!(b1.id(), b2.id(), "identical topology should produce same BRep id");
}
