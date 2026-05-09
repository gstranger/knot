use std::sync::Arc;
use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::curve::{Curve, LineSeg};
use knot_geom::surface::{Surface, Plane};
use knot_topo::*;

fn make_quad_face(pts: [Point3; 4], normal: Vector3) -> Face {
    let verts: Vec<Arc<Vertex>> = pts.iter().map(|p| Arc::new(Vertex::new(*p))).collect();
    let mut edges = Vec::new();
    for i in 0..4 {
        let j = (i + 1) % 4;
        let curve = Arc::new(Curve::Line(LineSeg::new(*verts[i].point(), *verts[j].point())));
        let edge = Arc::new(Edge::new(verts[i].clone(), verts[j].clone(), curve, 0.0, 1.0));
        edges.push(HalfEdge::new(edge, true));
    }
    let loop_ = Loop::new(edges, true).unwrap();
    let surface = Arc::new(Surface::Plane(Plane::new(pts[0], normal)));
    Face::new(surface, loop_, vec![], true).unwrap()
}

#[test]
fn valid_box_passes_validation() {
    // Build a proper closed box and verify it passes validation
    let f = [
        make_quad_face(
            [Point3::new(0.,0.,0.), Point3::new(1.,0.,0.), Point3::new(1.,1.,0.), Point3::new(0.,1.,0.)],
            -Vector3::z(),
        ),
        make_quad_face(
            [Point3::new(0.,0.,1.), Point3::new(1.,0.,1.), Point3::new(1.,1.,1.), Point3::new(0.,1.,1.)],
            Vector3::z(),
        ),
        make_quad_face(
            [Point3::new(0.,0.,0.), Point3::new(1.,0.,0.), Point3::new(1.,0.,1.), Point3::new(0.,0.,1.)],
            -Vector3::y(),
        ),
        make_quad_face(
            [Point3::new(0.,1.,0.), Point3::new(1.,1.,0.), Point3::new(1.,1.,1.), Point3::new(0.,1.,1.)],
            Vector3::y(),
        ),
        make_quad_face(
            [Point3::new(0.,0.,0.), Point3::new(0.,1.,0.), Point3::new(0.,1.,1.), Point3::new(0.,0.,1.)],
            -Vector3::x(),
        ),
        make_quad_face(
            [Point3::new(1.,0.,0.), Point3::new(1.,1.,0.), Point3::new(1.,1.,1.), Point3::new(1.,0.,1.)],
            Vector3::x(),
        ),
    ];

    let shell = Shell::new(f.to_vec(), true).unwrap();
    let solid = Solid::new(shell, vec![]).unwrap();
    let brep = BRep::new(vec![solid]).unwrap();
    assert!(brep.validate().is_ok());
}

#[test]
fn empty_loop_fails() {
    // A loop with zero edges should fail
    let result = Loop::new(vec![], true);
    assert!(result.is_err(), "empty loop should fail");
}

#[test]
fn single_edge_seam_loop_is_valid() {
    // A single closed edge (seam on a rotational surface) is valid in STEP.
    let v0 = Arc::new(Vertex::new(Point3::new(1., 0., 0.)));
    let c = Arc::new(Curve::Line(LineSeg::new(*v0.point(), *v0.point())));
    let e = Arc::new(Edge::new(v0.clone(), v0.clone(), c, 0.0, 1.0));
    let loop_ = Loop::new(vec![HalfEdge::new(e, true)], true).unwrap();
    let surface = Arc::new(Surface::Plane(Plane::new(Point3::origin(), Vector3::z())));
    let face = Face::new(surface, loop_, vec![], true).unwrap();
    // Single-edge face should pass validation (seam edge)
    let shell = Shell::new(vec![face], false).unwrap();
    let solid = Solid::new(shell, vec![]).unwrap();
    let brep = BRep::new(vec![solid]).unwrap();
    assert!(brep.validate().is_ok());
}

#[test]
fn open_shell_validation_passes_when_not_closed() {
    // A single triangle face as an open shell
    let pts = [
        Point3::new(0., 0., 0.),
        Point3::new(1., 0., 0.),
        Point3::new(0., 1., 0.),
    ];
    let verts: Vec<Arc<Vertex>> = pts.iter().map(|p| Arc::new(Vertex::new(*p))).collect();
    let mut edges = Vec::new();
    for i in 0..3 {
        let j = (i + 1) % 3;
        let curve = Arc::new(Curve::Line(LineSeg::new(*verts[i].point(), *verts[j].point())));
        let edge = Arc::new(Edge::new(verts[i].clone(), verts[j].clone(), curve, 0.0, 1.0));
        edges.push(HalfEdge::new(edge, true));
    }
    let loop_ = Loop::new(edges, true).unwrap();
    let surface = Arc::new(Surface::Plane(Plane::new(Point3::origin(), Vector3::z())));
    let face = Face::new(surface, loop_, vec![], true).unwrap();

    // Open shell (not closed) — should pass validation
    let shell = Shell::new(vec![face], false).unwrap();
    let solid = Solid::new(shell, vec![]).unwrap();
    let brep = BRep::new(vec![solid]).unwrap();
    assert!(brep.validate().is_ok());
}
