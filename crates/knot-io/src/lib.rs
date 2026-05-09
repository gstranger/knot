pub mod cbor;
pub mod gltf;
pub mod step;
pub mod stl;

use knot_core::KResult;
use knot_tessellate::TriMesh;
use knot_topo::BRep;

/// Serialize a BRep to CBOR bytes (native format).
pub fn to_cbor(brep: &BRep) -> KResult<Vec<u8>> {
    cbor::serialize(brep)
}

/// Deserialize a BRep from CBOR bytes.
pub fn from_cbor(data: &[u8]) -> KResult<BRep> {
    cbor::deserialize(data)
}

/// Read a BRep from a STEP file string.
pub fn from_step(input: &str) -> KResult<BRep> {
    step::read_step(input)
}

/// Write a BRep to a STEP file string.
pub fn to_step(brep: &BRep) -> KResult<String> {
    step::write_step(brep)
}

/// Write a TriMesh as binary STL.
pub fn to_stl(mesh: &TriMesh) -> KResult<Vec<u8>> {
    stl::to_binary_stl(mesh)
}

/// Write a TriMesh as ASCII STL.
pub fn to_stl_ascii(mesh: &TriMesh, solid_name: &str) -> KResult<String> {
    stl::to_ascii_stl(mesh, solid_name)
}

/// Write a TriMesh as GLB (binary glTF 2.0).
pub fn to_glb(mesh: &TriMesh) -> KResult<Vec<u8>> {
    gltf::to_glb(mesh)
}
