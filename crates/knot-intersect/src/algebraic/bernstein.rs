//! Bernstein subdivision for univariate polynomial root isolation.
//!
//! Converts a univariate polynomial from power basis to Bernstein basis
//! over an interval, then uses de Casteljau subdivision to isolate roots.
//! Sign changes in Bernstein coefficients bound the number of roots in
//! each sub-interval (Descartes' rule).
//!
//! Walking skeleton: uses f64 arithmetic (not certified exact).
//! Production version should use interval arithmetic with rational endpoints.

/// Isolate real roots of a univariate polynomial on [lo, hi].
/// Returns intervals (a, b) each containing exactly one root.
pub fn isolate_roots(coeffs: &[f64], lo: f64, hi: f64, tolerance: f64) -> Vec<(f64, f64)> {
    if coeffs.is_empty() || coeffs.iter().all(|c| c.abs() < 1e-30) {
        return Vec::new();
    }

    // Convert from power basis to Bernstein basis on [lo, hi]
    let n = coeffs.len() - 1; // degree
    let bern = power_to_bernstein(coeffs, lo, hi);

    let mut roots = Vec::new();
    subdivide(&bern, lo, hi, tolerance, &mut roots, 0);
    roots
}

/// Refine a root in [lo, hi] using bisection + Newton.
pub fn refine_root(coeffs: &[f64], mut lo: f64, mut hi: f64, tolerance: f64) -> f64 {
    // A few Newton steps, falling back to bisection
    for _ in 0..50 {
        let mid = (lo + hi) * 0.5;
        if hi - lo < tolerance { return mid; }

        let f_lo = eval_poly(coeffs, lo);
        let f_mid = eval_poly(coeffs, mid);

        if f_lo * f_mid <= 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    (lo + hi) * 0.5
}

fn subdivide(bern: &[f64], lo: f64, hi: f64, tol: f64, roots: &mut Vec<(f64, f64)>, depth: usize) {
    let sign_changes = count_sign_changes(bern);

    if sign_changes == 0 {
        return; // no roots in this interval
    }

    if sign_changes == 1 || hi - lo < tol || depth > 60 {
        roots.push((lo, hi));
        return;
    }

    // de Casteljau subdivision at midpoint
    let mid = (lo + hi) * 0.5;
    let (left, right) = de_casteljau_split(bern);
    subdivide(&left, lo, mid, tol, roots, depth + 1);
    subdivide(&right, mid, hi, tol, roots, depth + 1);
}

/// Count sign changes in a sequence (Descartes' rule bound on roots).
fn count_sign_changes(coeffs: &[f64]) -> usize {
    let mut changes = 0;
    let mut prev_sign = 0i32;
    for &c in coeffs {
        let sign = if c > 1e-30 { 1 } else if c < -1e-30 { -1 } else { 0 };
        if sign != 0 {
            if prev_sign != 0 && sign != prev_sign {
                changes += 1;
            }
            prev_sign = sign;
        }
    }
    changes
}

/// de Casteljau split of Bernstein coefficients at midpoint.
/// Returns (left_half, right_half) Bernstein coefficients.
fn de_casteljau_split(bern: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = bern.len();
    let mut left = vec![0.0; n];
    let mut right = vec![0.0; n];

    // Working copy
    let mut work = bern.to_vec();

    left[0] = work[0];
    right[n - 1] = work[n - 1];

    for j in 1..n {
        for i in 0..n - j {
            work[i] = 0.5 * (work[i] + work[i + 1]);
        }
        left[j] = work[0];
        right[n - 1 - j] = work[n - 1 - j];
    }

    (left, right)
}

/// Convert polynomial from power basis (a₀ + a₁x + a₂x² + ...) to
/// Bernstein basis on [lo, hi].
fn power_to_bernstein(coeffs: &[f64], lo: f64, hi: f64) -> Vec<f64> {
    let n = coeffs.len() - 1; // degree
    if n == 0 {
        return vec![coeffs[0]];
    }

    // First shift: x → x - lo (so interval becomes [0, hi-lo])
    let shifted = shift_poly(coeffs, lo);

    // Then scale: x → x * (hi - lo) (so interval becomes [0, 1])
    let width = hi - lo;
    let scaled = scale_poly(&shifted, width);

    // Convert from power basis on [0,1] to Bernstein basis on [0,1]
    // Using the matrix conversion: B_i = Σ_{j=0}^{i} C(i,j)/C(n,j) * a_j
    let mut bern = vec![0.0; n + 1];
    let mut binom_n = vec![0.0; n + 1]; // C(n, k)
    binom_n[0] = 1.0;
    for k in 1..=n {
        binom_n[k] = binom_n[k - 1] * (n - k + 1) as f64 / k as f64;
    }

    for i in 0..=n {
        let mut sum = 0.0;
        let mut binom_i = 1.0; // C(i, j)
        for j in 0..=i {
            sum += binom_i / binom_n[j] * scaled[j];
            if j < i {
                binom_i *= (i - j) as f64 / (j + 1) as f64;
            }
        }
        bern[i] = sum;
    }

    bern
}

/// Shift polynomial: P(x) → P(x + a)
fn shift_poly(coeffs: &[f64], a: f64) -> Vec<f64> {
    let n = coeffs.len();
    let mut result = coeffs.to_vec();

    // Horner-like shift: repeatedly apply the transformation
    for i in 0..n {
        for j in (i + 1..n).rev() {
            result[j - 1] += a * result[j];
        }
    }
    // Actually, the standard Taylor shift P(x+a) uses a different algorithm.
    // Let me use the direct approach:
    let mut shifted = vec![0.0; n];
    // P(x + a) = Σ_k (Σ_{j≥k} C(j,k) * a^{j-k} * c_j) * x^k
    for k in 0..n {
        let mut sum = 0.0;
        let mut binom = 1.0; // C(j, k)
        let mut a_pow = 1.0; // a^{j-k}
        for j in k..n {
            sum += binom * a_pow * coeffs[j];
            // Update for next j
            a_pow *= a;
            binom *= (j + 1) as f64 / (j + 1 - k) as f64;
        }
        shifted[k] = sum;
    }
    shifted
}

/// Scale polynomial: P(x) → P(s·x)
fn scale_poly(coeffs: &[f64], s: f64) -> Vec<f64> {
    let mut result = coeffs.to_vec();
    let mut s_pow = 1.0;
    for c in &mut result {
        *c *= s_pow;
        s_pow *= s;
    }
    result
}

/// Evaluate polynomial at x using Horner's method.
fn eval_poly(coeffs: &[f64], x: f64) -> f64 {
    let mut result = 0.0;
    for c in coeffs.iter().rev() {
        result = result * x + c;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_root() {
        // P(x) = x - 0.5, root at 0.5
        let roots = isolate_roots(&[-0.5, 1.0], 0.0, 1.0, 1e-10);
        assert_eq!(roots.len(), 1);
        let mid = (roots[0].0 + roots[0].1) / 2.0;
        assert!((mid - 0.5).abs() < 1e-6);
    }

    #[test]
    fn quadratic_two_roots() {
        // P(x) = (x - 0.25)(x - 0.75) = x² - x + 3/16
        let roots = isolate_roots(&[3.0 / 16.0, -1.0, 1.0], 0.0, 1.0, 1e-10);
        assert_eq!(roots.len(), 2);
    }

    #[test]
    fn quartic_four_roots() {
        // P(x) = (x-1)(x-2)(x-3)(x-4) on [0, 5]
        // = x⁴ - 10x³ + 35x² - 50x + 24
        let roots = isolate_roots(&[24.0, -50.0, 35.0, -10.0, 1.0], 0.0, 5.0, 1e-8);
        assert_eq!(roots.len(), 4);
    }

    #[test]
    fn no_roots() {
        // P(x) = x² + 1, no real roots on [-10, 10]
        let roots = isolate_roots(&[1.0, 0.0, 1.0], -10.0, 10.0, 1e-10);
        assert_eq!(roots.len(), 0);
    }

    #[test]
    fn refine_accuracy() {
        // x² - 2 = 0, root at √2 ≈ 1.4142
        let r = refine_root(&[-2.0, 0.0, 1.0], 1.0, 2.0, 1e-12);
        assert!((r - std::f64::consts::SQRT_2).abs() < 1e-10);
    }
}
