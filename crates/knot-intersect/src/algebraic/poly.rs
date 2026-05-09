//! Sparse bivariate polynomial with exact rational coefficients.
//!
//! Representation: Vec of (i, j, coeff) monomials where the polynomial is
//! Σ c_ij · x^i · y^j. Sorted by (i, j) for deterministic iteration.
//!
//! Operations: add, subtract, multiply, substitute, partial derivatives,
//! evaluate. All arithmetic is exact via `malachite_q::Rational`.

use malachite_q::Rational;
use std::collections::BTreeMap;

/// A sparse bivariate polynomial with exact rational coefficients.
/// P(x, y) = Σ c_{i,j} · x^i · y^j
#[derive(Clone, Debug)]
pub struct BiPoly {
    /// Coefficients indexed by (x_degree, y_degree).
    /// BTreeMap gives deterministic ordering.
    terms: BTreeMap<(u32, u32), Rational>,
}

impl BiPoly {
    /// Zero polynomial.
    pub fn zero() -> Self {
        Self { terms: BTreeMap::new() }
    }

    /// Constant polynomial.
    pub fn constant(c: Rational) -> Self {
        let mut p = Self::zero();
        if c != Rational::from(0) {
            p.terms.insert((0, 0), c);
        }
        p
    }

    /// Constant from f64. Uses exact rational representation.
    pub fn from_f64(v: f64) -> Self {
        Self::constant(Rational::try_from(v).unwrap_or(Rational::from(0)))
    }

    /// Single monomial: c · x^i · y^j
    pub fn monomial(i: u32, j: u32, c: Rational) -> Self {
        let mut p = Self::zero();
        if c != Rational::from(0) {
            p.terms.insert((i, j), c);
        }
        p
    }

    /// The variable x (= x^1 · y^0).
    pub fn x() -> Self {
        Self::monomial(1, 0, Rational::from(1))
    }

    /// The variable y (= x^0 · y^1).
    pub fn y() -> Self {
        Self::monomial(0, 1, Rational::from(1))
    }

    /// Is this the zero polynomial?
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    /// Total degree (max of i+j over all terms).
    pub fn total_degree(&self) -> u32 {
        self.terms.keys().map(|(i, j)| i + j).max().unwrap_or(0)
    }

    /// Degree in x.
    pub fn degree_x(&self) -> u32 {
        self.terms.keys().map(|(i, _)| *i).max().unwrap_or(0)
    }

    /// Degree in y.
    pub fn degree_y(&self) -> u32 {
        self.terms.keys().map(|(_, j)| *j).max().unwrap_or(0)
    }

    /// Number of nonzero terms.
    pub fn num_terms(&self) -> usize {
        self.terms.len()
    }

    /// Get coefficient of x^i · y^j.
    pub fn coeff(&self, i: u32, j: u32) -> &Rational {
        static ZERO: std::sync::LazyLock<Rational> = std::sync::LazyLock::new(|| Rational::from(0));
        self.terms.get(&(i, j)).unwrap_or(&ZERO)
    }

    /// Iterate over (i, j, &coeff) triples.
    pub fn iter(&self) -> impl Iterator<Item = (u32, u32, &Rational)> {
        self.terms.iter().map(|(&(i, j), c)| (i, j, c))
    }

    /// Add two polynomials.
    pub fn add(&self, other: &BiPoly) -> BiPoly {
        let mut result = self.terms.clone();
        for (&(i, j), c) in &other.terms {
            let entry = result.entry((i, j)).or_insert_with(|| Rational::from(0));
            *entry += c;
        }
        // Remove zero terms
        result.retain(|_, c| *c != Rational::from(0));
        BiPoly { terms: result }
    }

    /// Subtract: self - other.
    pub fn sub(&self, other: &BiPoly) -> BiPoly {
        let mut result = self.terms.clone();
        for (&(i, j), c) in &other.terms {
            let entry = result.entry((i, j)).or_insert_with(|| Rational::from(0));
            *entry -= c;
        }
        result.retain(|_, c| *c != Rational::from(0));
        BiPoly { terms: result }
    }

