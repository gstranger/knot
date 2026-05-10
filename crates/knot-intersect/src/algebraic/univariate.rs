//! Univariate polynomial operations: GCD, squarefree decomposition, derivative.
//!
//! Operates on dense f64 coefficient vectors: [a₀, a₁, ..., aₙ]
//! representing a₀ + a₁x + a₂x² + ... + aₙxⁿ.

/// Compute the derivative of a polynomial.
pub fn derivative(p: &[f64]) -> Vec<f64> {
    if p.len() <= 1 { return vec![]; }
    p[1..].iter().enumerate()
        .map(|(i, c)| c * (i + 1) as f64)
        .collect()
}

/// Polynomial GCD via the Euclidean algorithm.
/// Returns the monic GCD (leading coefficient = 1).
pub fn gcd(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut r0 = trim_leading_zeros(a);
    let mut r1 = trim_leading_zeros(b);

    if r0.is_empty() { return make_monic(&r1); }
    if r1.is_empty() { return make_monic(&r0); }

    while !r1.is_empty() && r1.iter().any(|c| c.abs() > 1e-10) {
        let rem = poly_rem(&r0, &r1);
        r0 = r1;
        r1 = trim_leading_zeros(&rem);
    }

    make_monic(&r0)
}

/// Polynomial division: returns quotient.
pub fn poly_div(a: &[f64], b: &[f64]) -> Vec<f64> {
    let (q, _) = poly_divmod(a, b);
    q
}

/// Polynomial remainder.
fn poly_rem(a: &[f64], b: &[f64]) -> Vec<f64> {
    let (_, r) = poly_divmod(a, b);
    r
}

/// Polynomial division with remainder: a = q*b + r.
fn poly_divmod(a: &[f64], b: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let b = trim_leading_zeros(b);
    if b.is_empty() { return (vec![], a.to_vec()); }

    let mut rem = a.to_vec();
    let db = b.len() - 1;
    let da = rem.len().saturating_sub(1);

    if da < db {
        return (vec![0.0], rem);
    }

    let mut quot = vec![0.0; da - db + 1];
    let lead_b = *b.last().unwrap();

    for i in (0..=da - db).rev() {
        let coeff = rem[i + db] / lead_b;
        quot[i] = coeff;
        for j in 0..=db {
            rem[i + j] -= coeff * b[j];
        }
    }

    (quot, trim_leading_zeros(&rem))
}

/// Yun's squarefree decomposition.
/// Returns Δ* = Δ / gcd(Δ, Δ') — the squarefree part of Δ.
/// All roots of Δ* are simple, so Bernstein sign-change detection
/// catches every critical point including tangent events.
pub fn squarefree_part(p: &[f64]) -> Vec<f64> {
    let p = trim_leading_zeros(p);
    if p.len() <= 1 { return p; }

    let dp = derivative(&p);
    if dp.is_empty() { return p; }

    let g = gcd(&p, &dp);

    // If gcd is constant (degree 0), p is already squarefree
    if g.len() <= 1 { return p; }

    // Δ* = Δ / gcd(Δ, Δ')
    poly_div(&p, &g)
}

fn trim_leading_zeros(p: &[f64]) -> Vec<f64> {
    let mut v = p.to_vec();
    while v.len() > 1 && v.last().map_or(false, |c| c.abs() < 1e-12) {
        v.pop();
    }
    v
}

fn make_monic(p: &[f64]) -> Vec<f64> {
    if p.is_empty() { return vec![]; }
    let lead = *p.last().unwrap();
    if lead.abs() < 1e-30 { return p.to_vec(); }
    p.iter().map(|c| c / lead).collect()
}

/// Evaluate polynomial at x.
pub fn eval(p: &[f64], x: f64) -> f64 {
    p.iter().rev().fold(0.0, |acc, c| acc * x + c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivative_basic() {
        // 3x² + 2x + 1 → 6x + 2
        let d = derivative(&[1.0, 2.0, 3.0]);
        assert_eq!(d.len(), 2);
        assert!((d[0] - 2.0).abs() < 1e-10);
        assert!((d[1] - 6.0).abs() < 1e-10);
    }

    #[test]
    fn gcd_coprime() {
        // gcd(x+1, x+2) = 1 (coprime)
        let g = gcd(&[1.0, 1.0], &[2.0, 1.0]);
        assert_eq!(g.len(), 1, "coprime polynomials have constant GCD");
    }

    #[test]
    fn gcd_common_factor() {
        // gcd((x-1)(x-2), (x-1)(x-3)) = (x-1)
        // (x-1)(x-2) = x²-3x+2
        // (x-1)(x-3) = x²-4x+3
        let g = gcd(&[2.0, -3.0, 1.0], &[3.0, -4.0, 1.0]);
        assert_eq!(g.len(), 2, "should find linear GCD");
        // GCD should be monic (x - 1), so g[0]/g[1] ≈ -1
        let root = -g[0] / g[1];
        assert!((root - 1.0).abs() < 1e-8, "GCD root should be 1, got {}", root);
    }

    #[test]
    fn squarefree_removes_double_root() {
        // (x-1)²(x-3) = x³ - 5x² + 7x - 3
        let p = [-3.0, 7.0, -5.0, 1.0];
        let sf = squarefree_part(&p);
        // squarefree part should be (x-1)(x-3) = x² - 4x + 3
        assert_eq!(sf.len(), 3, "squarefree should be degree 2, got {}", sf.len() - 1);
        // Roots of squarefree part
        let r1 = eval(&sf, 1.0);
        let r3 = eval(&sf, 3.0);
        assert!(r1.abs() < 1e-6, "sf(1) should be 0, got {}", r1);
        assert!(r3.abs() < 1e-6, "sf(3) should be 0, got {}", r3);
    }

    #[test]
    fn squarefree_already_squarefree() {
        // (x-1)(x-2)(x-3) — no repeated roots
        let p = [-6.0, 11.0, -6.0, 1.0];
        let sf = squarefree_part(&p);
        assert_eq!(sf.len(), p.len(), "already squarefree should keep same degree");
    }

    #[test]
    fn squarefree_quartic_double() {
        // (v-1)²(v-3)(v-5) = v⁴ - 10v³ + 32v² - 38v + 15
        let p = [15.0, -38.0, 32.0, -10.0, 1.0];
        let sf = squarefree_part(&p);
        // Should be degree 3: (v-1)(v-3)(v-5)
        assert_eq!(sf.len(), 4, "squarefree of quartic with one double should be cubic");
        assert!(eval(&sf, 1.0).abs() < 1e-4);
        assert!(eval(&sf, 3.0).abs() < 1e-4);
        assert!(eval(&sf, 5.0).abs() < 1e-4);
    }
}
