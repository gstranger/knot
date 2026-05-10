//! Quartic discriminant and topology determination.
//!
//! Given F(s, v) = a₄(s)v⁴ + a₃(s)v³ + a₂(s)v² + a₁(s)v + a₀(s),
//! computes the discriminant Δ(s) whose roots are the s-values where
//! two v-roots of F coincide (branch merge/split points).
//!
//! The discriminant partitions the s-axis into intervals where:
//! - The number of real v-roots is constant
//! - Root sorting by value is stable (no swaps)
//!
//! Branch tracing within these intervals is reliable; connectivity
//! at critical points comes from the topology graph.

use super::poly::BiPoly;
use malachite_q::Rational;

/// Evaluate the quartic discriminant numerically at a specific s value.
///
/// Given quartic coefficients a₀..a₄ (as f64), computes the discriminant
/// using the standard formula. This is O(1) per evaluation — no polynomial
/// multiplication needed.
/// Discriminant of quartic av⁴ + bv³ + cv² + dv + e.
/// Parameters: (a₀=e, a₁=d, a₂=c, a₃=b, a₄=a) in our power-basis convention.
fn eval_discriminant_f64(a0: f64, a1: f64, a2: f64, a3: f64, a4: f64) -> f64 {
    // Standard formula for Δ(av⁴ + bv³ + cv² + dv + e):
    let (e, d, c, b, a) = (a0, a1, a2, a3, a4);
    let a2v = a*a; let a3v = a2v*a;
    let b2 = b*b; let b3 = b2*b; let b4 = b2*b2;
    let c2 = c*c; let c3 = c2*c; let c4 = c2*c2;
    let d2 = d*d; let d3 = d2*d; let d4 = d2*d2;
    let e2 = e*e; let e3 = e2*e;

    256.0*a3v*e3 - 192.0*a2v*b*d*e2 - 128.0*a2v*c2*e2 + 144.0*a2v*c*d2*e
    - 27.0*a2v*d4 + 144.0*a*b2*c*e2 - 6.0*a*b2*d2*e - 80.0*a*b*c2*d*e
    + 18.0*a*b*c*d3 + 16.0*a*c4*e - 4.0*a*c3*d2 - 27.0*b4*e2
    + 18.0*b3*c*d*e - 4.0*b3*d3 - 4.0*b2*c3*e + b2*c2*d2
}

