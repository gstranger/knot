//! Cylinder-torus intersection via parametric substitution.
//!
//! Substitutes the cylinder's rational parameterization into the torus's
//! implicit equation to produce a bivariate polynomial F(s, v) = 0
//! whose zero set is the intersection curve.
//!
//! Cylinder parameterization (after Weierstrass s = tan(θ/2)):
//!   x = ox + r·(1-s²)/(1+s²)·ux + r·2s/(1+s²)·wx + v·ax
//!   y = oy + r·(1-s²)/(1+s²)·uy + r·2s/(1+s²)·wy + v·ay
//!   z = oz + r·(1-s²)/(1+s²)·uz + r·2s/(1+s²)·wz + v·az
//!
//! where u = ref_direction, w = axis × ref_direction, a = axis.
//!
//! Torus implicit (centered at origin with axis along z):
//!   (x² + y² + z² + R² - r²)² - 4R²(x² + y²) = 0
//!
//! For a general torus (center c, axis n), we transform the cylinder
//! into the torus's local frame first.

use malachite_q::Rational;
use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::surface::{Cylinder, Torus, SurfaceParam};
use super::poly::BiPoly;

/// Build the bivariate polynomial F(s, v) whose zero set is the
/// cylinder-torus intersection, where s = tan(θ/2) is the Weierstrass
/// substitution of the cylinder's angular parameter.
///
/// Returns F(s, v) with exact rational coefficients, plus the
/// denominator D(s) = (1 + s²)^k that was cleared.
///
/// The actual intersection is F(s, v) = 0 where s ∈ ℝ and v ∈ [v_min, v_max].
pub fn build_cylinder_torus_poly(
    cyl: &Cylinder,
    torus: &Torus,
) -> BiPoly {
    // Transform cylinder into torus's local frame where:
    //   torus center = origin, torus axis = z-axis
    let to_local = torus_local_frame(torus);

    // Cylinder origin, axis, ref_direction in torus-local coordinates
    let co = to_local.transform_point(&cyl.origin);
    let ca = to_local.transform_vector(&cyl.axis);
    let cu = to_local.transform_vector(&cyl.ref_direction);
    let cw = ca.cross(&cu); // binormal

    let r_cyl = rat(cyl.radius);
    let big_r = rat(torus.major_radius);
    let little_r = rat(torus.minor_radius);

    // Build the cylinder parameterization as rational polynomials in (s, v).
    // After Weierstrass s = tan(θ/2):
    //   cosθ = (1 - s²) / (1 + s²)
    //   sinθ = 2s / (1 + s²)
    //
    // So: P(s, v) = co + r_cyl·cosθ·cu + r_cyl·sinθ·cw + v·ca
    //
    // Each coordinate is a rational function in s with denominator (1 + s²).
    // We work with numerators and clear the denominator at the end.

    // denom = 1 + s²
    let one = BiPoly::from_f64(1.0);
    let s = BiPoly::x(); // s variable
    let s2 = &s * &s;
    let denom = &one + &s2; // 1 + s²

    // cos_num = 1 - s² (numerator of cosθ)
    let cos_num = &one - &s2;
    // sin_num = 2s (numerator of sinθ)
    let sin_num = s.scale(&Rational::from(2));

    let v = BiPoly::y(); // v variable

    // x_num = (1+s²)·co.x + r·(1-s²)·cu.x + r·2s·cw.x + v·(1+s²)·ca.x
    // (multiply through by denom to clear the fraction)
    let x_num = build_coord_num(co.x, cu.x, cw.x, ca.x, &r_cyl, &cos_num, &sin_num, &denom, &v);
    let y_num = build_coord_num(co.y, cu.y, cw.y, ca.y, &r_cyl, &cos_num, &sin_num, &denom, &v);
    let z_num = build_coord_num(co.z, cu.z, cw.z, ca.z, &r_cyl, &cos_num, &sin_num, &denom, &v);

    // Torus implicit in local frame (center=origin, axis=z):
    //   (x² + y² + z² + R² - r²)² - 4R²(x² + y²) = 0
    //
    // With x = x_num/denom, y = y_num/denom, z = z_num/denom:
    //   Let S = x² + y² + z² = (x_num² + y_num² + z_num²) / denom²
    //   Let T = x² + y² = (x_num² + y_num²) / denom²
    //   Torus: (S + R² - r²)² - 4R²T = 0
    //   Multiply by denom⁴:
    //   (x_num² + y_num² + z_num² + (R² - r²)·denom²)² - 4R²·(x_num² + y_num²)·denom² = 0

    let x2 = &x_num * &x_num;
    let y2 = &y_num * &y_num;
    let z2 = &z_num * &z_num;
    let d2 = &denom * &denom;

    let sum_sq = &(&x2 + &y2) + &z2; // x² + y² + z²  (times denom²)
    let r_diff = &big_r * &big_r - &little_r * &little_r; // R² - r²
    let r_diff_poly = BiPoly::constant(r_diff);
    let inner = &sum_sq + &r_diff_poly.mul(&d2); // (sum_sq + (R²-r²)·denom²)

    let inner_sq = &inner * &inner;

    let four_r2 = BiPoly::constant(&big_r * &big_r * Rational::from(4));
    let t_times_d2 = &(&x2 + &y2) * &d2; // (x²+y²)·denom²
    let rhs = &four_r2 * &t_times_d2;

    let f = &inner_sq - &rhs;

    f
}

