use std::sync::Arc;
use knot_core::Id;
use knot_geom::{Point3, curve::Curve};
use super::vertex::Vertex;

/// A topological edge connecting two vertices with an underlying geometric curve.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Edge {
    id: Id<Edge>,
    start: Arc<Vertex>,
    end: Arc<Vertex>,
    curve: Arc<Curve>,
    t_start: f64,
    t_end: f64,
}

impl Edge {
    pub fn new(
        start: Arc<Vertex>,
        end: Arc<Vertex>,
        curve: Arc<Curve>,
        t_start: f64,
        t_end: f64,
    ) -> Self {
        
        use std::hash::{Hash, Hasher};

        let mut hasher = std::hash::DefaultHasher::new();
        start.id().hash_value().hash(&mut hasher);
        end.id().hash_value().hash(&mut hasher);
        t_start.to_bits().hash(&mut hasher);
        t_end.to_bits().hash(&mut hasher);
        let id = Id::from_hash(hasher.finish());

        Self { id, start, end, curve, t_start, t_end }
    }

    pub fn id(&self) -> Id<Edge> {
        self.id
    }

    pub fn start(&self) -> &Vertex {
        &self.start
    }

    pub fn end(&self) -> &Vertex {
        &self.end
    }

    pub fn curve(&self) -> &Arc<Curve> {
        &self.curve
    }

    pub fn t_start(&self) -> f64 {
        self.t_start
    }

    pub fn t_end(&self) -> f64 {
        self.t_end
    }

    pub fn is_closed(&self) -> bool {
        self.start.id() == self.end.id()
    }

    /// Evaluate a point on this edge at parameter t.
    pub fn point_at(&self, t: f64) -> Point3 {
        use knot_geom::curve::CurveParam;
        self.curve.point_at(CurveParam(t))
    }
}

/// A directed usage of an Edge within a Loop.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HalfEdge {
    edge: Arc<Edge>,
    same_sense: bool,
}

impl HalfEdge {
    pub fn new(edge: Arc<Edge>, same_sense: bool) -> Self {
        Self { edge, same_sense }
    }

    pub fn edge(&self) -> &Arc<Edge> {
        &self.edge
    }

    pub fn same_sense(&self) -> bool {
        self.same_sense
    }

    pub fn start_vertex(&self) -> &Vertex {
        if self.same_sense {
            self.edge.start()
        } else {
            self.edge.end()
        }
    }

    pub fn end_vertex(&self) -> &Vertex {
        if self.same_sense {
            self.edge.end()
        } else {
            self.edge.start()
        }
    }
}
