//! Coordinate frames along curves for sweep operations.
//!
//! Uses parallel transport (discrete Bishop frame) to propagate a
//! twist-free frame along a curve.  This avoids the singularities of
//! the Frenet frame at inflection points.

use crate::curve::{Curve, CurveParam};
use crate::point::{Point3, Vector3};
use crate::transform;

/// A coordinate frame at a point along a curve.
#[derive(Clone, Debug)]
pub struct CurveFrame {
    /// Position on the curve.
    pub origin: Point3,
    /// Unit tangent along the curve.
    pub tangent: Vector3,
    /// Unit normal perpendicular to tangent (frame "up").
    pub normal: Vector3,
    /// Unit binormal = tangent × normal (frame "right").
    pub binormal: Vector3,
}

/// Sample `n + 1` frames along a curve using parallel transport.
///
/// Frames are evenly spaced in parameter space.  The initial normal is
/// chosen perpendicular to the tangent at `t = domain.start` via a
/// stable heuristic (cross with the least-aligned world axis).
pub fn sample_frames(curve: &Curve, n: usize) -> Vec<CurveFrame> {
    assert!(n >= 1, "need at least 1 segment");

    let domain = curve.domain();
    let dt = (domain.end - domain.start) / n as f64;

    let mut frames = Vec::with_capacity(n + 1);

    // First frame.
    let d0 = curve.derivatives_at(CurveParam(domain.start));
    let t0 = d0.d1.normalize();
    let n0 = stable_perpendicular(&t0);
    let b0 = t0.cross(&n0);
    frames.push(CurveFrame {
        origin: d0.point,
        tangent: t0,
        normal: n0,
        binormal: b0,
    });

    // Propagate via parallel transport.
    for i in 1..=n {
        let t_param = domain.start + i as f64 * dt;
        let deriv = curve.derivatives_at(CurveParam(t_param));
        let new_t = deriv.d1.normalize();

        let prev = &frames[i - 1];
        let new_n = transport(&prev.tangent, &new_t, &prev.normal);
        let new_b = new_t.cross(&new_n).normalize();
        // Re-orthogonalise normal after transport to fight drift.
        let new_n = new_b.cross(&new_t).normalize();

        frames.push(CurveFrame {
            origin: deriv.point,
            tangent: new_t,
            normal: new_n,
            binormal: new_b,
        });
    }

    frames
}

/// Choose a unit vector perpendicular to `v`.
fn stable_perpendicular(v: &Vector3) -> Vector3 {
    let perp = if v.x.abs() < 0.9 {
        Vector3::x().cross(v)
    } else {
        Vector3::y().cross(v)
    };
    perp.normalize()
}

/// Parallel-transport `old_normal` from `old_tangent` to `new_tangent`.
///
/// Applies the minimal rotation that maps one tangent to the other,
/// then applies the same rotation to the normal.
fn transport(old_t: &Vector3, new_t: &Vector3, old_n: &Vector3) -> Vector3 {
    let axis = old_t.cross(new_t);
    let sin_a = axis.norm();
    if sin_a < 1e-12 {
        // Nearly parallel — check for 180° flip.
        if old_t.dot(new_t) < -0.999 {
            return -*old_n;
        }
        return *old_n;
    }
    let axis_unit = axis / sin_a;
    let cos_a = old_t.dot(new_t);
    let angle = sin_a.atan2(cos_a);
    let rot = transform::rotation(axis_unit, angle);
    transform::transform_vector(&rot, old_n)
}
