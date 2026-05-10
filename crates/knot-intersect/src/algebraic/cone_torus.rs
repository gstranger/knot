//! Cone-torus intersection via parametric substitution.
//!
//! Same framework as cylinder-torus: substitute the cone's rational
//! parameterization into the torus's implicit equation.
//!
//! Cone parameterization:
//!   P(θ, v) = apex + v·axis + v·tan(ha)·(cosθ·ref + sinθ·binorm)
//!
//! After Weierstrass s = tan(θ/2), each coordinate is rational in s
//! with denominator (1+s²), producing F(s, v) of degree 8 in s, 4 in v.

use malachite_q::Rational;
use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::surface::{Cone, Torus, SurfaceParam};
use super::poly::BiPoly;
use super::cylinder_torus::{self, LocalFrame};
use crate::SurfaceSurfaceTrace;
use knot_core::KResult;

/// Build F(s, v) for cone-torus intersection.
pub fn build_cone_torus_poly(cone: &Cone, torus: &Torus) -> BiPoly {
    let frame = cylinder_torus::torus_local_frame(torus);

    let apex = frame.transform_point(&cone.apex);
    let ca = frame.transform_vector(&cone.axis);
    let cu = frame.transform_vector(&cone.ref_direction);
    let cw = ca.cross(&cu);

    let tan_ha = rat(cone.half_angle.tan());
    let big_r = rat(torus.major_radius);
    let little_r = rat(torus.minor_radius);

    let one = BiPoly::from_f64(1.0);
    let s = BiPoly::x();
    let s2 = &s * &s;
    let denom = &one + &s2;
    let cos_num = &one - &s2;
    let sin_num = s.scale(&Rational::from(2));
    let v = BiPoly::y();

    // Cone point = apex + v·axis + v·tan(ha)·(cosθ·ref + sinθ·binorm)
    // Multiply through by denom:
    // x_num = denom·apex.x + v·denom·ca.x + v·tan_ha·(cos_num·cu.x + sin_num·cw.x)
    let build_coord = |orig: f64, ax: f64, ux: f64, wx: f64| -> BiPoly {
        let origin_term = denom.scale(&rat(orig));
        let axis_term = denom.mul(&v).scale(&rat(ax));
        let u_term = cos_num.scale(&(&tan_ha * &rat(ux)));
        let w_term = sin_num.scale(&(&tan_ha * &rat(wx)));
        let radial = &(&u_term + &w_term) * &v;
        &(&origin_term + &axis_term) + &radial
    };

    let x_num = build_coord(apex.x, ca.x, cu.x, cw.x);
    let y_num = build_coord(apex.y, ca.y, cu.y, cw.y);
    let z_num = build_coord(apex.z, ca.z, cu.z, cw.z);

    // Torus implicit: (x²+y²+z²+R²-r²)² - 4R²(x²+y²) = 0
    // Multiplied by denom⁴
    let x2 = &x_num * &x_num;
    let y2 = &y_num * &y_num;
    let z2 = &z_num * &z_num;
    let d2 = &denom * &denom;

    let sum_sq = &(&x2 + &y2) + &z2;
    let r_diff = &big_r * &big_r - &little_r * &little_r;
    let inner = &sum_sq + &BiPoly::constant(r_diff).mul(&d2);
    let inner_sq = &inner * &inner;

    let four_r2 = BiPoly::constant(&big_r * &big_r * Rational::from(4));
    let t_d2 = &(&x2 + &y2) * &d2;

    &inner_sq - &(&four_r2 * &t_d2)
}

/// Full pipeline using discriminant-based topology with implicit
/// validation as a safety gate.
///
/// 1. Build F(s,v) via parametric substitution
/// 2. Compute discriminant Δ(s) to find critical s-values
/// 3. Trace branches in stable intervals between critical points
/// 4. Validate every output point lies on both surfaces; drop points
///    or whole traces that fail validation. This makes the path
///    self-policing: a regression manifests as empty output, which
///    routes the dispatcher to the marcher fallback rather than
///    propagating bad geometry into the boolean pipeline.
pub fn intersect_cone_torus(
    cone: &Cone,
    torus: &Torus,
    tolerance: f64,
) -> KResult<Vec<SurfaceSurfaceTrace>> {
    let f = build_cone_torus_poly(cone, torus);
    let v_coeffs = f.collect_y();

    let s_range = 20.0;
    let branches = super::discriminant::trace_branches_with_topology(
        &v_coeffs, s_range, cone.v_min, cone.v_max, tolerance,
    );
    let frame = cylinder_torus::torus_local_frame(torus);

    // Validation tolerance scales with the geometry: a small CAD model
    // vs a 1e6-mm engineering model need different absolute thresholds.
    // We use a fraction of the dominant length scale plus a fixed
    // floor so unit-scale tests still pass.
    let scale = (cone.v_max.abs() + cone.v_min.abs()).max(torus.major_radius.abs())
        .max(torus.minor_radius.abs())
        .max(1.0);
    let validate_tol = (tolerance * 1e3).max(scale * 1e-5);

    let mut traces = Vec::new();
    for branch in &branches {
        if branch.len() < 2 {
            continue;
        }
        let raw = build_trace_from_branch(branch, cone, torus, &frame);
        if let Some(trace) = filter_and_close(raw, cone, torus, validate_tol, tolerance) {
            traces.push(trace);
        }
    }

    Ok(traces)
}

