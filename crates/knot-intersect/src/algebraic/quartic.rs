//! Univariate polynomial root finding.
//!
//! Two entry points:
//!
//! - [`solve_quartic`]: takes `[f64; 5]` (quartic by construction) and
//!   uses Ferrari's resolvent cubic. Falls through to lower-degree
//!   solvers when the leading coefficient vanishes. Hybrid Bernstein
//!   fallback when Ferrari misses roots.
//!
//! - [`solve_univariate`]: takes a `&[f64]` of any length and picks
//!   the right path: Ferrari for degree ≤ 4 (closed-form, fast),
//!   Bernstein subdivision + Newton polish for degree > 4. Used by
//!   the topology connector when intersecting NURBS surfaces, where
//!   the per-`s` polynomial in `t` can have degree 6 (sphere/cylinder/
//!   cone-vs-bicubic-NURBS) or 12 (torus-vs-bicubic-NURBS).

/// Find all real roots of a quartic: a[0] + a[1]x + a[2]x² + a[3]x³ + a[4]x⁴ = 0
/// Returns roots sorted in ascending order.
///
/// Hybrid solver: Ferrari first (fast), Bernstein fallback (reliable).
///
/// Ferrari handles most cases in ~100ns. When Ferrari misses roots
/// (detected by checking the expected root count from sign changes),
/// Bernstein subdivision finds them reliably but slower.
pub fn solve_quartic(a: &[f64; 5]) -> Vec<f64> {
    let [e, d, c, b, a4] = *a;

    if a4.abs() < 1e-30 {
        return solve_cubic_raw(e, d, c, b);
    }

    // Try Ferrari first
    let mut roots = solve_quartic_ferrari(a);

    // Verify: Ferrari can miss roots due to resolvent cubic instability.
    // Use Bernstein as fallback whenever Ferrari found fewer than 4 roots
    // (the maximum possible). This adds ~1μs overhead for the common case
    // where Ferrari found all roots, vs ~100μs for the Bernstein fallback.
    let bound = 1.0 + [e, d, c, b].iter().map(|x| (x / a4).abs()).fold(0.0f64, f64::max);
    let bound = bound.min(1e4);

    if roots.len() < 4 {
        // Bernstein fallback — find all roots reliably
        let intervals = super::bernstein::isolate_roots(a, -bound, bound, 1e-10);
        for (lo, hi) in intervals {
            let r = super::bernstein::refine_root(a, lo, hi, 1e-14);
            // Newton polish
            let mut x = r;
            for _ in 0..10 {
                let f = e + x * (d + x * (c + x * (b + x * a4)));
                let df = d + x * (2.0 * c + x * (3.0 * b + x * 4.0 * a4));
                if df.abs() < 1e-30 { break; }
                let step = f / df;
                x -= step;
                if step.abs() < 1e-14 { break; }
            }
            // Only add if not already found by Ferrari
            if !roots.iter().any(|r| (r - x).abs() < 1e-8) {
                roots.push(x);
            }
        }
        roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
        roots.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
    }

    roots
}

