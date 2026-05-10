//! Constant-distance curve offset.
//!
//! For a curve `c` and signed distance `d`, the offset curve is the locus
//! of points `c(t) + d * (plane_normal × tangent(t))`.
//!
//! Exact offsets exist for lines (still a line) and circular arcs (still an
//! arc, with a shifted radius). NURBS and elliptical arcs do not have
//! exact offsets of the same type — the true offset is a higher-degree
//! curve in general — so they're rejected here. Approximation by NURBS
//! refit is a separate problem; we'd rather fail loudly than silently
//! degrade.

use knot_core::{ErrorCode, KResult, KernelError};

use crate::point::Vector3;
use super::{Curve, CircularArc, LineSeg};

const PARALLEL_TOL: f64 = 1.0e-6;

/// Offset a curve by `distance` in the plane with the given normal.
///
/// The offset direction at parameter `t` is `plane_normal × tangent(t)`.
/// Positive distance offsets in that direction; negative distance offsets
/// the other way. A zero-length tangent or a degenerate plane (zero
/// `plane_normal`) returns `MalformedInput`.
pub fn offset(curve: &Curve, distance: f64, plane_normal: Vector3) -> KResult<Curve> {
    let pn_norm = plane_normal.norm();
    if pn_norm < 1e-15 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: "offset plane normal is zero-length".into(),
        });
    }
    let pn = plane_normal / pn_norm;

    match curve {
        Curve::Line(line) => offset_line(line, distance, pn),
        Curve::CircularArc(arc) => offset_arc(arc, distance, pn),
        Curve::EllipticalArc(_) | Curve::Nurbs(_) => Err(KernelError::OperationFailed {
            code: ErrorCode::UnsupportedConfiguration,
            detail: "exact offset is only defined for line and circular-arc curves; \
                     NURBS / elliptical-arc offset requires sample-and-fit, not yet implemented"
                .into(),
        }),
    }
}

fn offset_line(line: &LineSeg, distance: f64, plane_normal: Vector3) -> KResult<Curve> {
    let dir = line.direction();
    let len = dir.norm();
    if len < 1e-15 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: "cannot offset a zero-length line".into(),
        });
    }
    let tangent = dir / len;
    // The offset direction must be perpendicular to the tangent. If
    // `plane_normal` is parallel to the tangent the cross product is zero
    // and the offset is undefined.
    let cross = plane_normal.cross(&tangent);
    let cross_len = cross.norm();
    if cross_len < 1e-12 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: "offset plane normal is parallel to the line direction".into(),
        });
    }
    let offset_dir = cross / cross_len;
    let shift = offset_dir * distance;
    Ok(Curve::Line(LineSeg::new(line.start + shift, line.end + shift)))
}

fn offset_arc(arc: &CircularArc, distance: f64, plane_normal: Vector3) -> KResult<Curve> {
    let arc_n = arc.normal.norm();
    if arc_n < 1e-15 {
        return Err(KernelError::InvalidGeometry {
            code: ErrorCode::DegenerateCurve,
            detail: "arc normal is zero-length".into(),
        });
    }
    let arc_normal_unit = arc.normal / arc_n;

    // The arc lies in a fixed plane. A constant offset stays a circle only
    // when the offset plane matches the arc plane (up to sign). Otherwise
    // the offset distance varies along the arc and the result is not a
    // circular arc.
    let alignment = arc_normal_unit.dot(&plane_normal);
    if (alignment.abs() - 1.0).abs() > PARALLEL_TOL {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: "arc offset requires the offset plane normal to be parallel to the arc plane normal".into(),
        });
    }

    // Sign of plane_normal relative to arc.normal flips the offset side.
    let signed_distance = if alignment >= 0.0 { distance } else { -distance };

    // Derivation in arc.rs notation: tangent at t is
    //     -sin(t)*ref + cos(t)*binormal
    // and `plane_normal × tangent` resolves to `-(cos(t)*ref + sin(t)*binormal)`,
    // i.e. the inward radial direction. So a positive offset shrinks the
    // radius; a negative offset enlarges it.
    let new_radius = arc.radius - signed_distance;
    if new_radius <= 1e-12 {
        return Err(KernelError::InvalidInput {
            code: ErrorCode::MalformedInput,
            detail: format!(
                "arc offset would collapse or invert the radius (orig={}, distance={}, new={})",
                arc.radius, distance, new_radius
            ),
        });
    }

    Ok(Curve::CircularArc(CircularArc {
        center: arc.center,
        normal: arc.normal,
        radius: new_radius,
        ref_direction: arc.ref_direction,
        start_angle: arc.start_angle,
        end_angle: arc.end_angle,
    }))
}