/// Materialize a (s,v) branch into a `SurfaceSurfaceTrace`. No filtering
/// here — validation runs in `filter_and_close`.
fn build_trace_from_branch(
    branch: &[(f64, f64)],
    cone: &Cone,
    torus: &Torus,
    frame: &cylinder_torus::LocalFrame,
) -> SurfaceSurfaceTrace {
    let binorm = cone.axis.cross(&cone.ref_direction);
    let tan_ha = cone.half_angle.tan();
    let mut points = Vec::with_capacity(branch.len());
    let mut params_a = Vec::with_capacity(branch.len());
    let mut params_b = Vec::with_capacity(branch.len());
    for &(s, v) in branch {
        let theta = 2.0 * s.atan();
        let r = v * tan_ha;
        let pt = cone.apex
            + cone.axis * v
            + cone.ref_direction * (r * theta.cos())
            + binorm * (r * theta.sin());
        points.push(pt);
        params_a.push(SurfaceParam { u: theta, v });
        let local = frame.transform_point(&pt);
        let u_t = local.y.atan2(local.x).rem_euclid(std::f64::consts::TAU);
        let rho = (local.x * local.x + local.y * local.y).sqrt();
        let v_t = local.z.atan2(rho - torus.major_radius);
        params_b.push(SurfaceParam { u: u_t, v: v_t });
    }
    SurfaceSurfaceTrace { points, params_a, params_b }
}

/// Drop trace points that fail the implicit check (Euclidean distance
/// to either surface > validate_tol). Snap a near-closed loop. Return
/// None if too few points survive — the caller will treat None as
/// "this branch wasn't recoverable" and the empty trace list will
/// route the boolean through the marcher fallback.
fn filter_and_close(
    mut trace: SurfaceSurfaceTrace,
    cone: &Cone,
    torus: &Torus,
    validate_tol: f64,
    tolerance: f64,
) -> Option<SurfaceSurfaceTrace> {
    let mut keep = vec![true; trace.points.len()];
    for i in 0..trace.points.len() {
        let p = &trace.points[i];
        if cone_implicit_distance(cone, p) > validate_tol
            || torus_implicit_distance(torus, p) > validate_tol
        {
            keep[i] = false;
        }
    }

    let surviving: usize = keep.iter().filter(|k| **k).count();
    if surviving < 3 {
        return None;
    }
    // If a single contiguous run survived, keep it. If validation
    // punched holes, fragment so each contiguous run becomes its own
    // sub-trace candidate — but for production the safer choice is to
    // refuse the whole trace (don't ship a non-manifold polyline). We
    // accept only traces where validation didn't fragment.
    let mut prev_kept = false;
    let mut transitions = 0;
    for k in &keep {
        if *k != prev_kept {
            transitions += 1;
            prev_kept = *k;
        }
    }
    // transitions = 1 means [F..F][T..T] (one transition into the kept
    // run), 2 means [F..F][T..T][F..F] (in-and-out once), 3 or more
    // means the validation produced multiple disjoint kept runs.
    if transitions > 2 {
        return None;
    }

    let pts: Vec<_> = trace.points.iter().cloned().zip(keep.iter()).filter(|(_, k)| **k).map(|(p, _)| p).collect();
    let pa: Vec<_> = trace.params_a.iter().cloned().zip(keep.iter()).filter(|(_, k)| **k).map(|(p, _)| p).collect();
    let pb: Vec<_> = trace.params_b.iter().cloned().zip(keep.iter()).filter(|(_, k)| **k).map(|(p, _)| p).collect();
    trace.points = pts;
    trace.params_a = pa;
    trace.params_b = pb;

    if trace.points.len() >= 3 {
        let gap = (trace.points[0] - *trace.points.last().unwrap()).norm();
        if gap < tolerance * 100.0 {
            *trace.points.last_mut().unwrap() = trace.points[0];
            *trace.params_a.last_mut().unwrap() = trace.params_a[0];
            *trace.params_b.last_mut().unwrap() = trace.params_b[0];
        }
    }
    Some(trace)
}

