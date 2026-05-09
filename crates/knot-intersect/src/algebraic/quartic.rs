//! Ferrari's quartic solver with deflation.
//!
//! Solves ax⁴ + bx³ + cx² + dx + e = 0 for real roots.
//! Uses the resolvent cubic to reduce to two quadratics.
//!
//! Walking skeleton: straightforward Ferrari. If topology priors tell us
//! which root to track, disambiguation is trivial. Upgrade to Strobach
//! only if double-root cases break this in practice.

/// Find all real roots of a quartic: a[0] + a[1]x + a[2]x² + a[3]x³ + a[4]x⁴ = 0
/// Returns roots sorted in ascending order.
pub fn solve_quartic(a: &[f64; 5]) -> Vec<f64> {
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

    roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
    roots.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
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
}
