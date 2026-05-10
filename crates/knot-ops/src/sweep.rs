//! Sweep operations: sweep a profile along one or two rail curves.

use std::sync::Arc;

use knot_core::{ErrorCode, KResult, KernelError};
use knot_geom::curve::{Curve, CurveParam};
use knot_geom::frame;
use knot_geom::surface::{Plane, Surface};
use knot_geom::{Point3, Vector3};
use knot_topo::*;

use crate::extrude::{first_face, loop_points, make_quad_face, newell_normal, safe_normalize};
use crate::topo_builder::line_he;

/// Sweep a profile along a single rail curve.
///
/// The profile (first face of the BRep) is placed perpendicular to the
/// rail at each sample point using a parallel-transport frame, then
/// adjacent positions are connected with quad faces.
///
/// If the rail is closed (endpoints coincide), the result is a closed
/// tube with no cap faces.  Otherwise planar caps are added at each end.
pub fn sweep_1rail(profile: &BRep, rail: &Curve) -> KResult<BRep> {
    let domain = rail.domain();
    if (domain.end - domain.start).abs() < 1e-15 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: "rail curve has zero-length domain".into(),
        });
    }

    // ── extract profile ──────────────────────────────────────────────
    let face = first_face(profile)?;
    let pts = loop_points(face.outer_loop());
    let nv = pts.len();
    if nv < 3 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: "profile must have at least 3 vertices".into(),
        });
    }

    let pn = newell_normal(&pts);
    if pn.norm() < 1e-15 {
        return Err(KernelError::InvalidGeometry {
            code: ErrorCode::DegenerateCurve,
            detail: "profile is degenerate (collinear vertices)".into(),
        });
    }
    let pn = pn.normalize();

    // ── profile local coordinates ────────────────────────────────────
    // Build an orthonormal basis in the profile plane and express each
    // vertex as a 2D offset from the centroid.
    let centroid = {
        let sum = pts.iter().fold(Vector3::zeros(), |a, p| a + p.coords);
        Point3::from(sum / nv as f64)
    };
    let u_axis = {
        let u = if pn.x.abs() < 0.9 {
            Vector3::x().cross(&pn)
        } else {
            Vector3::y().cross(&pn)
        };
        u.normalize()
    };
    let v_axis = pn.cross(&u_axis);

    let offsets: Vec<[f64; 2]> = pts
        .iter()
        .map(|p| {
            let d = p - centroid;
            [d.dot(&u_axis), d.dot(&v_axis)]
        })
        .collect();

    // Inner loop offsets.
    let inner_loops: Vec<Vec<[f64; 2]>> = face
        .inner_loops()
        .iter()
        .filter(|il| il.half_edges().len() >= 3)
        .map(|il| {
            il.half_edges()
                .iter()
                .map(|he| {
                    let d = he.start_vertex().point() - centroid;
                    [d.dot(&u_axis), d.dot(&v_axis)]
                })
                .collect()
        })
        .collect();

    // ── sample frames along rail ─────────────────────────────────────
    let n_seg = segment_count(rail);
    let frames = frame::sample_frames(rail, n_seg);

    let rail_start = rail.point_at(CurveParam(domain.start));
    let rail_end = rail.point_at(CurveParam(domain.end));
    let closed = (rail_end - rail_start).norm() < 1e-10;

    let n_rings = if closed { n_seg } else { n_seg + 1 };

    // ── build vertex rings ───────────────────────────────────────────
    let build_rings = |offs: &[[f64; 2]]| -> Vec<Vec<Arc<Vertex>>> {
        (0..n_rings)
            .map(|k| {
                let f = &frames[k];
                offs.iter()
                    .map(|&[u, v]| {
                        let pos = f.origin + u * f.normal + v * f.binormal;
                        Arc::new(Vertex::new(pos))
                    })
                    .collect()
            })
            .collect()
    };

    let rings = build_rings(&offsets);
    let inner_rings: Vec<Vec<Vec<Arc<Vertex>>>> =
        inner_loops.iter().map(|il| build_rings(il)).collect();

    // ── solid centre for orientation checks ──────────────────────────
    let solid_center = {
        let sum: Vector3 = rings
            .iter()
            .flat_map(|r| r.iter().map(|v| v.point().coords))
            .fold(Vector3::zeros(), |a, c| a + c);
        Point3::from(sum / (n_rings * nv) as f64)
    };

    // ── side faces ───────────────────────────────────────────────────
    let mut faces = Vec::new();

    for i in 0..nv {
        let j = (i + 1) % nv;
        for k in 0..n_seg {
            let kn = if closed { (k + 1) % n_seg } else { k + 1 };
            let a0 = &rings[k][i];
            let a1 = &rings[kn][i];
            let b0 = &rings[k][j];
            let b1 = &rings[kn][j];
            faces.push(make_quad_face(a0, a1, b1, b0, &solid_center)?);
        }
    }

    // ── inner side quads (tunnel walls) ─────────────────────────────
    for ir in &inner_rings {
        let m = ir[0].len();
        for i in 0..m {
            let j = (i + 1) % m;
            for k in 0..n_seg {
                let kn = if closed { (k + 1) % n_seg } else { k + 1 };
                let a0 = &ir[k][i];
                let a1 = &ir[kn][i];
                let b0 = &ir[k][j];
                let b1 = &ir[kn][j];
                faces.push(make_quad_face(a0, a1, b1, b0, &solid_center)?);
            }
        }
    }

    // ── cap faces (open rail only) ───────────────────────────────────
    if !closed {
        // Start cap outer loop.
        let sc: Vec<HalfEdge> =
            (0..nv).map(|i| line_he(&rings[0][i], &rings[0][(i + 1) % nv])).collect();
        let sn = newell_normal(
            &sc.iter().map(|h| *h.start_vertex().point()).collect::<Vec<_>>(),
        );
        let sl = Loop::new(sc, true)?;
        // Start cap inner loops.
        let s_inner: Vec<Loop> = inner_rings
            .iter()
            .map(|ir| {
                let m = ir[0].len();
                let edges: Vec<HalfEdge> =
                    (0..m).map(|i| line_he(&ir[0][i], &ir[0][(i + 1) % m])).collect();
                Loop::new(edges, false)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let ss = Arc::new(Surface::Plane(Plane::new(
            *rings[0][0].point(),
            safe_normalize(sn),
        )));
        faces.push(Face::new(ss, sl, s_inner, true)?);

        // End cap outer loop (reversed winding).
        let ec: Vec<HalfEdge> = (0..nv)
            .rev()
            .map(|i| {
                let j = if i == 0 { nv - 1 } else { i - 1 };
                line_he(&rings[n_seg][i], &rings[n_seg][j])
            })
            .collect();
        let en = newell_normal(
            &ec.iter().map(|h| *h.start_vertex().point()).collect::<Vec<_>>(),
        );
        let el = Loop::new(ec, true)?;
        // End cap inner loops (reversed).
        let e_inner: Vec<Loop> = inner_rings
            .iter()
            .map(|ir| {
                let m = ir[0].len();
                let edges: Vec<HalfEdge> = (0..m)
                    .rev()
                    .map(|i| {
                        let j = if i == 0 { m - 1 } else { i - 1 };
                        line_he(&ir[n_seg][i], &ir[n_seg][j])
                    })
                    .collect();
                Loop::new(edges, false)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let es = Arc::new(Surface::Plane(Plane::new(
            *rings[n_seg][0].point(),
            safe_normalize(en),
        )));
        faces.push(Face::new(es, el, e_inner, true)?);
    }

    let shell = Shell::new(faces, true)?;
    let solid = Solid::new(shell, vec![])?;
    BRep::new(vec![solid])
}

/// Sweep a profile along two rail curves.
pub fn sweep_2rail(_profile: &BRep, _rail_a: &Curve, _rail_b: &Curve) -> KResult<BRep> {
    Err(KernelError::OperationFailed {
        code: ErrorCode::UnsupportedConfiguration,
        detail: "2-rail sweep not yet implemented".into(),
    })
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// Choose segment count based on rail curve type.
fn segment_count(rail: &Curve) -> usize {
    match rail {
        Curve::Line(_) => 1,
        Curve::CircularArc(a) => {
            let span = (a.end_angle - a.start_angle).abs();
            (24.0 * span / std::f64::consts::TAU).ceil().max(1.0) as usize
        }
        Curve::EllipticalArc(a) => {
            let span = (a.end_angle - a.start_angle).abs();
            (24.0 * span / std::f64::consts::TAU).ceil().max(1.0) as usize
        }
        Curve::Nurbs(_) => 24,
    }
}