/// Euclidean distance from `p` to the (double-)cone surface. Uses the
/// radial-vs-axial decomposition: a point at axial distance v should
/// be at radial distance |v|·tan(α) from the axis. The residual is the
/// gap between actual and expected radial distance.
fn cone_implicit_distance(cone: &Cone, p: &Point3) -> f64 {
    let d = *p - cone.apex;
    let v_along = d.dot(&cone.axis);
    let radial = (d - cone.axis * v_along).norm();
    let expected = v_along.abs() * cone.half_angle.tan();
    (radial - expected).abs()
}

/// Euclidean distance from `p` to the torus surface. The point's
/// distance to the central circle of radius R, then offset by minor
/// radius r — direct geometric formula.
fn torus_implicit_distance(torus: &Torus, p: &Point3) -> f64 {
    let d = *p - torus.center;
    let axial = d.dot(&torus.axis);
    let radial = (d - torus.axis * axial).norm();
    let dist_to_central_circle =
        ((radial - torus.major_radius).powi(2) + axial.powi(2)).sqrt();
    (dist_to_central_circle - torus.minor_radius).abs()
}

fn eval_quartic_at_s(v_coeffs: &[(u32, BiPoly)], s: f64) -> [f64; 5] {
    let mut result = [0.0f64; 5];
    for &(deg, ref poly) in v_coeffs {
        if (deg as usize) < 5 {
            result[deg as usize] = poly.eval_f64(s, 0.0);
        }
    }
    result
}

