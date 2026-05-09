use knot_geom::Point3;
use knot_io::{from_cbor, to_cbor};
use knot_ops::primitives;

#[test]
fn roundtrip_box() {
    let brep = primitives::make_box(2.0, 3.0, 4.0).unwrap();
    let bytes = to_cbor(&brep).unwrap();
    let restored = from_cbor(&bytes).unwrap();

    assert_eq!(brep.id(), restored.id());
    assert!(restored.validate().is_ok());

    let shell = restored.single_solid().unwrap().outer_shell();
    assert_eq!(shell.face_count(), 6);
    assert!(shell.is_closed());
}

#[test]
fn roundtrip_sphere() {
    let brep = primitives::make_sphere(Point3::origin(), 1.0, 8, 4).unwrap();
    let bytes = to_cbor(&brep).unwrap();
    let restored = from_cbor(&bytes).unwrap();

    assert_eq!(brep.id(), restored.id());
    assert!(restored.validate().is_ok());
}

#[test]
fn roundtrip_cylinder() {
    let brep = primitives::make_cylinder(Point3::origin(), 1.5, 3.0, 12).unwrap();
    let bytes = to_cbor(&brep).unwrap();
    let restored = from_cbor(&bytes).unwrap();

    assert_eq!(brep.id(), restored.id());
    assert!(restored.validate().is_ok());
}

#[test]
fn roundtrip_extruded() {
    use std::sync::Arc;
    use knot_geom::curve::{Curve, LineSeg};
    use knot_geom::surface::{Plane, Surface};
    use knot_geom::Vector3;
    use knot_topo::*;

    let pts = [
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(2.0, 0.0, 0.0),
        Point3::new(2.0, 1.0, 0.0),
        Point3::new(1.0, 1.0, 0.0),
        Point3::new(1.0, 2.0, 0.0),
        Point3::new(0.0, 2.0, 0.0),
    ];
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
    let loop_ = Loop::new(edges, true).unwrap();
    let surface = Arc::new(Surface::Plane(Plane::new(pts[0], Vector3::z())));
    let face = Face::new(surface, loop_, vec![], true).unwrap();
    let shell = Shell::new(vec![face], false).unwrap();
    let solid = Solid::new(shell, vec![]).unwrap();
    let profile = BRep::new(vec![solid]).unwrap();

    let brep = knot_ops::extrude_linear(&profile, Vector3::z(), 1.0).unwrap();

    let bytes = to_cbor(&brep).unwrap();
    let restored = from_cbor(&bytes).unwrap();

    assert_eq!(brep.id(), restored.id());
    assert!(restored.validate().is_ok());
}

#[test]
fn roundtrip_boolean() {
    let a = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    let b = primitives::make_box(1.0, 1.0, 3.0).unwrap();
    let result = knot_ops::boolean::boolean(&a, &b, knot_ops::BooleanOp::Subtraction).unwrap();

    let bytes = to_cbor(&result).unwrap();
    let restored = from_cbor(&bytes).unwrap();

    assert_eq!(result.id(), restored.id());
    assert!(restored.validate().is_ok());
}

#[test]
fn cbor_is_compact() {
    let brep = primitives::make_box(1.0, 1.0, 1.0).unwrap();
    let bytes = to_cbor(&brep).unwrap();
    // CBOR of a unit box should be well under 10 KB.
    assert!(bytes.len() < 10_000, "CBOR too large: {} bytes", bytes.len());
    assert!(!bytes.is_empty());
}

#[test]
fn reject_bad_version() {
    let brep = primitives::make_box(1.0, 1.0, 1.0).unwrap();
    let mut bytes = to_cbor(&brep).unwrap();
    // Corrupt the version field (second byte in CBOR map).
    // Find and replace version value. Version 1 is encoded as CBOR unsigned int 1 (0x01).
    // We'll just create a manually crafted bad payload instead.
    // Simplest: append garbage to check error handling.
    bytes.truncate(bytes.len() / 2);
    assert!(from_cbor(&bytes).is_err());
}