/// Find real roots of a univariate polynomial of arbitrary degree.
///
/// Coefficients are in ascending power order: `coeffs[i]` is the
/// coefficient of `x^i`. The polynomial's effective degree is the
/// index of the highest coefficient that is large *relative to the
/// rest of the polynomial* — symbolic algebra often produces ULP-
/// scale residuals at high degrees that should be treated as zero,
/// because failing to trim them causes the closed-form solvers to
/// invert a near-zero leading coefficient and emit spurious roots.
///
/// Strategy:
/// - Trim trailing relatively-near-zero coefficients (effective degree).
/// - Degree ≤ 4: dispatch to the closed-form quartic/cubic/quadratic
///   solvers. These are fast (~100 ns) and robust on simple roots.
/// - Degree > 4: Bernstein subdivision over a coefficient-derived
///   bound, then Newton polish each isolated root on the original
///   polynomial. Bounded iteration count, no closed-form precision
///   loss.
///
/// Returns roots sorted ascending, deduplicated to ~1e-10.
pub fn solve_univariate(coeffs: &[f64]) -> Vec<f64> {
    // Effective degree: trim trailing terms that are numerically
    // negligible *relative to the polynomial's overall magnitude*.
    // `1e-12 * max_abs` catches the ~1e-17 residuals that emerge
    // from BiPoly arithmetic on bidegree-(3, 3) inputs while still
    // preserving genuine small leading coefficients (those come with
    // proportionally small overall coefficients).
    let max_abs = coeffs.iter().map(|c| c.abs()).fold(0.0f64, f64::max);
    let trim_threshold = (max_abs * 1e-12).max(1e-30);
    let mut n = coeffs.len();
    while n > 0 && coeffs[n - 1].abs() < trim_threshold {
        n -= 1;
    }
    if n == 0 {
        return Vec::new();
    }
    let coeffs = &coeffs[..n];
    let deg = n - 1;

    match deg {
        0 => Vec::new(), // constant nonzero polynomial → no roots
        1 => {
            // a + b·x = 0
            let (a, b) = (coeffs[0], coeffs[1]);
            if b.abs() < 1e-30 { Vec::new() } else { vec![-a / b] }
        }
        2 => solve_quadratic_raw(coeffs[0], coeffs[1], coeffs[2]),
        3 => solve_cubic_raw(coeffs[0], coeffs[1], coeffs[2], coeffs[3]),
        4 => {
            let mut a = [0.0f64; 5];
            a[..5].copy_from_slice(&coeffs[..5]);
            solve_quartic(&a)
        }
        _ => solve_high_degree(coeffs),
    }
}

/// Bernstein-isolation path for polynomials of degree > 4. The
/// coefficient bound (Cauchy bound) gives an over-estimate of where
/// real roots can live; subdivision isolates each, Newton polishes.
fn solve_high_degree(coeffs: &[f64]) -> Vec<f64> {
    let n = coeffs.len() - 1;
    let lead = coeffs[n];
    if lead.abs() < 1e-30 {
        return Vec::new();
    }

    // Cauchy bound: |root| ≤ 1 + max_i (|c_i| / |lead|).
    let bound = 1.0
        + coeffs[..n]
            .iter()
            .map(|c| (c / lead).abs())
            .fold(0.0f64, f64::max);
    let bound = bound.min(1e6);

    let intervals = super::bernstein::isolate_roots(coeffs, -bound, bound, 1e-10);
    let mut roots = Vec::with_capacity(intervals.len());
    for (lo, hi) in intervals {
        let r = super::bernstein::refine_root(coeffs, lo, hi, 1e-14);
        // Newton polish on the original polynomial. Recovers precision
        // lost in the Bernstein conversion + bisection.
        let mut x = r;
        for _ in 0..16 {
            let (f, df) = eval_poly_and_deriv(coeffs, x);
            if df.abs() < 1e-30 {
                break;
            }
            let step = f / df;
            x -= step;
            if step.abs() < 1e-14 {
                break;
            }
        }
        // Drop polished roots that drifted off the polynomial entirely.
        let scale = coeffs.iter().map(|c| c.abs()).fold(0.0f64, f64::max).max(1.0);
        let f = eval_poly(coeffs, x);
        if f.abs() < 1e-6 * scale {
            roots.push(x);
        }
    }
    roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
    roots.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
    roots
}

fn eval_poly(coeffs: &[f64], x: f64) -> f64 {
    let mut acc = 0.0;
    for &c in coeffs.iter().rev() {
        acc = acc * x + c;
    }
    acc
}

fn eval_poly_and_deriv(coeffs: &[f64], x: f64) -> (f64, f64) {
    let mut p = 0.0;
    let mut dp = 0.0;
    for &c in coeffs.iter().rev() {
        dp = dp * x + p;
        p = p * x + c;
    }
    (p, dp)
}

/// Estimate real root count by probing sign changes at many points.
fn count_sign_changes_probed(a: &[f64; 5], bound: f64) -> usize {
    let eval = |x: f64| -> f64 {
        a[0] + x * (a[1] + x * (a[2] + x * (a[3] + x * a[4])))
    };
    let n = 20;
    let mut changes = 0;
    let mut prev_sign = 0i32;
    for i in 0..=n {
        let x = -bound + 2.0 * bound * i as f64 / n as f64;
        let v = eval(x);
        let sign = if v > 1e-10 { 1 } else if v < -1e-10 { -1 } else { 0 };
        if sign != 0 && prev_sign != 0 && sign != prev_sign {
            changes += 1;
        }
        if sign != 0 { prev_sign = sign; }
    }
    changes
}

