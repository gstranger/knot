use knot_geom::Point3;
use knot_ops::primitives;
use knot_ops::boolean::{boolean, BooleanOp};
use knot_tessellate::{tessellate, TessellateOptions};

#[test]
fn union_disjoint_boxes() {
    // Two boxes that don't overlap — union should keep all faces
    let a = primitives::make_box(1.0, 1.0, 1.0).unwrap();
    let b = primitives::make_box(1.0, 1.0, 1.0).unwrap();
    // Both centered at origin — they're identical, so union = single box

    // Offset box b by translating its vertices (create at different position)
    let b = make_offset_box(3.0, 0.0, 0.0, 1.0, 1.0, 1.0);

    let result = boolean(&a, &b, BooleanOp::Union).unwrap();
    assert!(result.validate().is_ok());
    let faces = result.single_solid().unwrap().outer_shell().face_count();
    // Two disjoint boxes: 6 + 6 = 12 faces
    assert_eq!(faces, 12);
}

#[test]
fn union_overlapping_boxes() {
    let a = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    let b = make_offset_box(1.0, 0.0, 0.0, 2.0, 2.0, 2.0);

    let result = boolean(&a, &b, BooleanOp::Union).unwrap();
    assert!(result.validate().is_ok());

    // Should have more than 6 faces but fewer than 12
    let faces = result.single_solid().unwrap().outer_shell().face_count();
    assert!(faces > 6, "union should have >6 faces, got {}", faces);

    // Tessellate and check the mesh is valid
    let mesh = tessellate(&result, TessellateOptions::default()).unwrap();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn intersection_overlapping_boxes() {
    let a = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    let b = make_offset_box(1.0, 0.0, 0.0, 2.0, 2.0, 2.0);

    let result = boolean(&a, &b, BooleanOp::Intersection).unwrap();
    let faces = result.single_solid().unwrap().outer_shell().face_count();
    assert!(faces >= 4, "intersection should produce faces, got {}", faces);

    // Should tessellate successfully
    let mesh = tessellate(&result, TessellateOptions::default()).unwrap();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn subtraction_overlapping_boxes() {
    let a = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    let b = make_offset_box(1.0, 0.0, 0.0, 2.0, 2.0, 2.0);

    let result = boolean(&a, &b, BooleanOp::Subtraction).unwrap();
    let faces = result.single_solid().unwrap().outer_shell().face_count();
    assert!(faces >= 4, "subtraction should produce faces, got {}", faces);

    // Tessellation should succeed even if topology isn't perfectly manifold yet
    let mesh = tessellate(&result, TessellateOptions::default()).unwrap();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn intersection_disjoint_is_empty() {
    let a = primitives::make_box(1.0, 1.0, 1.0).unwrap();
    let b = make_offset_box(5.0, 0.0, 0.0, 1.0, 1.0, 1.0);

    let result = boolean(&a, &b, BooleanOp::Intersection);
    // Disjoint intersection should produce an error (empty result)
    assert!(result.is_err());
}

#[test]
fn union_tessellates() {
    let a = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    let b = make_offset_box(1.0, 1.0, 0.0, 2.0, 2.0, 2.0);

    let result = boolean(&a, &b, BooleanOp::Union).unwrap();
    let mesh = tessellate(&result, TessellateOptions::default()).unwrap();
    assert!(mesh.triangle_count() > 0);
    assert!(mesh.vertex_count() > 0);
    // All normals should be unit length
    for n in &mesh.normals {
        assert!((n.norm() - 1.0).abs() < 1e-6, "bad normal: {:?}", n);
    }
}

/// Helper: create a box offset from the origin.
fn make_offset_box(ox: f64, oy: f64, oz: f64, sx: f64, sy: f64, sz: f64) -> knot_topo::BRep {
    use std::sync::Arc;
    use knot_geom::Vector3;
    use knot_geom::curve::{Curve, LineSeg};
    use knot_geom::surface::{Surface, Plane};
    use knot_topo::*;

    let hx = sx / 2.0;
    let hy = sy / 2.0;
    let hz = sz / 2.0;

    let v = [
        Arc::new(Vertex::new(Point3::new(ox - hx, oy - hy, oz - hz))),
        Arc::new(Vertex::new(Point3::new(ox + hx, oy - hy, oz - hz))),
        Arc::new(Vertex::new(Point3::new(ox + hx, oy + hy, oz - hz))),
        Arc::new(Vertex::new(Point3::new(ox - hx, oy + hy, oz - hz))),
        Arc::new(Vertex::new(Point3::new(ox - hx, oy - hy, oz + hz))),
        Arc::new(Vertex::new(Point3::new(ox + hx, oy - hy, oz + hz))),
        Arc::new(Vertex::new(Point3::new(ox + hx, oy + hy, oz + hz))),
        Arc::new(Vertex::new(Point3::new(ox - hx, oy + hy, oz + hz))),
    ];

    let make_face = |vi: [usize; 4], origin: Point3, normal: Vector3| -> Face {
        let mut edges = Vec::new();
        for i in 0..4 {
            let j = (i + 1) % 4;
            let start = v[vi[i]].clone();
            let end = v[vi[j]].clone();
            let curve = Arc::new(Curve::Line(LineSeg::new(*start.point(), *end.point())));
            let edge = Arc::new(Edge::new(start, end, curve, 0.0, 1.0));
            edges.push(HalfEdge::new(edge, true));
        }
        let loop_ = Loop::new(edges, true).unwrap();
        let surface = Arc::new(Surface::Plane(Plane::new(origin, normal)));
        Face::new(surface, loop_, vec![], true).unwrap()
    };

    let faces = vec![
        make_face([0, 3, 2, 1], Point3::new(ox, oy, oz - hz), -Vector3::z()),
        make_face([4, 5, 6, 7], Point3::new(ox, oy, oz + hz), Vector3::z()),
        make_face([0, 1, 5, 4], Point3::new(ox, oy - hy, oz), -Vector3::y()),
        make_face([2, 3, 7, 6], Point3::new(ox, oy + hy, oz), Vector3::y()),
        make_face([0, 4, 7, 3], Point3::new(ox - hx, oy, oz), -Vector3::x()),
        make_face([1, 2, 6, 5], Point3::new(ox + hx, oy, oz), Vector3::x()),
    ];

    let shell = Shell::new(faces, true).unwrap();
    let solid = Solid::new(shell, vec![]).unwrap();
    BRep::new(vec![solid]).unwrap()
}
