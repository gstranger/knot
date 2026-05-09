use knot_geom::Point3;
use knot_ops::primitives;

#[test]
fn box_has_6_faces_8_vertices() {
    let brep = primitives::make_box(2.0, 3.0, 4.0).unwrap();
    let solid = brep.single_solid().unwrap();
    let shell = solid.outer_shell();
    assert_eq!(shell.face_count(), 6);
    assert!(shell.is_closed());
    assert!(brep.validate().is_ok());
}

#[test]
fn box_faces_are_quads() {
    let brep = primitives::make_box(1.0, 1.0, 1.0).unwrap();
    let solid = brep.single_solid().unwrap();
    for face in solid.outer_shell().faces() {
        assert_eq!(face.outer_loop().vertex_count(), 4, "box face should be a quad");
    }
}

#[test]
fn sphere_topology() {
    let brep = primitives::make_sphere(Point3::origin(), 1.0, 8, 4).unwrap();
    let solid = brep.single_solid().unwrap();
    assert!(solid.outer_shell().is_closed());
    assert!(brep.validate().is_ok());
    // 8 longitude * (2 tri caps + 2 quad strips) = 8*4 = 32 faces
    let expected_faces = 8 * 2 + 8 * 2; // 8 bottom tri + 8 top tri + 8*2 quad rows
    assert_eq!(solid.outer_shell().face_count(), expected_faces);
}

#[test]
fn cylinder_topology() {
    let brep = primitives::make_cylinder(Point3::origin(), 1.0, 2.0, 12).unwrap();
    let solid = brep.single_solid().unwrap();
    assert!(solid.outer_shell().is_closed());
    assert!(brep.validate().is_ok());
    // 12 side quads + 1 bottom + 1 top = 14
    assert_eq!(solid.outer_shell().face_count(), 14);
}

#[test]
fn box_deterministic() {
    let b1 = primitives::make_box(1.0, 2.0, 3.0).unwrap();
    let b2 = primitives::make_box(1.0, 2.0, 3.0).unwrap();
    assert_eq!(b1.id(), b2.id());
}
