use knot_geom::Point3;
use knot_ops::primitives;
use knot_ops::fillet::{fillet, chamfer};
use knot_tessellate::{tessellate, TessellateOptions};

// ── Chamfer tests ────────────────────────────────────────────────────────────

#[test]
fn chamfer_box_single_edge() {
    let brep = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    // Chamfer the bottom-front edge: from (-1,-1,-1) to (1,-1,-1)
    let result = chamfer(
        &brep,
        &[(Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, -1.0, -1.0))],
        0.3,
    ).unwrap();

    // 6 original faces + 1 chamfer face = 7
    let shell = result.single_solid().unwrap().outer_shell();
    assert_eq!(shell.face_count(), 7, "chamfer should produce 7 faces, got {}", shell.face_count());
    assert!(result.validate().is_ok(), "chamfer result must validate");

    // Should tessellate
    let mesh = tessellate(&result, TessellateOptions::default()).unwrap();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn chamfer_box_two_parallel_edges() {
    let brep = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    // Chamfer two parallel bottom edges
    let result = chamfer(
        &brep,
        &[
            (Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, -1.0, -1.0)),
            (Point3::new(-1.0, 1.0, -1.0), Point3::new(1.0, 1.0, -1.0)),
        ],
        0.3,
    ).unwrap();

    assert_eq!(result.single_solid().unwrap().outer_shell().face_count(), 8);
    assert!(result.validate().is_ok());
}

#[test]
fn chamfer_distance_too_large() {
    let brep = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    // Distance of 1.5 on a 2.0-wide face leaves only 0.5 — should still work
    let result = chamfer(
        &brep,
        &[(Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, -1.0, -1.0))],
        1.5,
    );
    // This should succeed (the geometry is valid even if tight)
    assert!(result.is_ok());
}

#[test]
fn chamfer_zero_distance_rejected() {
    let brep = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    let result = chamfer(
        &brep,
        &[(Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, -1.0, -1.0))],
        0.0,
    );
    assert!(result.is_err());
}

#[test]
fn chamfer_edge_not_found() {
    let brep = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    let result = chamfer(
        &brep,
        &[(Point3::new(99.0, 0.0, 0.0), Point3::new(100.0, 0.0, 0.0))],
        0.3,
    );
    assert!(result.is_err());
}

// ── Fillet tests ─────────────────────────────────────────────────────────────

#[test]
fn fillet_box_single_edge() {
    let brep = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    let result = fillet(
        &brep,
        &[(Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, -1.0, -1.0))],
        0.3,
    ).unwrap();

    let shell = result.single_solid().unwrap().outer_shell();
    assert_eq!(shell.face_count(), 7, "fillet should produce 7 faces, got {}", shell.face_count());
    assert!(result.validate().is_ok(), "fillet result must validate");

    let mesh = tessellate(&result, TessellateOptions::default()).unwrap();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn fillet_box_two_opposite_edges() {
    let brep = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    let result = fillet(
        &brep,
        &[
            (Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, -1.0, -1.0)),
            (Point3::new(-1.0, 1.0, 1.0), Point3::new(1.0, 1.0, 1.0)),
        ],
        0.3,
    ).unwrap();

    assert_eq!(result.single_solid().unwrap().outer_shell().face_count(), 8);
    assert!(result.validate().is_ok());
}

#[test]
fn fillet_box_four_parallel_edges() {
    // Fillet all 4 edges parallel to the x-axis
    let brep = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    let result = fillet(
        &brep,
        &[
            (Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, -1.0, -1.0)),
            (Point3::new(-1.0, 1.0, -1.0), Point3::new(1.0, 1.0, -1.0)),
            (Point3::new(-1.0, -1.0, 1.0), Point3::new(1.0, -1.0, 1.0)),
            (Point3::new(-1.0, 1.0, 1.0), Point3::new(1.0, 1.0, 1.0)),
        ],
        0.3,
    ).unwrap();

    // 6 original faces + 4 fillet faces = 10
    assert_eq!(result.single_solid().unwrap().outer_shell().face_count(), 10);
    assert!(result.validate().is_ok());
}

#[test]
fn fillet_zero_radius_rejected() {
    let brep = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    assert!(fillet(&brep, &[(Point3::new(-1.0,-1.0,-1.0), Point3::new(1.0,-1.0,-1.0))], 0.0).is_err());
}

#[test]
fn fillet_deterministic() {
    let brep = primitives::make_box(2.0, 2.0, 2.0).unwrap();
    let edges = [(Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, -1.0, -1.0))];
    let a = fillet(&brep, &edges, 0.3).unwrap();
    let b = fillet(&brep, &edges, 0.3).unwrap();
    assert_eq!(a.id(), b.id());
}