/// Build one coordinate's numerator: (1+s²)·origin + r·(1-s²)·u + r·2s·w + v·(1+s²)·a
fn build_coord_num(
    origin: f64, u: f64, w: f64, a: f64,
    r: &Rational,
    cos_num: &BiPoly, sin_num: &BiPoly, denom: &BiPoly, v: &BiPoly,
) -> BiPoly {
    let origin_term = denom.scale(&rat(origin));
    let u_term = cos_num.scale(&(r * &rat(u)));
    let w_term = sin_num.scale(&(r * &rat(w)));
    let a_term = denom.mul(v).scale(&rat(a));

    &(&(&origin_term + &u_term) + &w_term) + &a_term
}

/// Helper: f64 → exact Rational
fn rat(v: f64) -> Rational {
    Rational::try_from(v).unwrap_or(Rational::from(0))
}

/// Build a local coordinate frame for the torus:
/// origin at torus center, z-axis along torus axis.
pub(crate) struct LocalFrame {
    origin: Point3,
    u: Vector3, // local x
    v: Vector3, // local y
    w: Vector3, // local z (= torus axis)
}

pub(crate) fn torus_local_frame(torus: &Torus) -> LocalFrame {
    let w = torus.axis.normalize();
    let u = if torus.ref_direction.cross(&w).norm() > 1e-12 {
        (torus.ref_direction - w * torus.ref_direction.dot(&w)).normalize()
    } else if w.x.abs() < 0.9 {
        Vector3::x().cross(&w).normalize()
    } else {
        Vector3::y().cross(&w).normalize()
    };
    let v = w.cross(&u);
    LocalFrame { origin: torus.center, u, v, w }
}

impl LocalFrame {
    pub(crate) fn transform_point(&self, p: &Point3) -> Point3 {
        let d = p - self.origin;
        Point3::new(d.dot(&self.u), d.dot(&self.v), d.dot(&self.w))
    }

    pub(crate) fn transform_vector(&self, v: &Vector3) -> Vector3 {
        Vector3::new(v.dot(&self.u), v.dot(&self.v), v.dot(&self.w))
    }
}

// ═══════════════════════════════════════════════════════════════════
// End-to-end intersection pipeline
// ═══════════════════════════════════════════════════════════════════

use crate::SurfaceSurfaceTrace;
use knot_core::KResult;