/// Original Ferrari implementation — kept for reference and as fallback.
#[allow(dead_code)]
fn solve_quartic_ferrari(a: &[f64; 5]) -> Vec<f64> {
    let [e, d, c, b, a4] = *a;

    if a4.abs() < 1e-30 {
        // Degenerate: cubic or lower
        return solve_cubic_raw(e, d, c, b);
    }

    // Normalize: x⁴ + px³ + qx² + rx + s = 0
    let p = b / a4;
    let q = c / a4;
    let r = d / a4;
    let s = e / a4;

    // Depressed quartic via substitution x = t - p/4:
    // t⁴ + αt² + βt + γ = 0
    let p2 = p * p;
    let alpha = q - 3.0 * p2 / 8.0;
    let beta = r - p * q / 2.0 + p * p2 / 8.0;
    let gamma = s - p * r / 4.0 + p2 * q / 16.0 - 3.0 * p2 * p2 / 256.0;

    let shift = -p / 4.0;

    if beta.abs() < 1e-15 {
        // Biquadratic: t⁴ + αt² + γ = 0
        // Substitute u = t²
        let disc = alpha * alpha - 4.0 * gamma;
        if disc < -1e-15 { return Vec::new(); }
        let disc = disc.max(0.0).sqrt();
        let u1 = (-alpha + disc) / 2.0;
        let u2 = (-alpha - disc) / 2.0;

        let mut roots = Vec::new();
        if u1 >= -1e-15 {
            let t = u1.max(0.0).sqrt();
            roots.push(t + shift);
            if t > 1e-15 { roots.push(-t + shift); }
        }
        if u2 >= -1e-15 {
            let t = u2.max(0.0).sqrt();
            roots.push(t + shift);
            if t > 1e-15 { roots.push(-t + shift); }
        }
        roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
        roots.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        return roots;
    }

    // Ferrari's resolvent cubic: y³ + (α/2)y² + ((α²-4γ)/16)y - β²/64 = 0
    // We need one real root y₀ of this cubic.
    let rc_a = 1.0;
    let rc_b = alpha / 2.0;
    let rc_c = (alpha * alpha - 4.0 * gamma) / 16.0;
    let rc_d = -(beta * beta) / 64.0;

    let cubic_roots = solve_cubic_raw(rc_d, rc_c, rc_b, rc_a);
    if cubic_roots.is_empty() { return Vec::new(); }

    // Pick the largest real root of the resolvent cubic
    let y0 = *cubic_roots.last().unwrap();

    // Factor into two quadratics:
    // t² + (√(2y₀))t + (y₀ + α/2 + β/(2√(2y₀))) = 0
    // t² - (√(2y₀))t + (y₀ + α/2 - β/(2√(2y₀))) = 0
    let w = (2.0 * y0).max(0.0).sqrt();

    let mut roots = Vec::new();

    if w > 1e-15 {
        let q1 = y0 + alpha / 2.0 + beta / (2.0 * w);
        let q2 = y0 + alpha / 2.0 - beta / (2.0 * w);

        // First quadratic: t² + wt + q1 = 0
        let disc1 = w * w - 4.0 * q1;
        if disc1 >= -1e-15 {
            let sq = disc1.max(0.0).sqrt();
            roots.push((-w + sq) / 2.0 + shift);
            roots.push((-w - sq) / 2.0 + shift);
        }

        // Second quadratic: t² - wt + q2 = 0
        let disc2 = w * w - 4.0 * q2;
        if disc2 >= -1e-15 {
            let sq = disc2.max(0.0).sqrt();
            roots.push((w + sq) / 2.0 + shift);
            roots.push((w - sq) / 2.0 + shift);
        }
    }

    // Newton-polish each root on the original polynomial to recover
    // precision lost in the Ferrari reduction chain.
    for root in &mut roots {
        for _ in 0..10 {
            let x = *root;
            let f = e + x * (d + x * (c + x * (b + x * a4)));
            let df = d + x * (2.0 * c + x * (3.0 * b + x * 4.0 * a4));
            if df.abs() < 1e-30 { break; }
            let step = f / df;
            *root -= step;
            if step.abs() < 1e-14 { break; }
        }
    }

    // Filter: only keep roots where the polynomial is actually near zero.
    let scale = 1.0 + e.abs() + d.abs() + c.abs() + b.abs() + a4.abs();
    roots.retain(|x| {
        let f = e + x * (d + x * (c + x * (b + x * a4)));
        f.abs() < 1e-6 * scale
    });

    roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
    roots.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
    roots
}

