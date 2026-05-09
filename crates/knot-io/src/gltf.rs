//! glTF 2.0 export (GLB binary container) from a tessellated TriMesh.
//!
//! Produces a self-contained `.glb` file with a single mesh containing
//! POSITION, NORMAL attributes and indexed triangles.  No textures, no
//! materials beyond the default — just geometry.

use knot_core::KResult;
use knot_tessellate::TriMesh;

/// Write a GLB (binary glTF 2.0) file from a TriMesh.
pub fn to_glb(mesh: &TriMesh) -> KResult<Vec<u8>> {
    let vert_count = mesh.vertex_count();
    let tri_count = mesh.triangle_count();

    // ── Build BIN chunk data ────────────────────────────────────────
    // Layout: [positions (VEC3 f32)] [normals (VEC3 f32)] [indices (u32)]
    let pos_bytes = vert_count * 12; // 3 × f32
    let norm_bytes = vert_count * 12;
    let idx_bytes = tri_count * 3 * 4; // u32 per index
    let bin_len = pos_bytes + norm_bytes + idx_bytes;

    let mut bin = Vec::with_capacity(bin_len);

    // Compute AABB for positions accessor (required by spec).
    let (mut min_x, mut min_y, mut min_z) = (f64::MAX, f64::MAX, f64::MAX);
    let (mut max_x, mut max_y, mut max_z) = (f64::MIN, f64::MIN, f64::MIN);

    for p in &mesh.positions {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        min_z = min_z.min(p.z);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
        max_z = max_z.max(p.z);
    }

    // Handle empty mesh — set min/max to zero.
    if vert_count == 0 {
        min_x = 0.0; min_y = 0.0; min_z = 0.0;
        max_x = 0.0; max_y = 0.0; max_z = 0.0;
    }

    // Positions
    for p in &mesh.positions {
        bin.extend_from_slice(&(p.x as f32).to_le_bytes());
        bin.extend_from_slice(&(p.y as f32).to_le_bytes());
        bin.extend_from_slice(&(p.z as f32).to_le_bytes());
    }

    // Normals
    for n in &mesh.normals {
        bin.extend_from_slice(&(n.x as f32).to_le_bytes());
        bin.extend_from_slice(&(n.y as f32).to_le_bytes());
        bin.extend_from_slice(&(n.z as f32).to_le_bytes());
    }

    // Indices
    for &i in &mesh.indices {
        bin.extend_from_slice(&i.to_le_bytes());
    }

    // Pad BIN to 4-byte alignment (spec requires it).
    while bin.len() % 4 != 0 {
        bin.push(0);
    }

    // ── Build JSON chunk ────────────────────────────────────────────
    let json = format!(
        r#"{{"asset":{{"version":"2.0","generator":"Knot CAD"}},"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"NORMAL":1}},"indices":2,"mode":4}}]}}],"accessors":[{{"bufferView":0,"componentType":5126,"count":{vert_count},"type":"VEC3","min":[{min_x},{min_y},{min_z}],"max":[{max_x},{max_y},{max_z}]}},{{"bufferView":1,"componentType":5126,"count":{vert_count},"type":"VEC3"}},{{"bufferView":2,"componentType":5125,"count":{idx_count},"type":"SCALAR"}}],"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":{pos_bytes},"target":34962}},{{"buffer":0,"byteOffset":{norm_offset},"byteLength":{norm_bytes},"target":34962}},{{"buffer":0,"byteOffset":{idx_offset},"byteLength":{idx_bytes},"target":34963}}],"buffers":[{{"byteLength":{bin_padded_len}}}]}}"#,
        vert_count = vert_count,
        idx_count = tri_count * 3,
        min_x = min_x as f32,
        min_y = min_y as f32,
        min_z = min_z as f32,
        max_x = max_x as f32,
        max_y = max_y as f32,
        max_z = max_z as f32,
        pos_bytes = pos_bytes,
        norm_offset = pos_bytes,
        norm_bytes = norm_bytes,
        idx_offset = pos_bytes + norm_bytes,
        idx_bytes = idx_bytes,
        bin_padded_len = bin.len(),
    );

    let json_bytes = json.as_bytes();
    // Pad JSON to 4-byte alignment with spaces (spec requires 0x20 padding).
    let json_padded_len = (json_bytes.len() + 3) & !3;

    // ── Assemble GLB ────────────────────────────────────────────────
    // GLB header: magic + version + total length (12 bytes)
    // Chunk 0 (JSON): length + type + data
    // Chunk 1 (BIN):  length + type + data
    let total_len = 12 + 8 + json_padded_len + 8 + bin.len();

    let mut glb = Vec::with_capacity(total_len);

    // Header
    glb.extend_from_slice(&0x46546C67u32.to_le_bytes()); // magic "glTF"
    glb.extend_from_slice(&2u32.to_le_bytes()); // version
    glb.extend_from_slice(&(total_len as u32).to_le_bytes());

    // JSON chunk
    glb.extend_from_slice(&(json_padded_len as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
    glb.extend_from_slice(json_bytes);
    for _ in 0..(json_padded_len - json_bytes.len()) {
        glb.push(0x20); // space padding
    }

    // BIN chunk
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
    glb.extend_from_slice(&bin);

    Ok(glb)
}
