use knot_core::KResult;
use knot_geom::Point3;
use knot_geom::curve::{Curve, CurveParam, LineSeg};
use knot_geom::surface::{Surface, SurfaceParam, Plane, Sphere, Cylinder};
use super::CurveSurfaceHit;

/// Compute intersections between a curve and a surface.
pub fn intersect_curve_surface(
    curve: &Curve,
    surface: &Surface,
    tolerance: f64,
) -> KResult<Vec<CurveSurfaceHit>> {
    // Analytical fast-paths
    match (curve, surface) {
        (Curve::Line(line), Surface::Plane(plane)) => line_plane(line, plane, tolerance),
        (Curve::Line(line), Surface::Sphere(sphere)) => line_sphere(line, sphere, tolerance),
        (Curve::Line(line), Surface::Cylinder(cyl)) => line_cylinder(line, cyl, tolerance),
        _ => general_curve_surface(curve, surface, tolerance),
    }
}

/// Line-plane intersection. Returns 0 or 1 hits.
fn line_plane(line: &LineSeg, plane: &Plane, tolerance: f64) -> KResult<Vec<CurveSurfaceHit>> {
    let d = line.direction();
    let denom = d.dot(&plane.normal);

    if denom.abs() < 1e-15 {
        // Line parallel to plane — no intersection (or coincident, which we skip)
        return Ok(Vec::new());
    }

    let t = (plane.origin - line.start).dot(&plane.normal) / denom;

    if t < -tolerance || t > 1.0 + tolerance {
        return Ok(Vec::new());
    }

    let t = t.clamp(0.0, 1.0);
    let point = line.point_at(t);
    let v = point - plane.origin;
    let u_param = v.dot(&plane.u_axis);
    let v_param = v.dot(&plane.v_axis);

    Ok(vec![CurveSurfaceHit {
        point,
        curve_param: CurveParam(t),
        surface_param: SurfaceParam { u: u_param, v: v_param },
    }])
}

/// Line-sphere intersection. Returns 0, 1, or 2 hits.
fn line_sphere(line: &LineSeg, sphere: &Sphere, tolerance: f64) -> KResult<Vec<CurveSurfaceHit>> {
    let d = line.direction();
    let oc = line.start - sphere.center;

    let a = d.dot(&d);
    let b = 2.0 * oc.dot(&d);
    let c = oc.dot(&oc) - sphere.radius * sphere.radius;
    let disc = b * b - 4.0 * a * c;

    if disc < -tolerance {
        return Ok(Vec::new());
    }

    let disc = disc.max(0.0);
    let sqrt_disc = disc.sqrt();
    let mut hits = Vec::new();

    for &t in &[(-b - sqrt_disc) / (2.0 * a), (-b + sqrt_disc) / (2.0 * a)] {
        if t < -tolerance || t > 1.0 + tolerance {
            continue;
        }
        let t = t.clamp(0.0, 1.0);
        let point = line.point_at(t);
        let n = (point - sphere.center).normalize();
        // Convert to spherical UV
        let u = n.y.atan2(n.x).rem_euclid(std::f64::consts::TAU);
        let v = n.z.asin();
        hits.push(CurveSurfaceHit {
            point,
            curve_param: CurveParam(t),
            surface_param: SurfaceParam { u, v },
        });
    }

    // Deduplicate tangent hits
    if hits.len() == 2 && (hits[0].curve_param.0 - hits[1].curve_param.0).abs() < tolerance {
        hits.pop();
    }

    Ok(hits)
}

/// Line-cylinder intersection. Returns 0, 1, or 2 hits.
fn line_cylinder(line: &LineSeg, cyl: &Cylinder, tolerance: f64) -> KResult<Vec<CurveSurfaceHit>> {
    let d = line.direction();
    let oc = line.start - cyl.origin;

    // Project onto plane perpendicular to axis
    let d_perp = d - cyl.axis * d.dot(&cyl.axis);
    let oc_perp = oc - cyl.axis * oc.dot(&cyl.axis);

    let a = d_perp.dot(&d_perp);
    let b = 2.0 * oc_perp.dot(&d_perp);
    let c = oc_perp.dot(&oc_perp) - cyl.radius * cyl.radius;
    let disc = b * b - 4.0 * a * c;

    if disc < -tolerance || a < 1e-30 {
        return Ok(Vec::new());
    }

    let disc = disc.max(0.0);
    let sqrt_disc = disc.sqrt();
    let mut hits = Vec::new();

    for &t in &[(-b - sqrt_disc) / (2.0 * a), (-b + sqrt_disc) / (2.0 * a)] {
        if t < -tolerance || t > 1.0 + tolerance {
            continue;
        }
        let t = t.clamp(0.0, 1.0);
        let point = line.point_at(t);

        // Get cylinder UV params
        let local = point - cyl.origin;
        let v_param = local.dot(&cyl.axis);
        if v_param < cyl.v_min - tolerance || v_param > cyl.v_max + tolerance {
            continue;
        }
        let binormal = cyl.axis.cross(&cyl.ref_direction);
        let u_comp = local.dot(&cyl.ref_direction);
        let v_comp = local.dot(&binormal);
        let u_param = v_comp.atan2(u_comp).rem_euclid(std::f64::consts::TAU);

        hits.push(CurveSurfaceHit {
            point,
            curve_param: CurveParam(t),
            surface_param: SurfaceParam { u: u_param, v: v_param },
        });
    }

    if hits.len() == 2 && (hits[0].curve_param.0 - hits[1].curve_param.0).abs() < tolerance {
        hits.pop();
    }

    Ok(hits)
}