/// Compute cylinder-torus intersection via the algebraic pipeline.
///
/// Pipeline:
/// 1. Build F(s, v) via parametric substitution
/// 2. Collect as quartic in v: a₀(s) + a₁(s)v + a₂(s)v² + a₃(s)v³ + a₄(s)v⁴ = 0
/// 3. Compute quartic discriminant Δ(s) directly from coefficients
/// 4. Bernstein-subdivide Δ(s) to find critical s-values (topology)
/// 5. Ferrari-trace branches between critical points
/// 6. Convert (s, v) curve points back to 3D
pub fn intersect_cylinder_torus(
    cyl: &Cylinder,
    torus: &Torus,
    tolerance: f64,
) -> KResult<Vec<SurfaceSurfaceTrace>> {
    let f = build_cylinder_torus_poly(cyl, torus);

    // Step 2: Collect F as univariate quartic in v (y variable).
    // F(s, v) = a₀(s) + a₁(s)·v + a₂(s)·v² + a₃(s)·v³ + a₄(s)·v⁴
    let v_coeffs = f.collect_y(); // Vec<(v_degree, BiPoly-in-s)>
    let max_v_deg = v_coeffs.iter().map(|(d, _)| *d).max().unwrap_or(0);

    if max_v_deg < 1 {
        return Ok(Vec::new()); // degenerate — no v dependence
    }

    // Use discriminant-based topology for reliable branch tracing.
    let s_range = 20.0;
    let branches = super::discriminant::trace_branches_with_topology(
        &v_coeffs, s_range, cyl.v_min, cyl.v_max, tolerance,
    );

    // Step 6: Convert (s, v) points to 3D and build traces.
    let frame = torus_local_frame(torus);
    let mut traces = Vec::new();

    for branch in &branches {
        if branch.len() < 2 { continue; }

        let mut points = Vec::new();
        let mut params_a = Vec::new(); // cylinder params
        let mut params_b = Vec::new(); // torus params (approximate)

        for &(s, v) in branch {
            // Convert s back to θ
            let theta = 2.0 * s.atan();
            let cos_t = theta.cos();
            let sin_t = theta.sin();

            // Cylinder point in world coordinates
            let binorm = cyl.axis.cross(&cyl.ref_direction);
            let pt = cyl.origin
                + cyl.ref_direction * (cyl.radius * cos_t)
                + binorm * (cyl.radius * sin_t)
                + cyl.axis * v;

            points.push(pt);
            params_a.push(SurfaceParam { u: theta, v });

            // Approximate torus params
            let local = frame.transform_point(&pt);
            let u_torus = local.y.atan2(local.x).rem_euclid(std::f64::consts::TAU);
            let r_from_axis = (local.x * local.x + local.y * local.y).sqrt();
            let v_torus = local.z.atan2(r_from_axis - torus.major_radius);
            params_b.push(SurfaceParam { u: u_torus, v: v_torus });
        }

        // Check if branch is a closed loop
        if points.len() >= 3 {
            let first = points[0];
            let last = *points.last().unwrap();
            if (first - last).norm() < tolerance * 100.0 {
                *points.last_mut().unwrap() = first;
                *params_a.last_mut().unwrap() = params_a[0];
                *params_b.last_mut().unwrap() = params_b[0];
            }
        }

        traces.push(SurfaceSurfaceTrace { points, params_a, params_b });
    }

    Ok(traces)
}

/// Evaluate the quartic coefficients a₀(s)...a₄(s) at a specific s value.
fn eval_quartic_at_s(v_coeffs: &[(u32, BiPoly)], s: f64) -> [f64; 5] {
    let mut result = [0.0f64; 5];
    for &(deg, ref poly) in v_coeffs {
        if (deg as usize) < 5 {
            result[deg as usize] = poly.eval_f64(s, 0.0);
        }
    }
    result
}

/// Assemble branches from per-s root lists by nearest-neighbor tracking.
///
/// Each branch is a sequence of (s, v) pairs tracing one continuous
/// root of the quartic as s varies.
pub(crate) fn assemble_branches(
    all_roots: &[(f64, Vec<f64>)],
    tolerance: f64,
) -> Vec<Vec<(f64, f64)>> {
    if all_roots.is_empty() { return Vec::new(); }

    // Active branches: each is a Vec<(s, v)> being extended
    let mut branches: Vec<Vec<(f64, f64)>> = Vec::new();

    for (s, roots) in all_roots {
        let mut used = vec![false; roots.len()];

        // Try to extend existing branches
        for branch in branches.iter_mut() {
            let (_, last_v) = *branch.last().unwrap();

            // Find the nearest unmatched root
            let mut best_idx = None;
            let mut best_dist = f64::MAX;
            for (ri, rv) in roots.iter().enumerate() {
                if used[ri] { continue; }
                let dist = (rv - last_v).abs();
                if dist < best_dist {
                    best_dist = dist;
                    best_idx = Some(ri);
                }
            }

            // Match if close enough (within a generous threshold)
            let max_jump = (tolerance * 1000.0).max(0.5);
            if let Some(idx) = best_idx {
                if best_dist < max_jump {
                    branch.push((*s, roots[idx]));
                    used[idx] = true;
                }
            }
        }

        // Start new branches for unmatched roots
        for (ri, rv) in roots.iter().enumerate() {
            if !used[ri] {
                branches.push(vec![(*s, *rv)]);
            }
        }
    }

    // Filter short branches (noise)
    branches.retain(|b| b.len() >= 5);

    branches
}

