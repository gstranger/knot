use std::sync::Arc;
use knot_core::{Id, KResult};
use super::solid::Solid;

/// A complete boundary representation. Immutable, structurally shared.
/// This is the primary exchange type for the kernel.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BRep {
    #[serde(with = "knot_core::serde_arc::arc_slice")]
    solids: Arc<[Solid]>,
    id: Id<BRep>,
}

impl BRep {
    pub fn new(solids: Vec<Solid>) -> KResult<Self> {
        use std::hash::{Hash, Hasher, DefaultHasher};

        let mut hasher = DefaultHasher::new();
        for solid in &solids {
            solid.id().hash_value().hash(&mut hasher);
        }
        let id = Id::from_hash(hasher.finish());

        Ok(Self {
            solids: solids.into(),
            id,
        })
    }

    pub fn solids(&self) -> &[Solid] {
        &self.solids
    }

    pub fn id(&self) -> Id<BRep> {
        self.id
    }

    /// Convenience for single-solid BReps.
    pub fn single_solid(&self) -> Option<&Solid> {
        if self.solids.len() == 1 {
            Some(&self.solids[0])
        } else {
            None
        }
    }

    /// Validate the BRep topology.
    pub fn validate(&self) -> KResult<()> {
        super::validate::validate_brep(self)
    }
}
