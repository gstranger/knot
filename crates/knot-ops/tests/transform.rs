use knot_ops::primitives::{make_box, make_sphere, make_cylinder};
use knot_ops::transform::{transform_brep, scale_brep};
use knot_geom::transform::translation;
use knot_geom::{Point3, Vector3};

#[test]
fn translate_box() {
    let brep = make_box(2.0, 2.0, 2.0).unwrap();
    let iso = translation(Vector3::new(10.0, 0.0, 0.0));
    let moved = transform_brep(&brep, &iso).unwrap();

    // Should still be valid
    moved.validate().unwrap();

    // All vertices should have x shifted by 10
    let mesh = knot_tessellate::tessellate(&moved, Default::default()).unwrap();
    for p in &mesh.positions {
        assert!(p.x >= 9.0 - 1e-10 && p.x <= 11.0 + 1e-10,
            "vertex x={} should be in [9, 11]", p.x);
    }
}

#[test]
fn rotate_box() {
    let brep = make_box(1.0, 1.0, 1.0).unwrap();
    let iso = knot_geom::transform::rotation(Vector3::z(), std::f64::consts::FRAC_PI_2);
    let rotated = transform_brep(&brep, &iso).unwrap();
    rotated.validate().unwrap();

    // Same face count
    let orig_faces: u32 = brep.solids().iter().map(|s| s.outer_shell().face_count() as u32).sum();
    let rot_faces: u32 = rotated.solids().iter().map(|s| s.outer_shell().face_count() as u32).sum();
    assert_eq!(orig_faces, rot_faces);
}

#[test]
fn translate_then_boolean() {
    let a = make_box(2.0, 2.0, 2.0).unwrap();
    let b = make_box(2.0, 2.0, 2.0).unwrap();
    let iso = translation(Vector3::new(1.0, 0.0, 0.0));
    let b_moved = transform_brep(&b, &iso).unwrap();

    // Should be able to boolean with a translated BRep
    let result = knot_ops::boolean::boolean(&a, &b_moved, knot_ops::BooleanOp::Union);
    assert!(result.is_ok(), "boolean union after translate should succeed");
}

// ── Scaling tests ──

#[test]
fn uniform_scale_box() {
    let brep = make_box(2.0, 2.0, 2.0).unwrap();
    let scaled = scale_brep(&brep, 3.0, 3.0, 3.0).unwrap();
    scaled.validate().unwrap();

    // Original box is [-1,1]^3, scaled by 3 → [-3,3]^3
    let mesh = knot_tessellate::tessellate(&scaled, Default::default()).unwrap();
    for p in &mesh.positions {
        assert!(p.x >= -3.0 - 1e-10 && p.x <= 3.0 + 1e-10,
            "vertex x={} should be in [-3, 3]", p.x);
        assert!(p.y >= -3.0 - 1e-10 && p.y <= 3.0 + 1e-10,
            "vertex y={} should be in [-3, 3]", p.y);
    }
}

#[test]
fn non_uniform_scale_box() {
    // Box is all planar faces + line edges — non-uniform should work fine
    let brep = make_box(1.0, 1.0, 1.0).unwrap();
    let scaled = scale_brep(&brep, 2.0, 3.0, 4.0).unwrap();
    scaled.validate().unwrap();

    let mesh = knot_tessellate::tessellate(&scaled, Default::default()).unwrap();
    let (mut max_x, mut max_y, mut max_z) = (f64::MIN, f64::MIN, f64::MIN);
    for p in &mesh.positions {
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
        max_z = max_z.max(p.z);
    }
    assert!((max_x - 1.0).abs() < 1e-10, "max x={} should be 1.0 (0.5*2)", max_x);
    assert!((max_y - 1.5).abs() < 1e-10, "max y={} should be 1.5 (0.5*3)", max_y);
    assert!((max_z - 2.0).abs() < 1e-10, "max z={} should be 2.0 (0.5*4)", max_z);
}

#[test]
fn uniform_scale_sphere() {
    let brep = make_sphere(Point3::origin(), 1.0, 16, 8).unwrap();
    let scaled = scale_brep(&brep, 2.0, 2.0, 2.0).unwrap();
    scaled.validate().unwrap();
}

#[test]
fn non_uniform_scale_sphere_fails() {
    let brep = make_sphere(Point3::origin(), 1.0, 16, 8).unwrap();
    let result = scale_brep(&brep, 2.0, 3.0, 2.0);
    assert!(result.is_err(), "non-uniform scale of sphere should fail");
}

#[test]
fn uniform_scale_cylinder() {
    let brep = make_cylinder(Point3::origin(), 1.0, 2.0, 16).unwrap();
    let scaled = scale_brep(&brep, 0.5, 0.5, 0.5).unwrap();
    scaled.validate().unwrap();
}

#[test]
fn non_uniform_scale_cylinder_fails() {
    let brep = make_cylinder(Point3::origin(), 1.0, 2.0, 16).unwrap();
    let result = scale_brep(&brep, 1.0, 2.0, 1.0);
    assert!(result.is_err(), "non-uniform scale of cylinder should fail");
}

#[test]
fn scale_then_boolean() {
    let a = make_box(2.0, 2.0, 2.0).unwrap();
    let b = make_box(1.0, 1.0, 1.0).unwrap();
    let b_big = scale_brep(&b, 3.0, 3.0, 3.0).unwrap();
    let result = knot_ops::boolean::boolean(&a, &b_big, knot_ops::BooleanOp::Intersection);
    assert!(result.is_ok(), "boolean after uniform scale should succeed");
}