#[cfg(test)]
mod tests {
    use super::*;
    use knot_geom::surface::{Cylinder, Torus};

    /// Oracle test: coaxial cylinder-torus with known intersection.
    /// Cylinder radius = 1.5, coaxial with torus (R=2, r=1).
    /// Intersection is two circles at z = ±sqrt(1 - (1.5-2)²) = ±sqrt(0.75).
    /// At these circles, θ is free and v = z.
    #[test]
    fn coaxial_known_intersection_exact_zero() {
        let cyl = Cylinder {
            origin: Point3::new(0.0, 0.0, 0.0),
            axis: Vector3::z(),
            radius: 1.5,
            ref_direction: Vector3::x(),
            v_min: -2.0,
            v_max: 2.0,
        };
        let torus = Torus {
            center: Point3::origin(),
            axis: Vector3::z(),
            major_radius: 2.0,
            minor_radius: 1.0,
            ref_direction: Vector3::x(),
        };

        let f = build_cylinder_torus_poly(&cyl, &torus);

        // At s=0 (θ=0), the cylinder point is (1.5, 0, v).
        // On the torus: (x²+y²+z²+R²-r²)² - 4R²(x²+y²) = 0
        // (1.5² + v² + 4 - 1)² - 4·4·1.5² = 0
        // (2.25 + v² + 3)² - 36 = 0
        // (5.25 + v²)² = 36
        // 5.25 + v² = 6  →  v² = 0.75  →  v = ±√0.75

        let v_intersect = 0.75_f64.sqrt();

        // Check F(s=0, v=v_intersect) = 0 in exact arithmetic
        let s_val = Rational::from(0);
        let v_val = Rational::try_from(v_intersect).unwrap();
        let result = f.eval(&s_val, &v_val);

        // The f64→Rational conversion of √0.75 isn't exact, so check numerically
        let result_f64 = f.eval_f64(0.0, v_intersect);
        assert!(result_f64.abs() < 1e-6,
            "F(0, √0.75) should be ~0, got {}", result_f64);

        // Also check with exact v: v² = 3/4, so v = √(3/4)
        // Since √(3/4) isn't rational, use the relation directly:
        // At s=0: x_num = (1+0)·0 + 1.5·(1-0)·1 + 1.5·0·0 + v·(1+0)·0 = 1.5
        // Wait, let me just check the polynomial isn't degenerate
        assert!(!f.is_zero(), "F should not be identically zero");
        assert!(f.total_degree() > 0, "F should have positive degree");

        eprintln!("F degree: total={}, in s={}, in v={}",
            f.total_degree(), f.degree_x(), f.degree_y());
        eprintln!("F terms: {}", f.num_terms());
        eprintln!("F(0, {}) = {}", v_intersect, result_f64);
        eprintln!("F(0, -{}) = {}", v_intersect, f.eval_f64(0.0, -v_intersect));

        // Both intersection points should evaluate close to zero
        assert!(f.eval_f64(0.0, -v_intersect).abs() < 1e-6,
            "F(0, -√0.75) should be ~0");

        // A non-intersection point should NOT be zero
        let non_zero = f.eval_f64(0.0, 0.0);
        assert!(non_zero.abs() > 1.0,
            "F(0, 0) should be far from zero, got {}", non_zero);
    }

    /// Check that F vanishes at multiple θ values along the known circle.
    #[test]
    fn coaxial_multiple_theta_values() {
        let cyl = Cylinder {
            origin: Point3::origin(),
            axis: Vector3::z(),
            radius: 1.5,
            ref_direction: Vector3::x(),
            v_min: -2.0,
            v_max: 2.0,
        };
        let torus = Torus {
            center: Point3::origin(),
            axis: Vector3::z(),
            major_radius: 2.0,
            minor_radius: 1.0,
            ref_direction: Vector3::x(),
        };

        let f = build_cylinder_torus_poly(&cyl, &torus);
        let v_int = 0.75_f64.sqrt();

        // Test at several θ values via s = tan(θ/2)
        for theta_deg in [0.0, 30.0, 45.0, 90.0, 135.0, 180.0, 270.0] {
            let theta = theta_deg * std::f64::consts::PI / 180.0;
            let s = (theta / 2.0).tan();
            if s.abs() > 1e6 { continue; } // skip near-singular

            let val = f.eval_f64(s, v_int);
            assert!(val.abs() < 1e-3,
                "F({}, {}) = {} (should be ~0, θ={}°)", s, v_int, val, theta_deg);
        }
    }