/// General curve-surface intersection using subdivision + Newton refinement.
fn general_curve_surface(
    curve: &Curve,
    surface: &Surface,
    tolerance: f64,
) -> KResult<Vec<CurveSurfaceHit>> {
    let c_domain = curve.domain();
    let s_domain = surface.domain();

    // Clamp surface domain for sampling (infinite planes get bounded)
    let s_u_lo = s_domain.u_start.max(-100.0);
    let s_u_hi = s_domain.u_end.min(100.0);
    let s_v_lo = s_domain.v_start.max(-100.0);
    let s_v_hi = s_domain.v_end.min(100.0);

    // Phase 1: sample the curve and find closest points on surface as starting guesses
    let n_samples = 32;
    let dt = (c_domain.end - c_domain.start) / n_samples as f64;

    let mut candidates = Vec::new();

    for i in 0..=n_samples {
        let t = c_domain.start + dt * i as f64;
        let p = curve.point_at(CurveParam(t));

        // Simple grid search on surface for closest point to p
        let nu = 8;
        let nv = 8;
        let du = (s_u_hi - s_u_lo) / nu as f64;
        let dv = (s_v_hi - s_v_lo) / nv as f64;
        let mut best_dist = f64::MAX;
        let mut best_uv = SurfaceParam { u: 0.0, v: 0.0 };

        for iu in 0..=nu {
            for iv in 0..=nv {
                let uv = SurfaceParam {
                    u: s_u_lo + du * iu as f64,
                    v: s_v_lo + dv * iv as f64,
                };
                let sp = surface.point_at(uv);
                let d = (sp - p).norm();
                if d < best_dist {
                    best_dist = d;
                    best_uv = uv;
                }
            }
        }

        if best_dist < tolerance * 100.0 {
            candidates.push((t, best_uv));
        }
    }

    // Phase 2: Newton refinement
    let mut hits = Vec::new();
    for (t_init, uv_init) in &candidates {
        if let Some(hit) = newton_curve_surface(curve, surface, *t_init, *uv_init, tolerance) {
            hits.push(hit);
        }
    }

    // Phase 3: Deduplicate
    hits.sort_by(|a, b| a.curve_param.0.partial_cmp(&b.curve_param.0).unwrap());
    let mut deduped = Vec::new();
    for hit in hits {
        if deduped.last().map_or(true, |last: &CurveSurfaceHit|
            (hit.curve_param.0 - last.curve_param.0).abs() > tolerance * 10.0
        ) {
            deduped.push(hit);
        }
    }

    Ok(deduped)
}

/// Newton iteration for curve-surface intersection.
/// Solves: C(t) = S(u, v) for (t, u, v).
fn newton_curve_surface(
    curve: &Curve,
    surface: &Surface,
    mut t: f64,
    mut uv: SurfaceParam,
    tolerance: f64,
) -> Option<CurveSurfaceHit> {
    let c_domain = curve.domain();
    let s_domain = surface.domain();

    for _ in 0..30 {
        let cd = curve.derivatives_at(CurveParam(t));
        let sd = surface.derivatives_at(uv);
        let diff = cd.point - sd.point;

        if diff.norm() < tolerance {
            return Some(CurveSurfaceHit {
                point: cd.point,
                curve_param: CurveParam(t),
                surface_param: uv,
            });
        }

        // System: diff = 0, where diff = C(t) - S(u,v)
        // Jacobian columns: dC/dt, -dS/du, -dS/dv
        // We solve J * [dt, du, dv]^T = -diff using least-squares (3x3)
        let j0 = cd.d1;      // dC/dt
        let j1 = -sd.du;     // -dS/du
        let j2 = -sd.dv;     // -dS/dv

        // Build normal equations: J^T J x = J^T (-diff)
        let jtj = nalgebra::Matrix3::new(
            j0.dot(&j0), j0.dot(&j1), j0.dot(&j2),
            j1.dot(&j0), j1.dot(&j1), j1.dot(&j2),
            j2.dot(&j0), j2.dot(&j1), j2.dot(&j2),
        );
        let rhs = nalgebra::Vector3::new(
            -j0.dot(&diff),
            -j1.dot(&diff),
            -j2.dot(&diff),
        );

        let det = jtj.determinant();
        if det.abs() < 1e-30 {
            break;
        }

        if let Some(inv) = jtj.try_inverse() {
            let delta = inv * rhs;
            t += delta.x;
            uv.u += delta.y;
            uv.v += delta.z;

            t = t.clamp(c_domain.start, c_domain.end);
            // Only clamp for finite domains
            if s_domain.u_start.is_finite() {
                uv.u = uv.u.clamp(s_domain.u_start, s_domain.u_end);
            }
            if s_domain.v_start.is_finite() {
                uv.v = uv.v.clamp(s_domain.v_start, s_domain.v_end);
            }

            if delta.norm() < 1e-14 {
                break;
            }
        } else {
            break;
        }
    }

    let cp = curve.point_at(CurveParam(t));
    let sp = surface.point_at(uv);
    if (cp - sp).norm() < tolerance {
        Some(CurveSurfaceHit {
            point: cp,
            curve_param: CurveParam(t),
            surface_param: uv,
        })
    } else {
        None
    }
}
