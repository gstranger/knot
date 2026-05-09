//! Shared topology construction utilities.
//!
//! `TopologyBuilder` deduplicates vertices and edges via snap-grid lattice
//! indices. `line_he` is a convenience for creating line-segment half-edges.
//! Both are used by boolean, extrude, fillet, and other operations.

use std::collections::HashMap;
use std::sync::Arc;

use knot_core::snap::LatticeIndex;
use knot_core::{ErrorCode, KResult, KernelError, SnapGrid};
use knot_geom::curve::{Curve, LineSeg};
use knot_geom::surface::Surface;
use knot_geom::Point3;
use knot_topo::*;

/// Session-scoped topology allocator that enforces shared-edge topology.
///
/// All vertices are snap-rounded to the model grid. Same lattice cell = same
/// `Arc<Vertex>`. Same lattice endpoint pair = same `Arc<Edge>`. This ensures
/// that when two faces share a boundary (e.g. at an intersection curve), they
/// reference the same underlying edge with opposite half-edge orientations.
pub struct TopologyBuilder {
    grid: SnapGrid,
    vertex_cache: HashMap<LatticeIndex, Arc<Vertex>>,
    edge_cache: HashMap<(LatticeIndex, LatticeIndex), Arc<Edge>>,
}

impl TopologyBuilder {
    pub fn new(grid: SnapGrid) -> Self {
        Self {
            grid,
            vertex_cache: HashMap::new(),
            edge_cache: HashMap::new(),
        }
    }

    /// Get or create a vertex at the given point, snapped to the grid.
    pub fn vertex(&mut self, point: Point3) -> Arc<Vertex> {
        let li = self.grid.lattice_index(point);
        self.vertex_cache
            .entry(li)
            .or_insert_with(|| Arc::new(Vertex::new(self.grid.lattice_to_point(li))))
            .clone()
    }

    /// Get or create an edge between two vertices.
    /// Returns (edge, same_sense) where same_sense indicates whether the
    /// edge's canonical direction matches the requested start→end direction.
    pub fn edge(&mut self, start: &Arc<Vertex>, end: &Arc<Vertex>) -> (Arc<Edge>, bool) {
        let start_li = self.grid.lattice_index(*start.point());
        let end_li = self.grid.lattice_index(*end.point());

        let (key, same_sense) = if start_li <= end_li {
            ((start_li, end_li), true)
        } else {
            ((end_li, start_li), false)
        };

        let edge = self
            .edge_cache
            .entry(key)
            .or_insert_with(|| {
                let (canon_start, canon_end) = if same_sense {
                    (start.clone(), end.clone())
                } else {
                    (end.clone(), start.clone())
                };
                let curve = Arc::new(Curve::Line(LineSeg::new(
                    *canon_start.point(),
                    *canon_end.point(),
                )));
                Arc::new(Edge::new(canon_start, canon_end, curve, 0.0, 1.0))
            })
            .clone();

        (edge, same_sense)
    }

    /// Convert a polygon to a Face, deduplicating vertices and edges.
    /// Snaps all vertices to the grid and filters degenerate edges.
    pub fn polygon_to_face(
        &mut self,
        polygon: &[Point3],
        surface: Arc<Surface>,
        same_sense: bool,
    ) -> KResult<Face> {
        if polygon.len() < 3 {
            return Err(KernelError::TopoInconsistency {
                code: ErrorCode::DanglingReference,
                detail: "polygon must have at least 3 vertices".into(),
            });
        }

        let verts: Vec<Arc<Vertex>> = polygon.iter().map(|p| self.vertex(*p)).collect();

        // Filter consecutive duplicate vertices (collapsed edges after snapping)
        let mut deduped: Vec<Arc<Vertex>> = Vec::new();
        for v in &verts {
            if deduped
                .last()
                .map_or(true, |last: &Arc<Vertex>| last.id() != v.id())
            {
                deduped.push(v.clone());
            }
        }
        if deduped.len() >= 2 && deduped.first().unwrap().id() == deduped.last().unwrap().id() {
            deduped.pop();
        }

        let n = deduped.len();
        if n < 3 {
            return Err(KernelError::TopoInconsistency {
                code: ErrorCode::DanglingReference,
                detail: "polygon degenerate after snap-rounding".into(),
            });
        }

        let mut half_edges = Vec::new();
        for i in 0..n {
            let j = (i + 1) % n;
            let (edge, fwd) = self.edge(&deduped[i], &deduped[j]);
            half_edges.push(HalfEdge::new(edge, fwd));
        }

        let loop_ = Loop::new(half_edges, true)?;
        Face::new(surface, loop_, vec![], same_sense)
    }

    /// Access the underlying grid.
    pub fn grid(&self) -> &SnapGrid {
        &self.grid
    }
}

/// Create a line-segment half-edge between two vertices.
pub fn line_he(start: &Arc<Vertex>, end: &Arc<Vertex>) -> HalfEdge {
    let curve = Arc::new(Curve::Line(LineSeg::new(*start.point(), *end.point())));
    let edge = Arc::new(Edge::new(start.clone(), end.clone(), curve, 0.0, 1.0));
    HalfEdge::new(edge, true)
}
