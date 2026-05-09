use std::sync::Arc;
use knot_core::{ErrorCode, KResult, KernelError};
use super::edge::HalfEdge;

/// A closed cycle of half-edges forming a face boundary.
/// Outer loops are CCW (viewed along face normal); inner loops (holes) are CW.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Loop {
    #[serde(with = "knot_core::serde_arc::arc_slice")]
    half_edges: Arc<[HalfEdge]>,
    is_outer: bool,
}

impl Loop {
    pub fn new(half_edges: Vec<HalfEdge>, is_outer: bool) -> KResult<Self> {
        if half_edges.is_empty() {
            return Err(KernelError::TopoInconsistency {
                code: ErrorCode::LoopNotClosed,
                detail: "loop must have at least one half-edge".into(),
            });
        }

        // Verify topological closure: end of each half-edge == start of next
        for i in 0..half_edges.len() {
            let next = (i + 1) % half_edges.len();
            let end_id = half_edges[i].end_vertex().id();
            let start_id = half_edges[next].start_vertex().id();
            if end_id != start_id {
                return Err(KernelError::TopoInconsistency {
                    code: ErrorCode::LoopNotClosed,
                    detail: format!("loop not closed at half-edge {}", i),
                });
            }
        }

        Ok(Self {
            half_edges: half_edges.into(),
            is_outer,
        })
    }

    pub fn half_edges(&self) -> &[HalfEdge] {
        &self.half_edges
    }

    pub fn is_outer(&self) -> bool {
        self.is_outer
    }

    pub fn vertex_count(&self) -> usize {
        self.half_edges.len()
    }
}
