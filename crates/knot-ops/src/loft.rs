//! Loft operations: connect a sequence of planar profiles with ruled side
//! surfaces and planar end caps.
//!
//! v1 scope:
//!   - Outer loops only (inner loops on profiles are ignored for now).
//!   - All profiles must share the same outer-loop vertex count;
//!     vertex `i` of profile `k` is connected to vertex `i` of profile `k+1`.
//!     Resampling and curve-matching are deferred — they're a separate quality
//!     problem from getting the topology right.
//!   - Side faces are flat quads (ruled). Tangency / smooth surface
//!     fitting through all sections is deferred (would produce a NURBS
//!     skin and need its own SSI plumbing).
//!
//! Inputs that don't satisfy these constraints fail fast with
//! `KernelError::InvalidInput` rather than silently coercing.

use std::sync::Arc;

use knot_core::{ErrorCode, KResult, KernelError};
use knot_geom::surface::{Plane, Surface};
use knot_geom::{Point3, Vector3};
use knot_topo::*;

use crate::extrude::{first_face, loop_points, make_quad_face, newell_normal, safe_normalize};
use crate::topo_builder::line_he;

/// Loft through a sequence of planar profile BReps.
///
/// Returns a single closed solid bounded by:
///   - planar caps from the first and last profile faces,
///   - ruled quad strips between consecutive profile pairs.
///
/// Errors:
///   - fewer than two profiles,
///   - any profile with fewer than three outer-loop vertices,
///   - profiles with mismatched outer-loop vertex counts,
///   - any profile whose outer loop is degenerate (collinear).
pub fn loft(profiles: &[BRep]) -> KResult<BRep> {
    if profiles.len() < 2 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: "loft requires at least two profiles".into(),
        });
    }

    // ── Extract outer loops, validate matching vertex counts ────────
    let rings_pts: Vec<Vec<Point3>> = profiles
        .iter()
        .map(|p| {
            let face = first_face(p)?;
            let pts = loop_points(face.outer_loop());
            if pts.len() < 3 {
                return Err(KernelError::InvalidInput {
                    code: ErrorCode::MalformedInput,
                    detail: "loft profile must have at least 3 vertices".into(),
                });
            }
            Ok(pts)
        })
        .collect::<KResult<Vec<_>>>()?;

    let nv = rings_pts[0].len();
    for (i, r) in rings_pts.iter().enumerate().skip(1) {
        if r.len() != nv {
            return Err(KernelError::InvalidInput {
                code: ErrorCode::MalformedInput,
                detail: format!(
                    "loft profiles must share vertex count: profile 0 has {nv}, profile {i} has {}",
                    r.len()
                ),
            });
        }
    }

    // Validate each profile is non-degenerate and capture its plane normal.
    let normals: Vec<Vector3> = rings_pts
        .iter()
        .enumerate()
        .map(|(i, pts)| {
            let n = newell_normal(pts);
            if n.norm() < 1e-15 {
                return Err(KernelError::InvalidGeometry {
                    code: ErrorCode::DegenerateCurve,
                    detail: format!("loft profile {i} is degenerate (collinear vertices)"),
                });
            }
            Ok(safe_normalize(n))
        })
        .collect::<KResult<Vec<_>>>()?;

    // ── Materialize vertex rings ────────────────────────────────────
    let rings: Vec<Vec<Arc<Vertex>>> = rings_pts
        .iter()
        .map(|pts| pts.iter().map(|p| Arc::new(Vertex::new(*p))).collect())
        .collect();

    // Centre of all ring vertices, used for outward-facing orientation
    // checks on each side quad.
    let solid_center = {
        let total: usize = rings.len() * nv;
        let sum: Vector3 = rings
            .iter()
            .flat_map(|r| r.iter().map(|v| v.point().coords))
            .fold(Vector3::zeros(), |a, c| a + c);
        Point3::from(sum / total as f64)
    };

    // ── Side faces between consecutive rings ────────────────────────
    let mut faces: Vec<Face> = Vec::with_capacity((rings.len() - 1) * nv + 2);
    for k in 0..rings.len() - 1 {
        for i in 0..nv {
            let j = (i + 1) % nv;
            let a0 = &rings[k][i];
            let a1 = &rings[k + 1][i];
            let b0 = &rings[k][j];
            let b1 = &rings[k + 1][j];
            faces.push(make_quad_face(a0, a1, b1, b0, &solid_center)?);
        }
    }

    // ── Caps ────────────────────────────────────────────────────────
    // Start cap (first profile): emit loop in source order; flip the plane
    // normal away from `solid_center` so the cap points outward.
    faces.push(planar_cap(&rings[0], normals[0], &solid_center, false)?);

    // End cap (last profile): reverse winding so the loop traverses the
    // opposite direction relative to the side quads, then orient outward.
    let last = rings.len() - 1;
    faces.push(planar_cap(&rings[last], normals[last], &solid_center, true)?);

    let shell = Shell::new(faces, true)?;
    let solid = Solid::new(shell, vec![])?;
    BRep::new(vec![solid])
}

/// Build a planar cap face from a vertex ring.
///
/// `outward_hint` is the profile's plane normal in source winding order.
/// We flip it (and reverse the loop) if it doesn't point away from the
/// solid centre, so the resulting face has a consistent outward normal.
///
/// `reverse` swaps source-order traversal — used so the *end* cap winds
/// opposite to the start cap before orientation correction.
fn planar_cap(
    ring: &[Arc<Vertex>],
    outward_hint: Vector3,
    solid_center: &Point3,
    reverse: bool,
) -> KResult<Face> {
    let n = ring.len();

    // Centroid of the cap loop, used to decide whether `outward_hint`
    // already points outward.
    let cap_center = {
        let sum: Vector3 = ring.iter().map(|v| v.point().coords).fold(Vector3::zeros(), |a, c| a + c);
        Point3::from(sum / n as f64)
    };
    let outward = if (cap_center - solid_center).dot(&outward_hint) >= 0.0 {
        outward_hint
    } else {
        -outward_hint
    };

    // Build the half-edge loop. To get the correct winding for `outward`,
    // walk the ring forward if `(b-a) × (c-a)` agrees with `outward`,
    // otherwise walk it backward.
    let forward = {
        let a = ring[0].point();
        let b = ring[1].point();
        let c = ring[2].point();
        let wn = (b - a).cross(&(c - a));
        wn.dot(&outward) >= 0.0
    };
    let walk_forward = forward ^ reverse;

    let edges: Vec<HalfEdge> = if walk_forward {
        (0..n).map(|i| line_he(&ring[i], &ring[(i + 1) % n])).collect()
    } else {
        (0..n)
            .rev()
            .map(|i| {
                let prev = if i == 0 { n - 1 } else { i - 1 };
                line_he(&ring[i], &ring[prev])
            })
            .collect()
    };

    let lp = Loop::new(edges, true)?;
    let surface = Arc::new(Surface::Plane(Plane::new(*ring[0].point(), outward)));
    Face::new(surface, lp, vec![], true)
}