/// Solve cubic: a + bx + cx² + dx³ = 0 using Cardano's formula.
fn solve_cubic_raw(a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
    if d.abs() < 1e-30 {
        return solve_quadratic_raw(a, b, c);
    }

    // Normalize: x³ + px² + qx + r = 0
    let p = c / d;
    let q = b / d;
    let r = a / d;

    // Depressed cubic via t = x - p/3:
    // t³ + αt + β = 0
    let alpha = q - p * p / 3.0;
    let beta = r - p * q / 3.0 + 2.0 * p * p * p / 27.0;
    let shift = -p / 3.0;

    let disc = -4.0 * alpha * alpha * alpha - 27.0 * beta * beta;

    if disc > 1e-15 {
        // Three real roots (casus irreducibilis)
        let m = (-alpha / 3.0).sqrt();
        let theta = (-beta / (2.0 * m * m * m)).clamp(-1.0, 1.0).acos() / 3.0;
        let mut roots = vec![
            2.0 * m * theta.cos() + shift,
            2.0 * m * (theta - std::f64::consts::TAU / 3.0).cos() + shift,
            2.0 * m * (theta + std::f64::consts::TAU / 3.0).cos() + shift,
        ];
        roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
        roots
    } else {
        // One or two real roots
        let sq = (beta * beta / 4.0 + alpha * alpha * alpha / 27.0).max(0.0).sqrt();
        let u = (-beta / 2.0 + sq).cbrt();
        let v = (-beta / 2.0 - sq).cbrt();
        let t = u + v;
        vec![t + shift]
    }
}

