use knot_core::KResult;
use knot_topo::BRep;
use serde::{Deserialize, Serialize};

/// Version-tagged envelope so we can evolve the format.
#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u32,
    brep: BRep,
}

const CURRENT_VERSION: u32 = 1;

/// Serialize a BRep to CBOR.
pub fn serialize(brep: &BRep) -> KResult<Vec<u8>> {
    let envelope = Envelope {
        version: CURRENT_VERSION,
        brep: brep.clone(),
    };
    let mut buf = Vec::new();
    ciborium::into_writer(&envelope, &mut buf).map_err(|e| knot_core::KernelError::Io {
        detail: format!("CBOR serialization failed: {}", e),
    })?;
    Ok(buf)
}

/// Deserialize a BRep from CBOR.
pub fn deserialize(data: &[u8]) -> KResult<BRep> {
    let envelope: Envelope =
        ciborium::from_reader(data).map_err(|e| knot_core::KernelError::Io {
            detail: format!("CBOR deserialization failed: {}", e),
        })?;
    if envelope.version != CURRENT_VERSION {
        return Err(knot_core::KernelError::Io {
            detail: format!(
                "unsupported CBOR version {} (expected {})",
                envelope.version, CURRENT_VERSION
            ),
        });
    }
    Ok(envelope.brep)
}