    /// Multiply two polynomials.
    pub fn mul(&self, other: &BiPoly) -> BiPoly {
        let mut result: BTreeMap<(u32, u32), Rational> = BTreeMap::new();
        for (&(i1, j1), c1) in &self.terms {
            for (&(i2, j2), c2) in &other.terms {
                let key = (i1 + i2, j1 + j2);
                let entry = result.entry(key).or_insert_with(|| Rational::from(0));
                *entry += c1 * c2;
            }
        }
        result.retain(|_, c| *c != Rational::from(0));
        BiPoly { terms: result }
    }

    /// Multiply by a scalar.
    pub fn scale(&self, s: &Rational) -> BiPoly {
        if *s == Rational::from(0) {
            return BiPoly::zero();
        }
        let terms = self.terms.iter()
            .map(|(&k, c)| (k, c * s))
            .filter(|(_, c)| *c != Rational::from(0))
            .collect();
        BiPoly { terms }
    }

    /// Partial derivative with respect to x.
    pub fn diff_x(&self) -> BiPoly {
        let mut terms = BTreeMap::new();
        for (&(i, j), c) in &self.terms {
            if i > 0 {
                let new_c = c * Rational::from(i);
                if new_c != Rational::from(0) {
                    terms.insert((i - 1, j), new_c);
                }
            }
        }
        BiPoly { terms }
    }

    /// Partial derivative with respect to y.
    pub fn diff_y(&self) -> BiPoly {
        let mut terms = BTreeMap::new();
        for (&(i, j), c) in &self.terms {
            if j > 0 {
                let new_c = c * Rational::from(j);
                if new_c != Rational::from(0) {
                    terms.insert((i, j - 1), new_c);
                }
            }
        }
        BiPoly { terms }
    }

    /// Substitute x = p(x, y) into self, keeping y as-is.
    /// Returns a new polynomial where every x^i becomes p(x,y)^i.
    pub fn substitute_x(&self, p: &BiPoly) -> BiPoly {
        let mut result = BiPoly::zero();
        // Group by x-degree
        let max_i = self.degree_x();
        // Precompute powers of p
        let mut p_powers = vec![BiPoly::constant(Rational::from(1))]; // p^0
        for _ in 1..=max_i {
            let next = p_powers.last().unwrap().mul(p);
            p_powers.push(next);
        }
        for (&(i, j), c) in &self.terms {
            // c · p^i · y^j
            let yj = BiPoly::monomial(0, j, Rational::from(1));
            let term = p_powers[i as usize].mul(&yj).scale(c);
            result = result.add(&term);
        }
        result
    }

    /// Substitute y = p(x, y) into self.
    pub fn substitute_y(&self, p: &BiPoly) -> BiPoly {
        let mut result = BiPoly::zero();
        let max_j = self.degree_y();
        let mut p_powers = vec![BiPoly::constant(Rational::from(1))];
        for _ in 1..=max_j {
            let next = p_powers.last().unwrap().mul(p);
            p_powers.push(next);
        }
        for (&(i, j), c) in &self.terms {
            let xi = BiPoly::monomial(i, 0, Rational::from(1));
            let term = p_powers[j as usize].mul(&xi).scale(c);
            result = result.add(&term);
        }
        result
    }

    /// Evaluate at (x, y) using exact rational arithmetic.
    pub fn eval(&self, x: &Rational, y: &Rational) -> Rational {
        let mut result = Rational::from(0);
        for (&(i, j), c) in &self.terms {
            let mut term = c.clone();
            for _ in 0..i { term *= x; }
            for _ in 0..j { term *= y; }
            result += term;
        }
        result
    }

    /// Evaluate at f64 (for numerical checks, not exact arithmetic).
    pub fn eval_f64(&self, x: f64, y: f64) -> f64 {
        let mut result = 0.0;
        for (&(i, j), c) in &self.terms {
            use malachite_base::num::conversion::traits::RoundingFrom;
            use malachite_base::rounding_modes::RoundingMode;
            let (cf, _) = f64::rounding_from(c, RoundingMode::Nearest);
            result += cf * x.powi(i as i32) * y.powi(j as i32);
        }
        result
    }

    /// Collect as a univariate polynomial in y with BiPoly-in-x coefficients.
    /// Returns vec of (y_degree, coefficient_poly_in_x).
    pub fn collect_y(&self) -> Vec<(u32, BiPoly)> {
        let mut by_j: BTreeMap<u32, BTreeMap<(u32, u32), Rational>> = BTreeMap::new();
        for (&(i, j), c) in &self.terms {
            by_j.entry(j)
                .or_default()
                .insert((i, 0), c.clone());
        }
        by_j.into_iter()
            .map(|(j, terms)| (j, BiPoly { terms }))
            .collect()
    }
}

