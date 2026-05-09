use std::sync::Arc;
use knot_core::{Id, KResult};
use super::face::Face;

/// A connected set of faces. May be open or closed.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Shell {
    id: Id<Shell>,
    #[serde(with = "knot_core::serde_arc::arc_slice")]
    faces: Arc<[Face]>,
    is_closed: bool,
}

impl Shell {
    pub fn new(faces: Vec<Face>, is_closed: bool) -> KResult<Self> {
        use std::hash::{Hash, Hasher, DefaultHasher};

        let mut hasher = DefaultHasher::new();
        for face in &faces {
            face.id().hash_value().hash(&mut hasher);
        }
        is_closed.hash(&mut hasher);
        let id = Id::from_hash(hasher.finish());

        Ok(Self {
            id,
            faces: faces.into(),
            is_closed,
        })
    }

    pub fn id(&self) -> Id<Shell> {
        self.id
    }

    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    pub fn is_closed(&self) -> bool {
        self.is_closed
    }

    pub fn face_count(&self) -> usize {
        self.faces.len()
    }
}
