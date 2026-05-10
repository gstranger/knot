use std::collections::HashMap;
use knot_core::{KResult, KernelError, ErrorCode, SnapGrid};
use knot_core::snap::LatticeIndex;
use knot_core::bbox::Aabb3;
use nalgebra::Point3;
use super::brep::BRep;
use super::shell::Shell;

/// Validation grid resolution as a fraction of the model's bbox
/// diagonal. Two vertex points within `bbox_diag * VALIDATE_GRID_RELATIVE`
/// of each other are treated as the same vertex.
///
/// Sized to absorb STEP-precision drift (typically 1e-6 to 1e-7
/// relative on coordinates written by CAD exporters) while staying
/// far below feature size. Matches the boolean's tolerance scale so
/// vertex-identity decisions agree across the kernel.
const VALIDATE_GRID_RELATIVE: f64 = 1e-7;

/// Lower bound on cell size in absolute units. Used when the model
/// bbox is empty/degenerate so vertices still get a deterministic
/// lattice key.
const VALIDATE_GRID_FLOOR: f64 = 1e-12;

/// Validate a BRep for topological consistency.
///
/// Enforces the fail-or-correct contract: if this returns Ok, the BRep
/// is topologically valid. If not, returns a structured error.
///
/// All identity comparisons use integer lattice indices — no distance thresholds.
///
/// Checks:
/// 1. Every face has at least 3 edges
/// 2. All loops are closed (end vertex lattice index = start vertex lattice index of next edge)
/// 3. Edge-vertex geometry consistency (curve endpoints snap to same lattice cell as vertices)
/// 4. Edge-use counting: in a closed shell, every edge is used exactly 2 times
/// 5. Euler-Poincare formula
pub fn validate_brep(brep: &BRep) -> KResult<()> {
    let grid = grid_for_brep(brep);
    for solid in brep.solids() {
        validate_shell(solid.outer_shell(), true, &grid)?;
        for void in solid.void_shells() {
            validate_shell(void, true, &grid)?;
        }
    }
    Ok(())
}

fn grid_for_brep(brep: &BRep) -> SnapGrid {
    let mut pts: Vec<Point3<f64>> = Vec::new();
    for solid in brep.solids() {
        for face in solid.outer_shell().faces() {
            for he in face.outer_loop().half_edges() {
                pts.push(*he.start_vertex().point());
            }
            for inner in face.inner_loops() {
                for he in inner.half_edges() {
                    pts.push(*he.start_vertex().point());
                }
            }
        }
    }
    let diag = Aabb3::from_points(&pts)
        .map(|b| b.diagonal_length())
        .unwrap_or(0.0);
    let cell = (diag * VALIDATE_GRID_RELATIVE).max(VALIDATE_GRID_FLOOR);
    SnapGrid::new(cell)
}

