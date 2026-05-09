use std::sync::Arc;
use knot_core::{Id, KResult};
use super::shell::Shell;

/// A solid bounded by one or more closed shells.
/// The first shell is the outer boundary; additional shells are voids.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Solid {
    id: Id<Solid>,
    outer_shell: Shell,
    #[serde(with = "knot_core::serde_arc::arc_slice")]
    void_shells: Arc<[Shell]>,
}

impl Solid {
    pub fn new(outer_shell: Shell, void_shells: Vec<Shell>) -> KResult<Self> {
        use std::hash::{Hash, Hasher, DefaultHasher};

        let mut hasher = DefaultHasher::new();
        outer_shell.id().hash_value().hash(&mut hasher);
        for vs in &void_shells {
            vs.id().hash_value().hash(&mut hasher);
        }
        let id = Id::from_hash(hasher.finish());

        Ok(Self {
            id,
            outer_shell,
            void_shells: void_shells.into(),
        })
    }

    pub fn id(&self) -> Id<Solid> {
        self.id
    }

    pub fn outer_shell(&self) -> &Shell {
        &self.outer_shell
    }

    pub fn void_shells(&self) -> &[Shell] {
        &self.void_shells
    }
}