/// Find critical s-values where the v-root topology of F(s, v) = 0
/// changes — points at which two real roots merge (transversal U-turn)
/// or appear/disappear (creation/annihilation event).
///
/// Detects critical points by **root-count change**: scan s in fixed
/// steps, count real v-roots at each step via the generalized
/// univariate solver, and bisect any interval where the count
/// changes. Generic across polynomial degrees; for the quartic case
/// we additionally use the closed-form quartic discriminant as a
/// secondary signal that catches tangent events where two roots
/// coalesce momentarily without changing the count (rare in practice
/// but real).
pub fn find_critical_s_values(
    v_coeffs: &[(u32, BiPoly)],
    s_range: f64,
) -> Vec<f64> {
    let n = 200;
    let step = 2.0 * s_range / n as f64;
    let mut critical = Vec::new();

    let max_v_deg = v_coeffs
        .iter()
        .map(|(d, _)| *d as usize)
        .max()
        .unwrap_or(0);
    let is_quartic = max_v_deg == 4;

    let root_count = |s: f64| -> usize {
        let coeffs = eval_v_coeffs_at_s(v_coeffs, s);
        super::quartic::solve_univariate(&coeffs).len()
    };

    // Quartic-only secondary signal: closed-form discriminant.
    let eval_disc = |s: f64| -> Option<f64> {
        if !is_quartic {
            return None;
        }
        let qc = eval_quartic_coeffs(v_coeffs, s);
        Some(eval_discriminant_f64(qc[0], qc[1], qc[2], qc[3], qc[4]))
    };

    let mut prev_disc = eval_disc(-s_range);
    let mut prev_nroots = root_count(-s_range);
    let mut prev_s = -s_range;

    for i in 1..=n {
        let s = -s_range + step * i as f64;
        let nr = root_count(s);
        let disc = eval_disc(s);

        // Primary detection: real-root count changes between samples.
        if nr != prev_nroots {
            let mut lo = prev_s;
            let mut hi = s;
            for _ in 0..30 {
                let mid = (lo + hi) * 0.5;
                if (hi - lo) < 1e-8 {
                    break;
                }
                if root_count(mid) == prev_nroots {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let crit = (lo + hi) * 0.5;
            if !critical.iter().any(|c: &f64| (c - crit).abs() < step) {
                critical.push(crit);
            }
        }

        // Secondary (quartic only): discriminant sign change. Catches
        // tangent mergers — two roots coalesce briefly then separate
        // again with the count unchanged. Rare but real; relying on
        // root-count alone misses these.
        if let (Some(pd), Some(d)) = (prev_disc, disc) {
            if pd * d < 0.0 {
                let mut lo = prev_s;
                let mut hi = s;
                let mut vlo = pd;
                for _ in 0..50 {
                    let mid = (lo + hi) * 0.5;
                    if (hi - lo) < 1e-10 {
                        break;
                    }
                    let mv = eval_disc(mid).unwrap_or(0.0);
                    if vlo * mv <= 0.0 {
                        hi = mid;
                    } else {
                        lo = mid;
                        vlo = mv;
                    }
                }
                let crit = (lo + hi) * 0.5;
                if !critical.iter().any(|c: &f64| (c - crit).abs() < step) {
                    critical.push(crit);
                }
            }
        }

        prev_disc = disc;
        prev_nroots = nr;
        prev_s = s;
    }

    critical.sort_by(|a, b| a.partial_cmp(b).unwrap());
    critical.dedup_by(|a, b| (*a - *b).abs() < 1e-8);
    critical
}

/// Evaluate F(s, v) at fixed s, returning the resulting univariate
/// polynomial in v as ascending-power coefficients. Same shape as
/// `branch_topology::eval_v_coeffs_at_s` but kept local so the two
/// modules stay decoupled.
fn eval_v_coeffs_at_s(v_coeffs: &[(u32, BiPoly)], s: f64) -> Vec<f64> {
    let max_deg = v_coeffs
        .iter()
        .map(|(d, _)| *d as usize)
        .max()
        .unwrap_or(0);
    let mut result = vec![0.0f64; max_deg + 1];
    for (deg, poly) in v_coeffs {
        result[*deg as usize] = poly.eval_f64(s, 0.0);
    }
    result
}

/// Trace branches of F(s,v)=0 across all stable intervals between
/// critical s-values, threading connections through the critical
/// points so that U-turns and pass-throughs are stitched into single
/// curves. Delegates to `branch_topology::trace_branches_topology`,
/// which owns the production-ready topology-aware connector.
///
/// Output: one (s,v) polyline per intersection curve, clipped to the
/// caller's [v_min, v_max] window. Closed loops are emitted with
/// matching first/last vertices.
pub fn trace_branches_with_topology(
    v_coeffs: &[(u32, BiPoly)],
    s_range: f64,
    v_min: f64,
    v_max: f64,
    tolerance: f64,
) -> Vec<Vec<(f64, f64)>> {
    super::branch_topology::trace_branches_topology(
        v_coeffs, s_range, v_min, v_max, tolerance,
    )
}

fn eval_quartic_coeffs(v_coeffs: &[(u32, BiPoly)], s: f64) -> [f64; 5] {
    let mut result = [0.0f64; 5];
    for &(deg, ref poly) in v_coeffs {
        if (deg as usize) < 5 {
            result[deg as usize] = poly.eval_f64(s, 0.0);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminant_zero_for_double_root() {
        // (v-1)²(v-3)(v-5): a₀=15, a₁=-38, a₂=32, a₃=-10, a₄=1
        let d = eval_discriminant_f64(15.0, -38.0, 32.0, -10.0, 1.0);
        assert!(d.abs() < 1e-6, "double root → discriminant should be ~0, got {}", d);
    }

    #[test]
    fn discriminant_nonzero_for_distinct_roots() {
        // (v-1)(v-2)(v-3)(v-4): a₀=24, a₁=-50, a₂=35, a₃=-10, a₄=1
        let d = eval_discriminant_f64(24.0, -50.0, 35.0, -10.0, 1.0);
        assert!(d.abs() > 1.0, "distinct roots → discriminant should be nonzero");
    }

    #[test]
    fn critical_values_for_varying_quartic() {
        // F(s,v) = v⁴ - s·v² + 1: at s=2 has double root (discriminant=0)
        let a0 = BiPoly::from_f64(1.0);
        let a1 = BiPoly::zero();
        // a₂ = -s (depends on s)
        let a2 = BiPoly::x().scale(&malachite_q::Rational::from(-1));
        let a3 = BiPoly::zero();
        let a4 = BiPoly::from_f64(1.0);

        let v_coeffs = vec![
            (0u32, a0), (1, a1), (2, a2), (3, a3), (4, a4),
        ];
        let critical = find_critical_s_values(&v_coeffs, 10.0);
        eprintln!("Critical s-values: {:?}", critical);
        // Should find critical points near s=±2 (where v⁴ - 2v² + 1 = (v²-1)² has double root)
        assert!(!critical.is_empty(), "should find critical s-values");
        assert!(critical.iter().any(|s| (s - 2.0).abs() < 0.1),
            "should find critical near s=2");
    }
}