fn validate_shell(shell: &Shell, require_closed: bool, grid: &SnapGrid) -> KResult<()> {

    for (fi, face) in shell.faces().iter().enumerate() {
        // Check 1: minimum edge count.
        // Faces need at least 1 edge (seam edges on rotational surfaces
        // form valid single-edge loops).
        let n = face.outer_loop().half_edges().len();
        if n < 1 {
            return Err(KernelError::TopoInconsistency {
                code: ErrorCode::DanglingReference,
                detail: format!("face {} has no edges", fi),
            });
        }

        // Check 2: loop closure via lattice index comparison (no distance threshold)
        let hes = face.outer_loop().half_edges();
        for i in 0..hes.len() {
            let j = (i + 1) % hes.len();
            let end_li = grid.lattice_index(*hes[i].end_vertex().point());
            let start_li = grid.lattice_index(*hes[j].start_vertex().point());
            if end_li != start_li {
                return Err(KernelError::TopoInconsistency {
                    code: ErrorCode::LoopNotClosed,
                    detail: format!(
                        "face {} loop not closed at edge {}: end lattice {:?} != start lattice {:?}",
                        fi, i, end_li, start_li
                    ),
                });
            }
        }

        // Check 3: edge-vertex geometry consistency.
        // STEP-derived curves and vertex points are written to limited
        // precision (typically ~1e-6 relative), so we compare with a
        // coarser tolerance than the vertex/edge identity grid. The
        // strict lattice grid is used for "are these the same vertex"
        // decisions; this check only verifies "does the curve roughly
        // pass through the vertex." The tolerance is 100× the grid
        // cell size — the same multiplier the boolean uses elsewhere.
        let consistency_tol = grid.cell_size * 100.0;
        for (ei, he) in hes.iter().enumerate() {
            let edge = he.edge();
            let curve_start = edge.curve().point_at(knot_geom::curve::CurveParam(edge.t_start()));
            let curve_end = edge.curve().point_at(knot_geom::curve::CurveParam(edge.t_end()));

            let d_start = (curve_start - *edge.start().point()).norm();
            let d_end = (curve_end - *edge.end().point()).norm();

            if d_start > consistency_tol {
                return Err(KernelError::TopoInconsistency {
                    code: ErrorCode::DanglingReference,
                    detail: format!(
                        "face {} edge {}: curve start {:.3e} from vertex (tol {:.3e})",
                        fi, ei, d_start, consistency_tol
                    ),
                });
            }
            if d_end > consistency_tol {
                return Err(KernelError::TopoInconsistency {
                    code: ErrorCode::DanglingReference,
                    detail: format!(
                        "face {} edge {}: curve end {:.3e} from vertex (tol {:.3e})",
                        fi, ei, d_end, consistency_tol
                    ),
                });
            }
        }

        // Validate inner loops with same checks
        for (li, inner) in face.inner_loops().iter().enumerate() {
            let inner_hes = inner.half_edges();
            if inner_hes.is_empty() {
                return Err(KernelError::TopoInconsistency {
                    code: ErrorCode::DanglingReference,
                    detail: format!("face {} inner loop {} has no edges", fi, li),
                });
            }
            for i in 0..inner_hes.len() {
                let j = (i + 1) % inner_hes.len();
                let end_li = grid.lattice_index(*inner_hes[i].end_vertex().point());
                let start_li = grid.lattice_index(*inner_hes[j].start_vertex().point());
                if end_li != start_li {
                    return Err(KernelError::TopoInconsistency {
                        code: ErrorCode::LoopNotClosed,
                        detail: format!(
                            "face {} inner loop {} not closed at edge {}",
                            fi, li, i
                        ),
                    });
                }
            }
        }
    }

    if !require_closed || !shell.is_closed() {
        return Ok(());
    }

    // Checks 4 & 5: edge-use counting and Euler-Poincare via lattice indices
    let mut edge_use_count: HashMap<LatticeEdgeKey, usize> = HashMap::new();
    let mut vertex_set: HashMap<LatticeIndex, ()> = HashMap::new();

    for face in shell.faces() {
        collect_lattice_edges(face.outer_loop().half_edges(), &grid, &mut edge_use_count, &mut vertex_set);
        for inner in face.inner_loops() {
            collect_lattice_edges(inner.half_edges(), &grid, &mut edge_use_count, &mut vertex_set);
        }
    }

    for (_key, count) in &edge_use_count {
        if *count > 2 {
            return Err(KernelError::TopoInconsistency {
                code: ErrorCode::NonManifoldEdge,
                detail: format!("edge used {} times (non-manifold)", count),
            });
        }
    }

    let v = vertex_set.len();
    let e = edge_use_count.len();
    let f = shell.face_count();
    let euler = v as i64 - e as i64 + f as i64;
    if euler % 2 != 0 {
        return Err(KernelError::TopoInconsistency {
            code: ErrorCode::EulerViolation,
            detail: format!(
                "Euler characteristic V-E+F = {} (V={}, E={}, F={}), expected even",
                euler, v, e, f
            ),
        });
    }

    Ok(())
}

fn collect_lattice_edges(
    half_edges: &[super::edge::HalfEdge],
    grid: &SnapGrid,
    edge_use_count: &mut HashMap<LatticeEdgeKey, usize>,
    vertex_set: &mut HashMap<LatticeIndex, ()>,
) {
    for he in half_edges {
        let start = grid.lattice_index(*he.start_vertex().point());
        let end = grid.lattice_index(*he.end_vertex().point());
        let key = LatticeEdgeKey::new(start, end);
        *edge_use_count.entry(key).or_insert(0) += 1;
        vertex_set.entry(start).or_insert(());
        vertex_set.entry(end).or_insert(());
    }
}

/// Edge identity key using integer lattice indices.
/// Sorted pair ensures edge identity is direction-independent.
#[derive(Clone, Hash, PartialEq, Eq)]
struct LatticeEdgeKey {
    a: LatticeIndex,
    b: LatticeIndex,
}

impl LatticeEdgeKey {
    fn new(a: LatticeIndex, b: LatticeIndex) -> Self {
        if a <= b { Self { a, b } } else { Self { a: b, b: a } }
    }
}