// ── Operator overloads for convenience ──

impl std::ops::Add for &BiPoly {
    type Output = BiPoly;
    fn add(self, rhs: &BiPoly) -> BiPoly { self.add(rhs) }
}

impl std::ops::Sub for &BiPoly {
    type Output = BiPoly;
    fn sub(self, rhs: &BiPoly) -> BiPoly { self.sub(rhs) }
}

impl std::ops::Mul for &BiPoly {
    type Output = BiPoly;
    fn mul(self, rhs: &BiPoly) -> BiPoly { self.mul(rhs) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_and_zero() {
        let z = BiPoly::zero();
        assert!(z.is_zero());
        let c = BiPoly::from_f64(3.0);
        assert!(!c.is_zero());
        assert_eq!(c.total_degree(), 0);
    }

    #[test]
    fn add_and_sub() {
        let x = BiPoly::x();
        let y = BiPoly::y();
        let sum = &x + &y; // x + y
        assert_eq!(sum.num_terms(), 2);
        let diff = &sum - &x; // y
        assert_eq!(diff.num_terms(), 1);
        assert_eq!(*diff.coeff(0, 1), Rational::from(1));
    }

    #[test]
    fn multiply() {
        let x = BiPoly::x();
        let y = BiPoly::y();
        let xy = &x * &y; // x*y
        assert_eq!(xy.total_degree(), 2);
        assert_eq!(*xy.coeff(1, 1), Rational::from(1));

        // (x + 1) * (x + 1) = x² + 2x + 1
        let xp1 = &x + &BiPoly::from_f64(1.0);
        let sq = &xp1 * &xp1;
        assert_eq!(*sq.coeff(2, 0), Rational::from(1));
        assert_eq!(*sq.coeff(1, 0), Rational::from(2));
        assert_eq!(*sq.coeff(0, 0), Rational::from(1));
    }

    #[test]
    fn derivatives() {
        // P = 3x²y + 2xy² + y
        let p = BiPoly::monomial(2, 1, Rational::from(3))
            .add(&BiPoly::monomial(1, 2, Rational::from(2)))
            .add(&BiPoly::monomial(0, 1, Rational::from(1)));

        let dx = p.diff_x(); // 6xy + 2y²
        assert_eq!(*dx.coeff(1, 1), Rational::from(6));
        assert_eq!(*dx.coeff(0, 2), Rational::from(2));

        let dy = p.diff_y(); // 3x² + 4xy + 1
        assert_eq!(*dy.coeff(2, 0), Rational::from(3));
        assert_eq!(*dy.coeff(1, 1), Rational::from(4));
        assert_eq!(*dy.coeff(0, 0), Rational::from(1));
    }

    #[test]
    fn evaluation() {
        // P = x² + y² - 1 (unit circle)
        let p = &(&BiPoly::monomial(2, 0, Rational::from(1))
            + &BiPoly::monomial(0, 2, Rational::from(1)))
            - &BiPoly::from_f64(1.0);

        // (1, 0) should give 0
        let v = p.eval(&Rational::from(1), &Rational::from(0));
        assert_eq!(v, Rational::from(0));

        // (0, 0) should give -1
        let v = p.eval(&Rational::from(0), &Rational::from(0));
        assert_eq!(v, Rational::from(-1));
    }

    #[test]
    fn substitution() {
        // P = x + y, substitute x = 2s
        let p = &BiPoly::x() + &BiPoly::y();
        let two_s = BiPoly::x().scale(&Rational::from(2)); // 2x (treating x as s)
        let q = p.substitute_x(&two_s); // 2s + y
        assert_eq!(*q.coeff(1, 0), Rational::from(2));
        assert_eq!(*q.coeff(0, 1), Rational::from(1));
    }

    #[test]
    fn collect_by_y() {
        // P = x²y² + 3xy + 2
        let p = BiPoly::monomial(2, 2, Rational::from(1))
            .add(&BiPoly::monomial(1, 1, Rational::from(3)))
            .add(&BiPoly::from_f64(2.0));

        let collected = p.collect_y();
        assert_eq!(collected.len(), 3); // y^0, y^1, y^2 terms
    }
}
