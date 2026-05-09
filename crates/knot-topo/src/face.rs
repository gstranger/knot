use std::sync::Arc;
use knot_core::{Id, KResult};
use knot_geom::surface::Surface;
use super::loop_::Loop;

/// A bounded region on a surface.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Face {
    id: Id<Face>,
    surface: Arc<Surface>,
    outer_loop: Loop,
    #[serde(with = "knot_core::serde_arc::arc_slice")]
    inner_loops: Arc<[Loop]>,
    same_sense: bool,
}

impl Face {
    pub fn new(
        surface: Arc<Surface>,
        outer_loop: Loop,
        inner_loops: Vec<Loop>,
        same_sense: bool,
    ) -> KResult<Self> {
        use std::hash::{Hash, Hasher, DefaultHasher};

        let mut hasher = DefaultHasher::new();
        // Hash based on vertex topology for deterministic ID
        for he in outer_loop.half_edges() {
            he.start_vertex().id().hash_value().hash(&mut hasher);
        }
        same_sense.hash(&mut hasher);
        let id = Id::from_hash(hasher.finish());

        Ok(Self {
            id,
            surface,
            outer_loop,
            inner_loops: inner_loops.into(),
            same_sense,
        })
    }

    pub fn id(&self) -> Id<Face> {
        self.id
    }

    pub fn surface(&self) -> &Arc<Surface> {
        &self.surface
    }

    pub fn outer_loop(&self) -> &Loop {
        &self.outer_loop
    }

    pub fn inner_loops(&self) -> &[Loop] {
        &self.inner_loops
    }

    pub fn same_sense(&self) -> bool {
        self.same_sense
    }
}
