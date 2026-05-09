use std::collections::HashMap;
use nalgebra::Point3;

/// Integer lattice index — the canonical identity of a snap-rounded vertex.
/// Deterministic, hashable, immune to f64 non-associativity.
///
/// Two points that snap to the same grid cell get the same `LatticeIndex`,
/// regardless of the floating-point operation order that produced them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LatticeIndex {
    pub ix: i64,
    pub iy: i64,
    pub iz: i64,
}

/// Snap-rounding grid. All geometric output is rounded to the nearest
/// grid point after topological decisions are made.
/// Guarantees minimum vertex separation = cell_size.
///
/// This is a model-level resolution: one grid for the entire model,
/// not per-entity tolerances.
///
/// Vertex identity is keyed on integer lattice indices, not f64 bit patterns.
/// This eliminates nondeterminism from f64 rounding non-associativity.
#[derive(Clone, Copy, Debug)]
pub struct SnapGrid {
    pub cell_size: f64,
}

impl SnapGrid {
    pub fn new(cell_size: f64) -> Self {
        assert!(cell_size > 0.0, "SnapGrid cell_size must be positive");
        Self { cell_size }
    }

    /// Create a grid from a bounding box diagonal and resolution factor.
    pub fn from_bbox_diagonal(diagonal: f64, resolution: f64) -> Self {
        Self::new(diagonal * resolution)
    }

    /// Compute the integer lattice index for a point.
    /// This is the canonical vertex identity after snap rounding.
    pub fn lattice_index(&self, p: Point3<f64>) -> LatticeIndex {
        LatticeIndex {
            ix: (p.x / self.cell_size).round() as i64,
            iy: (p.y / self.cell_size).round() as i64,
            iz: (p.z / self.cell_size).round() as i64,
        }
    }

    /// Convert a lattice index back to a 3D point.
    /// This is the only place where lattice → f64 conversion happens.
    pub fn lattice_to_point(&self, idx: LatticeIndex) -> Point3<f64> {
        Point3::new(
            idx.ix as f64 * self.cell_size,
            idx.iy as f64 * self.cell_size,
            idx.iz as f64 * self.cell_size,
        )
    }

    /// Round a point to the nearest grid vertex.
    /// Equivalent to `lattice_to_point(lattice_index(p))`.
    pub fn snap(&self, p: Point3<f64>) -> Point3<f64> {
        self.lattice_to_point(self.lattice_index(p))
    }

    /// Snap a single coordinate to the grid.
    pub fn snap_scalar(&self, v: f64) -> f64 {
        (v / self.cell_size).round() as i64 as f64 * self.cell_size
    }

    /// Are two points coincident on this grid?
    /// Decided by integer lattice index equality — no f64 comparison.
    pub fn coincident(&self, a: Point3<f64>, b: Point3<f64>) -> bool {
        self.lattice_index(a) == self.lattice_index(b)
    }

    /// Snap-round a set of points and merge coincident ones.
    /// Uses integer lattice indices for identity, so merge decisions
    /// are deterministic regardless of f64 operation order.
    /// Returns (snapped_points, index_map).
    pub fn snap_and_merge(&self, points: &[Point3<f64>]) -> (Vec<Point3<f64>>, Vec<usize>) {
        let mut index_to_uid: HashMap<LatticeIndex, usize> = HashMap::new();
        let mut unique: Vec<Point3<f64>> = Vec::new();
        let mut index_map = Vec::with_capacity(points.len());

        for p in points {
            let li = self.lattice_index(*p);
            let uid = *index_to_uid.entry(li).or_insert_with(|| {
                let id = unique.len();
                unique.push(self.lattice_to_point(li));
                id
            });
            index_map.push(uid);
        }

        (unique, index_map)
    }
}