fn rat(v: f64) -> Rational {
    Rational::try_from(v).unwrap_or(Rational::from(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Oracle: verify F(s,v)=0 at a point known to be on both surfaces.
    /// Compute a cone point, check it satisfies the torus implicit, then
    /// check F vanishes there.
    #[test]
    fn oracle_exact_zero() {
        // Simple coaxial config: cone apex at origin, axis=z, half_angle=45°
        // Torus at z=0, R=3, r=1
        let cone = Cone {
            apex: Point3::origin(),
            axis: Vector3::z(),
            half_angle: std::f64::consts::FRAC_PI_4,
            ref_direction: Vector3::x(),
            v_min: 0.0, v_max: 10.0,
        };
        let torus = Torus {
            center: Point3::origin(),
            axis: Vector3::z(),
            major_radius: 3.0,
            minor_radius: 1.0,
            ref_direction: Vector3::x(),
        };

        let f = build_cone_torus_poly(&cone, &torus);

        // Cone at (θ=0, v): point = (0,0,0) + v*(0,0,1) + v*tan(45°)*(1,0,0) = (v, 0, v)
        // So x=v, y=0, z=v. On the torus: (v²+0+v²+9-1)² - 4·9·v² = 0
        // (2v²+8)² - 36v² = 0
        // 4v⁴ + 32v² + 64 - 36v² = 0
        // 4v⁴ - 4v² + 64 = 0
        // v⁴ - v² + 16 = 0 — discriminant = 1 - 64 < 0, no real roots.
        // So this config has no intersection at θ=0. Let me try a different config.

        // Better: cone apex at origin, torus center at (0,0,3), R=2, r=1
        let torus2 = Torus {
            center: Point3::new(0.0, 0.0, 3.0),
            axis: Vector3::z(),
            major_radius: 2.0,
            minor_radius: 1.0,
            ref_direction: Vector3::x(),
        };
        let f2 = build_cone_torus_poly(&cone, &torus2);

        // At θ=0, cone point is (v, 0, v) in world coords.
        // In torus-local frame (center at (0,0,3)): (v, 0, v-3)
        // Torus implicit: (v² + (v-3)² + 4 - 1)² - 4·4·v² = 0
        // (v² + v²-6v+9 + 3)² - 16v² = 0
        // (2v²-6v+12)² - 16v² = 0
        // Let's just evaluate numerically at a few v values to find a root
        for v_test in [1.0f64, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0] {
            let x = v_test;
            let z = v_test - 3.0;
            let rho2 = x * x;
            let sum = rho2 + z * z + 4.0 - 1.0;
            let torus_val = sum * sum - 4.0 * 4.0 * rho2;
            if torus_val.abs() < 5.0 {
                eprintln!("Near intersection: v={}, torus_val={:.4}", v_test, torus_val);
                // Check F at this point: s=0 (θ=0)
                let f_val = f2.eval_f64(0.0, v_test);
                eprintln!("  F(0, {}) = {:.6}", v_test, f_val);
            }
        }

        // Now sample many (s, v) pairs on the actual cone surface and check
        // which ones are close to the torus, then verify F vanishes there
        let mut found_intersection = false;
        for theta_deg in (0..360).step_by(15) {
            let theta = theta_deg as f64 * std::f64::consts::PI / 180.0;
            let s = (theta / 2.0).tan();
            if s.abs() > 100.0 { continue; }

            for v_i in 0..40 {
                let v = 0.5 + v_i as f64 * 0.1;
                let r = v * cone.half_angle.tan();
                let pt = Point3::new(r * theta.cos(), r * theta.sin(), v);
                // Check torus (centered at (0,0,3))
                let lx = pt.x;
                let ly = pt.y;
                let lz = pt.z - 3.0;
                let rho2 = lx * lx + ly * ly;
                let sum = rho2 + lz * lz + 4.0 - 1.0;
                let torus_val = sum * sum - 16.0 * rho2;

                if torus_val.abs() < 0.5 {
                    let f_val = f2.eval_f64(s, v);
                    eprintln!("  θ={}° v={:.1} torus_val={:.4} F(s,v)={:.6}",
                        theta_deg, v, torus_val, f_val);
                    if torus_val.abs() < 0.1 {
                        assert!(f_val.abs() < 1.0,
                            "F should be near zero when torus_val is near zero: F={}", f_val);
                        found_intersection = true;
                    }
                }
            }
        }
        assert!(found_intersection, "should find at least one near-intersection point");
    }

    #[test]
    fn coaxial_cone_torus_polynomial() {
        let cone = Cone {
            apex: Point3::origin(),
            axis: Vector3::z(),
            half_angle: std::f64::consts::FRAC_PI_4,
            ref_direction: Vector3::x(),
            v_min: 0.0, v_max: 5.0,
        };
        let torus = Torus {
            center: Point3::new(0.0, 0.0, 2.0),
            axis: Vector3::z(),
            major_radius: 3.0,
            minor_radius: 1.0,
            ref_direction: Vector3::x(),
        };

        let f = build_cone_torus_poly(&cone, &torus);
        assert!(!f.is_zero());
        assert_eq!(f.degree_y(), 4, "should be quartic in v");
        eprintln!("Cone-torus F: degree s={}, v={}, terms={}",
            f.degree_x(), f.degree_y(), f.num_terms());
    }

    #[test]
    fn coaxial_cone_torus_pipeline() {
        // Use the same config as oracle test: apex at origin, torus at z=3
        let cone = Cone {
            apex: Point3::origin(),
            axis: Vector3::z(),
            half_angle: std::f64::consts::FRAC_PI_4,
            ref_direction: Vector3::x(),
            v_min: 0.5, v_max: 5.0,
        };
        let torus = Torus {
            center: Point3::new(0.0, 0.0, 3.0),
            axis: Vector3::z(),
            major_radius: 2.0,
            minor_radius: 1.0,
            ref_direction: Vector3::x(),
        };

        // Check how many roots Ferrari finds at s=0
        let f = build_cone_torus_poly(&cone, &torus);
        let v_coeffs = f.collect_y();
        let qc = eval_quartic_at_s(&v_coeffs, 0.0);
        eprintln!("Quartic coeffs at s=0: {:?}", qc);
        // Also check: F(0, 2) and F(0, 3) should be zero
        eprintln!("F(0, 2) = {}", f.eval_f64(0.0, 2.0));
        eprintln!("F(0, 3) = {}", f.eval_f64(0.0, 3.0));
        // Reconstruct quartic from coeffs and evaluate at v=2, v=3
        let q_at_2 = qc[0] + qc[1]*2.0 + qc[2]*4.0 + qc[3]*8.0 + qc[4]*16.0;
        let q_at_3 = qc[0] + qc[1]*3.0 + qc[2]*9.0 + qc[3]*27.0 + qc[4]*81.0;
        eprintln!("Q(2) = {}, Q(3) = {}", q_at_2, q_at_3);
        let roots = super::super::quartic::solve_quartic(&qc);
        eprintln!("Roots at s=0: {:?}", roots);
        let filtered: Vec<f64> = roots.iter().copied()
            .filter(|v| *v >= 0.5 && *v <= 5.0)
            .collect();
        eprintln!("Filtered roots at s=0: {:?}", filtered);

        let traces = intersect_cone_torus(&cone, &torus, 1e-6).unwrap();
        eprintln!("Cone-torus traces: {}", traces.len());
        for (i, t) in traces.iter().enumerate() {
            eprintln!("  Trace {}: {} points", i, t.points.len());
            if !t.points.is_empty() {
                eprintln!("    z range: {:.4} to {:.4}",
                    t.points.iter().map(|p| p.z).fold(f64::MAX, f64::min),
                    t.points.iter().map(|p| p.z).fold(f64::MIN, f64::max));
            }
        }
        // Should find 2 branches (v=2 and v=3 circles)
        assert!(traces.len() >= 2, "expected ≥2 traces for 2 intersection circles, got {}", traces.len());
    }
}
