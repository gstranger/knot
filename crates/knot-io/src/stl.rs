//! STL export (binary and ASCII) from a tessellated TriMesh.

use knot_core::KResult;
use knot_tessellate::TriMesh;

/// Write a binary STL file from a TriMesh.
///
/// Binary STL layout:
///   80-byte header
///   u32 triangle count (little-endian)
///   per triangle: normal (3×f32), v1 (3×f32), v2 (3×f32), v3 (3×f32), u16 attribute
pub fn to_binary_stl(mesh: &TriMesh) -> KResult<Vec<u8>> {
    let tri_count = mesh.triangle_count();
    // 80 header + 4 count + 50 per triangle
    let size = 84 + tri_count * 50;
    let mut buf = Vec::with_capacity(size);

    // Header: 80 bytes
    let mut header = [0u8; 80];
    let tag = b"Knot CAD kernel - binary STL";
    header[..tag.len()].copy_from_slice(tag);
    buf.extend_from_slice(&header);

    // Triangle count
    buf.extend_from_slice(&(tri_count as u32).to_le_bytes());

    for tri in 0..tri_count {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;

        // Compute facet normal from vertex positions (STL spec: per-facet normal).
        let v0 = &mesh.positions[i0];
        let v1 = &mesh.positions[i1];
        let v2 = &mesh.positions[i2];
        let e1 = v1 - v0;
        let e2 = v2 - v0;
        let n = e1.cross(&e2);
        let len = n.norm();
        let n = if len > 1e-30 { n / len } else { mesh.normals[i0] };

        // Normal
        push_f32(&mut buf, n.x as f32);
        push_f32(&mut buf, n.y as f32);
        push_f32(&mut buf, n.z as f32);

        // Vertices
        for idx in [i0, i1, i2] {
            let p = &mesh.positions[idx];
            push_f32(&mut buf, p.x as f32);
            push_f32(&mut buf, p.y as f32);
            push_f32(&mut buf, p.z as f32);
        }

        // Attribute byte count (unused)
        buf.extend_from_slice(&0u16.to_le_bytes());
    }

    Ok(buf)
}

/// Write an ASCII STL string from a TriMesh.
pub fn to_ascii_stl(mesh: &TriMesh, solid_name: &str) -> KResult<String> {
    let tri_count = mesh.triangle_count();
    let mut out = String::with_capacity(tri_count * 256);

    out.push_str(&format!("solid {solid_name}\n"));

    for tri in 0..tri_count {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;

        let v0 = &mesh.positions[i0];
        let v1 = &mesh.positions[i1];
        let v2 = &mesh.positions[i2];
        let e1 = v1 - v0;
        let e2 = v2 - v0;
        let n = e1.cross(&e2);
        let len = n.norm();
        let n = if len > 1e-30 { n / len } else { mesh.normals[i0] };

        out.push_str(&format!("  facet normal {} {} {}\n", n.x, n.y, n.z));
        out.push_str("    outer loop\n");
        for idx in [i0, i1, i2] {
            let p = &mesh.positions[idx];
            out.push_str(&format!("      vertex {} {} {}\n", p.x, p.y, p.z));
        }
        out.push_str("    endloop\n");
        out.push_str("  endfacet\n");
    }

    out.push_str(&format!("endsolid {solid_name}\n"));
    Ok(out)
}

fn push_f32(buf: &mut Vec<u8>, val: f32) {
    buf.extend_from_slice(&val.to_le_bytes());
}