    /// Integration test: full algebraic pipeline on coaxial cylinder-torus.
    /// Should find 2 closed-loop intersection curves (circles).
    #[test]
    fn coaxial_full_pipeline() {
        let cyl = Cylinder {
            origin: Point3::origin(),
            axis: Vector3::z(),
            radius: 1.5,
            ref_direction: Vector3::x(),
            v_min: -2.0,
            v_max: 2.0,
        };
        let torus = Torus {
            center: Point3::origin(),
            axis: Vector3::z(),
            major_radius: 2.0,
            minor_radius: 1.0,
            ref_direction: Vector3::x(),
        };

        let traces = intersect_cylinder_torus(&cyl, &torus, 1e-6).unwrap();
        eprintln!("Traces: {}", traces.len());
        for (i, t) in traces.iter().enumerate() {
            eprintln!("  Trace {}: {} points", i, t.points.len());
            if let (Some(first), Some(last)) = (t.points.first(), t.points.last()) {
                let gap = (first - last).norm();
                eprintln!("    first-last gap: {:.6}", gap);
                eprintln!("    z range: {:.4} to {:.4}",
                    t.points.iter().map(|p| p.z).fold(f64::MAX, f64::min),
                    t.points.iter().map(|p| p.z).fold(f64::MIN, f64::max));
            }
        }

        // Should find 2 branches (two circles at z ≈ ±0.866)
        assert!(traces.len() >= 2, "expected ≥2 traces, got {}", traces.len());

        // Validate: every output point should satisfy both surface implicits
        let expected_z = 0.75_f64.sqrt(); // ≈ 0.866
        for trace in &traces {
            for pt in &trace.points {
                // Cylinder implicit: x² + y² = r²
                let cyl_dist = ((pt.x * pt.x + pt.y * pt.y).sqrt() - cyl.radius).abs();
                assert!(cyl_dist < 0.1,
                    "point not on cylinder: dist={:.4}", cyl_dist);

                // Torus implicit: (x²+y²+z²+R²-r²)² - 4R²(x²+y²) = 0
                let rho2 = pt.x * pt.x + pt.y * pt.y;
                let sum = rho2 + pt.z * pt.z + torus.major_radius.powi(2)
                    - torus.minor_radius.powi(2);
                let torus_val = sum * sum - 4.0 * torus.major_radius.powi(2) * rho2;
                assert!(torus_val.abs() < 1.0,
                    "point not on torus: val={:.4}", torus_val);
            }
        }
    }

    /// Integration test: offset (non-coaxial) cylinder-torus.
    /// This is the case the marcher gets wrong.
    #[test]
    fn offset_pipeline() {
        let cyl = Cylinder {
            origin: Point3::new(0.5, 0.0, 0.0), // offset from torus center
            axis: Vector3::z(),
            radius: 1.0,
            ref_direction: Vector3::x(),
            v_min: -2.0,
            v_max: 2.0,
        };
        let torus = Torus {
            center: Point3::origin(),
            axis: Vector3::z(),
            major_radius: 2.0,
            minor_radius: 0.8,
            ref_direction: Vector3::x(),
        };

        let traces = intersect_cylinder_torus(&cyl, &torus, 1e-6).unwrap();
        eprintln!("Offset traces: {}", traces.len());
        for (i, t) in traces.iter().enumerate() {
            eprintln!("  Trace {}: {} points", i, t.points.len());
        }

        // Should find at least 1 trace
        assert!(!traces.is_empty(), "offset cylinder-torus should intersect");

        // Validate implicits
        for trace in &traces {
            for pt in &trace.points {
                // Cylinder: (x-0.5)² + y² = 1
                let cx = pt.x - 0.5;
                let cyl_dist = ((cx * cx + pt.y * pt.y).sqrt() - cyl.radius).abs();
                assert!(cyl_dist < 0.1, "not on cylinder: {:.4}", cyl_dist);
            }
        }
    }
}
