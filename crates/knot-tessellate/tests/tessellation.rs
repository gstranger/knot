use std::sync::Arc;
use knot_geom::curve::{Curve, LineSeg};
use knot_geom::surface::{Plane, Surface};
use knot_geom::{Point3, Vector3};
use knot_ops::primitives;
use knot_tessellate::{tessellate, TessellateOptions};
use knot_topo::*;

#[test]
fn tessellate_box() {
    let brep = primitives::make_box(1.0, 1.0, 1.0).unwrap();
    let mesh = tessellate(&brep, TessellateOptions::default()).unwrap();

    // A box has 6 faces, each quad → 2 triangles = 12 triangles
    assert_eq!(mesh.triangle_count(), 12);
    // Each face has 4 vertices (not shared across faces) = 24
    assert_eq!(mesh.vertex_count(), 24);
    // Every triangle should have a face ID
    assert_eq!(mesh.face_ids.len(), 12);
    // All normals should be unit length
    for n in &mesh.normals {
        assert!((n.norm() - 1.0).abs() < 1e-10, "normal not unit: {:?}", n);
    }
}

#[test]
fn tessellate_sphere() {
    let brep = primitives::make_sphere(Point3::origin(), 2.0, 8, 4).unwrap();
    let mesh = tessellate(&brep, TessellateOptions::default()).unwrap();
    assert!(mesh.triangle_count() > 0);
    assert!(mesh.vertex_count() > 0);
}

#[test]
fn tessellate_cylinder() {
    let brep = primitives::make_cylinder(Point3::origin(), 1.0, 3.0, 8).unwrap();
    let mesh = tessellate(&brep, TessellateOptions::default()).unwrap();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn flat_arrays_correct_length() {
    let brep = primitives::make_box(1.0, 1.0, 1.0).unwrap();
    let mesh = tessellate(&brep, TessellateOptions::default()).unwrap();
    assert_eq!(mesh.positions_flat().len(), mesh.vertex_count() * 3);
    assert_eq!(mesh.normals_flat().len(), mesh.vertex_count() * 3);
}

// ── concave polygon tests ────────────────────────────────────────────────────

/// Create a single-face planar profile BRep from a list of points.
fn make_profile_brep(pts: &[Point3], normal: Vector3) -> BRep {
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

#[test]
fn tessellate_concave_l_shape_extruded() {
    // L-shaped concave profile → extrude → tessellate.
    let profile = make_profile_brep(
        &[
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(2.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
            Point3::new(0.0, 2.0, 0.0),
        ],
        Vector3::z(),
    );
    let brep = knot_ops::extrude_linear(&profile, Vector3::z(), 1.0).unwrap();
    assert!(brep.validate().is_ok());

    let mesh = tessellate(&brep, TessellateOptions::default()).unwrap();

    // 6 side quads → 12 tris + 2 hexagonal caps → 2×4 = 8 tris → total 20.
    assert_eq!(mesh.triangle_count(), 20);
    assert_no_degenerate_triangles(&mesh);
}

#[test]
fn tessellate_concave_arrow() {
    // Arrow/chevron: concave polygon with one reflex vertex.
    let profile = make_profile_brep(
        &[
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 1.0, 0.0),
            Point3::new(0.0, 2.0, 0.0),
            Point3::new(0.5, 1.0, 0.0), // reflex vertex (concavity)
        ],
        Vector3::z(),
    );
    let brep = knot_ops::extrude_linear(&profile, Vector3::z(), 1.0).unwrap();
    let mesh = tessellate(&brep, TessellateOptions::default()).unwrap();

    // 4 side quads → 8 tris + 2 quad caps → 2×2 = 4 tris → total 12.
    assert_eq!(mesh.triangle_count(), 12);
    assert_no_degenerate_triangles(&mesh);
}

#[test]
fn tessellate_concave_single_face() {
    // Tessellate a single concave face directly (no extrusion).
    let brep = make_profile_brep(
        &[
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(3.0, 0.0, 0.0),
            Point3::new(3.0, 2.0, 0.0),
            Point3::new(2.0, 1.0, 0.0), // reflex
            Point3::new(1.0, 2.0, 0.0),
            Point3::new(0.0, 2.0, 0.0),
        ],
        Vector3::z(),
    );
    let mesh = tessellate(&brep, TessellateOptions::default()).unwrap();
    // 6-gon → 4 triangles.
    assert_eq!(mesh.triangle_count(), 4);
    assert_no_degenerate_triangles(&mesh);
}

fn assert_no_degenerate_triangles(mesh: &knot_tessellate::TriMesh) {
    for i in 0..mesh.triangle_count() {
        let a = mesh.positions[mesh.indices[i * 3] as usize];
        let b = mesh.positions[mesh.indices[i * 3 + 1] as usize];
        let c = mesh.positions[mesh.indices[i * 3 + 2] as usize];
        let area = (b - a).cross(&(c - a)).norm() * 0.5;
        assert!(area > 1e-15, "degenerate triangle at index {}", i);
    }
}