/// Solve quadratic: a + bx + cx² = 0
fn solve_quadratic_raw(a: f64, b: f64, c: f64) -> Vec<f64> {
    if c.abs() < 1e-30 {
        if b.abs() < 1e-30 { return Vec::new(); }
        return vec![-a / b];
    }
    let disc = b * b - 4.0 * c * a;
    if disc < -1e-15 { return Vec::new(); }
    let sq = disc.max(0.0).sqrt();
    let mut roots = vec![(-b + sq) / (2.0 * c), (-b - sq) / (2.0 * c)];
    roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
    roots.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quartic_known_roots() {
        // (x-1)(x-2)(x-3)(x-4) = x⁴ - 10x³ + 35x² - 50x + 24
        let roots = solve_quartic(&[24.0, -50.0, 35.0, -10.0, 1.0]);
        assert_eq!(roots.len(), 4);
        for (r, expected) in roots.iter().zip(&[1.0, 2.0, 3.0, 4.0]) {
            assert!((r - expected).abs() < 1e-8, "root {} != {}", r, expected);
        }
    }

    #[test]
    fn quartic_two_roots() {
        // x⁴ - 1 = 0, roots at ±1
        let roots = solve_quartic(&[-1.0, 0.0, 0.0, 0.0, 1.0]);
        assert_eq!(roots.len(), 2);
        assert!((roots[0] - (-1.0)).abs() < 1e-8);
        assert!((roots[1] - 1.0).abs() < 1e-8);
    }

    #[test]
    fn quartic_no_real_roots() {
        // x⁴ + 1 = 0, no real roots
        let roots = solve_quartic(&[1.0, 0.0, 0.0, 0.0, 1.0]);
        assert!(roots.is_empty());
    }

    #[test]
    fn quartic_double_root() {
        // (x-1)²(x-3)² = x⁴ - 8x³ + 22x² - 24x + 9
        let roots = solve_quartic(&[9.0, -24.0, 22.0, -8.0, 1.0]);
        assert!(roots.len() >= 2);
        assert!(roots.iter().any(|r| (r - 1.0).abs() < 1e-6));
        assert!(roots.iter().any(|r| (r - 3.0).abs() < 1e-6));
    }

    #[test]
    fn cubic_three_roots() {
        // (x-1)(x-2)(x-3) = x³ - 6x² + 11x - 6
        let roots = solve_cubic_raw(-6.0, 11.0, -6.0, 1.0);
        assert_eq!(roots.len(), 3);
    }

    /// Generic dispatcher: degree 1 through 6, verify all real roots
    /// are recovered and none spurious.
    #[test]
    fn univariate_dispatch_low_degrees() {
        // Linear: 2 + 3x = 0  →  x = -2/3
        let r = solve_univariate(&[2.0, 3.0]);
        assert_eq!(r.len(), 1);
        assert!((r[0] - (-2.0 / 3.0)).abs() < 1e-12);

        // Quadratic: x² - 5x + 6 = 0  →  {2, 3}
        let r = solve_univariate(&[6.0, -5.0, 1.0]);
        assert_eq!(r.len(), 2);
        assert!((r[0] - 2.0).abs() < 1e-12);
        assert!((r[1] - 3.0).abs() < 1e-12);

        // Cubic: (x-1)(x-2)(x-3)  →  {1, 2, 3}
        let r = solve_univariate(&[-6.0, 11.0, -6.0, 1.0]);
        assert_eq!(r.len(), 3);

        // Quartic: same as solve_quartic
        let r = solve_univariate(&[24.0, -50.0, 35.0, -10.0, 1.0]);
        assert_eq!(r.len(), 4);

        // Trailing zero: should drop to lower degree (cubic in disguise)
        let r = solve_univariate(&[-6.0, 11.0, -6.0, 1.0, 0.0]);
        assert_eq!(r.len(), 3);
    }

    /// Quintic and sextic — the >4 degree path. Bernstein subdivision
    /// + Newton polish.
    #[test]
    fn univariate_high_degree() {
        // Quintic with 5 simple roots: (x+2)(x+1)(x)(x-1)(x-2) = x^5 - 5x^3 + 4x
        // Coefficients (ascending): [0, 4, 0, -5, 0, 1]
        let r = solve_univariate(&[0.0, 4.0, 0.0, -5.0, 0.0, 1.0]);
        assert_eq!(r.len(), 5, "expected 5 real roots for the quintic, got {:?}", r);
        for &expected in &[-2.0, -1.0, 0.0, 1.0, 2.0] {
            assert!(r.iter().any(|&x| (x - expected).abs() < 1e-8),
                "missing root {}", expected);
        }

        // Sextic with 4 real roots and 2 complex:
        // (x²+1)(x-1)(x-2)(x-3)(x-4) = (x²+1)(x⁴ - 10x³ + 35x² - 50x + 24)
        // expand: x⁶ - 10x⁵ + 36x⁴ - 60x³ + 59x² - 50x + 24
        let r = solve_univariate(&[24.0, -50.0, 59.0, -60.0, 36.0, -10.0, 1.0]);
        assert_eq!(r.len(), 4, "expected 4 real roots, got {:?}", r);
        for &expected in &[1.0, 2.0, 3.0, 4.0] {
            assert!(r.iter().any(|&x| (x - expected).abs() < 1e-6),
                "missing root {}", expected);
        }
    }

    /// Degree 12 — torus-vs-bicubic-NURBS regime. Coefficients are
    /// bounded (the Wilkinson "factorial growth" polynomial is known
    /// to defeat any f64-precision solver). Realistic NURBS-derived
    /// polynomials have coefficients on the order of the CAD model's
    /// linear extent and don't suffer this; we test a small-magnitude
    /// case here.
    #[test]
    fn univariate_degree_twelve() {
        // Roots evenly spaced in [-1.1, 1.1]. Coefficients stay bounded.
        let roots_target: Vec<f64> = (0..12).map(|k| -1.1 + 0.2 * k as f64).collect();
        let mut coeffs = vec![1.0_f64];
        for &r in &roots_target {
            let mut new = vec![0.0; coeffs.len() + 1];
            for i in 0..coeffs.len() {
                new[i] -= coeffs[i] * r;
                new[i + 1] += coeffs[i];
            }
            coeffs = new;
        }
        let r = solve_univariate(&coeffs);
        assert_eq!(r.len(), 12, "expected 12 distinct roots, got {} ({:?})", r.len(), r);
        for target in &roots_target {
            assert!(r.iter().any(|&x| (x - target).abs() < 1e-6),
                "missing root {} (got {:?})", target, r);
        }
    }
}
