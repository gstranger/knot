use knot_ops::primitives::make_box;
use knot_tessellate::{tessellate, TessellateOptions};

#[test]
fn stl_binary_box() {
    let brep = make_box(1.0, 1.0, 1.0).unwrap();
    let mesh = tessellate(&brep, TessellateOptions::default()).unwrap();
    let stl = knot_io::to_stl(&mesh).unwrap();

    // Header 80 + count 4 + 50 per triangle
    let tri_count = mesh.triangle_count();
    assert_eq!(stl.len(), 84 + tri_count * 50);

    // Check magic header prefix
    assert!(stl.starts_with(b"Knot CAD"));

    // Triangle count in the file matches
    let stored_count = u32::from_le_bytes(stl[80..84].try_into().unwrap());
    assert_eq!(stored_count as usize, tri_count);
}

#[test]
fn stl_ascii_box() {
    let brep = make_box(2.0, 3.0, 4.0).unwrap();
    let mesh = tessellate(&brep, TessellateOptions::default()).unwrap();
    let ascii = knot_io::to_stl_ascii(&mesh, "testbox").unwrap();

    assert!(ascii.starts_with("solid testbox\n"));
    assert!(ascii.ends_with("endsolid testbox\n"));
    assert!(ascii.contains("facet normal"));
    assert!(ascii.contains("vertex"));

    let facet_count = ascii.matches("endfacet").count();
    assert_eq!(facet_count, mesh.triangle_count());
}

#[test]
fn glb_box() {
    let brep = make_box(1.0, 1.0, 1.0).unwrap();
    let mesh = tessellate(&brep, TessellateOptions::default()).unwrap();
    let glb = knot_io::to_glb(&mesh).unwrap();

    // GLB magic
    assert_eq!(&glb[0..4], &0x46546C67u32.to_le_bytes()); // "glTF"
    // Version 2
    assert_eq!(&glb[4..8], &2u32.to_le_bytes());
    // Total length matches
    let total_len = u32::from_le_bytes(glb[8..12].try_into().unwrap());
    assert_eq!(total_len as usize, glb.len());

    // JSON chunk type
    assert_eq!(&glb[16..20], &0x4E4F534Au32.to_le_bytes());

    // JSON contains expected keys
    let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let json_str = std::str::from_utf8(&glb[20..20 + json_len]).unwrap();
    assert!(json_str.contains("\"POSITION\""));
    assert!(json_str.contains("\"NORMAL\""));
    assert!(json_str.contains("\"version\":\"2.0\""));
}

#[test]
fn glb_sphere() {
    let brep = knot_ops::primitives::make_sphere(
        knot_geom::Point3::new(0.0, 0.0, 0.0),
        1.0,
        16,
        8,
    )
    .unwrap();
    let mesh = tessellate(&brep, TessellateOptions::default()).unwrap();
    let glb = knot_io::to_glb(&mesh).unwrap();

    // Should produce a valid GLB with nonzero content
    assert!(glb.len() > 100);
    assert_eq!(&glb[0..4], &0x46546C67u32.to_le_bytes());
}

#[test]
fn stl_binary_empty_mesh() {
    let mesh = knot_tessellate::TriMesh {
        positions: vec![],
        normals: vec![],
        indices: vec![],
        face_ids: vec![],
    };
    let stl = knot_io::to_stl(&mesh).unwrap();
    assert_eq!(stl.len(), 84); // header + zero triangles
    let stored_count = u32::from_le_bytes(stl[80..84].try_into().unwrap());
    assert_eq!(stored_count, 0);
}

#[test]
fn glb_empty_mesh() {
    let mesh = knot_tessellate::TriMesh {
        positions: vec![],
        normals: vec![],
        indices: vec![],
        face_ids: vec![],
    };
    let glb = knot_io::to_glb(&mesh).unwrap();
    assert_eq!(&glb[0..4], &0x46546C67u32.to_le_bytes());
    let total_len = u32::from_le_bytes(glb[8..12].try_into().unwrap());
    assert_eq!(total_len as usize, glb.len());
}
