use knot_core::{Id, id::hash_deterministic};
use knot_geom::Point3;

/// A topological vertex — a point in model space.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Vertex {
    id: Id<Vertex>,
    point: Point3,
}

impl Vertex {
    pub fn new(point: Point3) -> Self {
        let id = Id::from_hash(hash_deterministic(&OrderedPoint(point)));
        Self { id, point }
    }

    pub fn point(&self) -> &Point3 {
        &self.point
    }

    pub fn id(&self) -> Id<Vertex> {
        self.id
    }
}

/// Wrapper for hashing Point3 deterministically via ordered f64 bits.
#[derive(Clone, Copy)]
struct OrderedPoint(Point3);

impl std::hash::Hash for OrderedPoint {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.x.to_bits().hash(state);
        self.0.y.to_bits().hash(state);
        self.0.z.to_bits().hash(state);
    }
}
