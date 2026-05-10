//! NURBS → BiPoly bridge.
//!
//! Phase 1 of the NURBS-vs-NURBS algebraic SSI. The pipeline:
//!
//! 1. NURBS surface (B-spline basis, possibly rational, possibly multi-span)
//! 2. Knot-insertion to elevate every interior knot to full multiplicity.
//!    The result is a piecewise-Bézier surface — one Bézier patch per
//!    non-degenerate knot rectangle.
//! 3. Each Bézier patch is converted to homogeneous-coordinate
//!    bivariate polynomials (X, Y, Z, W) with exact rational coefficients
//!    in (u, v) over the local domain [0, 1]².
//!
//! The actual surface point at local (u, v) is
//!     ( X(u, v) / W(u, v),  Y(u, v) / W(u, v),  Z(u, v) / W(u, v) ).
//!
//! For non-rational NURBS the weights are all 1 and W reduces to the
//! constant polynomial 1, but we always carry it through so consumers
//! (implicitization, substitution) can treat rational and polynomial
//! surfaces uniformly.
//!
//! The local-to-global map is recorded on each patch so callers can
//! convert back to the source NURBS parameter range.

use malachite_q::Rational;
use knot_geom::Point3;
use knot_geom::surface::NurbsSurface;
use super::poly::BiPoly;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// A single Bézier patch represented in homogeneous power-basis form.
#[derive(Clone, Debug)]
pub struct BezierPatch {
    pub degree_u: u32,
    pub degree_v: u32,
    /// Numerator polynomials (`x`, `y`, `z`) and the homogeneous
    /// denominator (`w`). The actual 3D point at local (u, v) is
    /// (x/w, y/w, z/w). Each polynomial is in (u, v) over [0, 1]².
    pub x: BiPoly,
    pub y: BiPoly,
    pub z: BiPoly,
    pub w: BiPoly,
    /// Source-NURBS parameter range that this patch covers.
    /// `global_u = u_range.0 + (u_range.1 - u_range.0) * local_u`.
    pub u_range: (f64, f64),
    pub v_range: (f64, f64),
}

impl BezierPatch {
    /// Evaluate at local (u, v) ∈ [0, 1]² in f64. Used for verifying
    /// the conversion against the source `NurbsSurface::point_at`.
    pub fn eval_f64(&self, u: f64, v: f64) -> Point3 {
        let xn = self.x.eval_f64(u, v);
        let yn = self.y.eval_f64(u, v);
        let zn = self.z.eval_f64(u, v);
        let wn = self.w.eval_f64(u, v);
        Point3::new(xn / wn, yn / wn, zn / wn)
    }

    /// Convert local patch parameter to source-NURBS parameter.
    pub fn local_to_global(&self, local_u: f64, local_v: f64) -> (f64, f64) {
        let g_u = self.u_range.0 + (self.u_range.1 - self.u_range.0) * local_u;
        let g_v = self.v_range.0 + (self.v_range.1 - self.v_range.0) * local_v;
        (g_u, g_v)
    }
}

/// Pascal's triangle up to row `n`. Returns `t[k][i] = C(k, i)` for
/// `0 ≤ i ≤ k ≤ n`. Exact rational coefficients (these are integers
/// stored as Rationals so they compose cleanly with the rest of the
/// arithmetic).
fn binomial_table(n: usize) -> Vec<Vec<Rational>> {
    let mut t = vec![vec![Rational::from(0); n + 1]; n + 1];
    for k in 0..=n {
        t[k][0] = Rational::from(1);
        for i in 1..=k {
            t[k][i] = if i == k {
                Rational::from(1)
            } else {
                &t[k - 1][i - 1] + &t[k - 1][i]
            };
        }
    }
    t
}

/// Build the Bernstein → power basis matrix for degree `n`. Returns
/// `m` such that `m[i][j]` is the coefficient of `t^j` in the i-th
/// Bernstein basis function `B_i^n(t) = C(n, i) t^i (1 - t)^(n - i)`.
///
/// Expanding (1 - t)^(n - i) gives:
///   B_i^n(t) = C(n, i) Σ_{j=i}^{n} (-1)^(j-i) C(n-i, j-i) t^j
///
/// so `m[i][j] = (-1)^(j-i) C(n, i) C(n-i, j-i)` for `j ≥ i`, else 0.
fn bernstein_power_matrix(n: u32) -> Vec<Vec<Rational>> {
    let n = n as usize;
    let binom = binomial_table(n);
    let mut m = vec![vec![Rational::from(0); n + 1]; n + 1];
    for i in 0..=n {
        for j in i..=n {
            let mag = &binom[n][i] * &binom[n - i][j - i];
            m[i][j] = if (j - i) % 2 == 0 { mag } else { -mag };
        }
    }
    m
}

/// Convert an `(degree_u + 1) × (degree_v + 1)` grid of weighted
/// control points (one Bézier patch in tensor-product form) into the
/// four homogeneous polynomials.
///
/// The patch is parameterized over the local rectangle [0, 1]². Layout
/// of `cps`: `cps[i][j]` is the control point at row i (along u) and
/// column j (along v).
pub fn bezier_grid_to_patch(
    degree_u: u32,
    degree_v: u32,
    cps: &[Vec<(Point3, f64)>],
    u_range: (f64, f64),
    v_range: (f64, f64),
) -> BezierPatch {
    assert_eq!(cps.len(), degree_u as usize + 1, "u row count mismatch");
    for row in cps {
        assert_eq!(row.len(), degree_v as usize + 1, "v column count mismatch");
    }

    let pu = bernstein_power_matrix(degree_u);
    let pv = bernstein_power_matrix(degree_v);
    let nu = degree_u as usize;
    let nv = degree_v as usize;

    let mut x = BiPoly::zero();
    let mut y = BiPoly::zero();
    let mut z = BiPoly::zero();
    let mut w = BiPoly::zero();

    let zero = Rational::from(0);

    // Decomposition: each Bernstein product B_i^p(u) · B_j^q(v) lifts
    // to a sum over (k1, k2) ≥ (i, j) of m_u[i][k1] · m_v[j][k2] · u^k1 v^k2.
    // Multiply by the homogeneous-control-point coefficients
    // (w_ij·P_ij.x, w_ij·P_ij.y, w_ij·P_ij.z, w_ij), accumulate.
    for i in 0..=nu {
        for j in 0..=nv {
            let (cp, weight) = &cps[i][j];
            let wr = rat(*weight);
            if wr == zero {
                continue;
            }
            let xr = &rat(cp.x) * &wr;
            let yr = &rat(cp.y) * &wr;
            let zr = &rat(cp.z) * &wr;

            for k1 in i..=nu {
                let cu = &pu[i][k1];
                if *cu == zero {
                    continue;
                }
                for k2 in j..=nv {
                    let cv = &pv[j][k2];
                    if *cv == zero {
                        continue;
                    }
                    let bcoeff = cu * cv;
                    let mono = BiPoly::monomial(k1 as u32, k2 as u32, bcoeff);
                    if xr != zero {
                        x = x.add(&mono.scale(&xr));
                    }
                    if yr != zero {
                        y = y.add(&mono.scale(&yr));
                    }
                    if zr != zero {
                        z = z.add(&mono.scale(&zr));
                    }
                    w = w.add(&mono.scale(&wr));
                }
            }
        }
    }

    BezierPatch { degree_u, degree_v, x, y, z, w, u_range, v_range }
}

/// Decompose a `NurbsSurface` into a list of Bézier patches via knot
/// insertion (Boehm's algorithm) in both parametric directions.
///
/// Strategy: insert each interior u-knot until full multiplicity p_u,
/// then insert each interior v-knot until full multiplicity p_v. The
/// result is a piecewise-Bézier surface. For each non-degenerate knot
/// rectangle, extract the (p_u + 1) × (p_v + 1) control-point grid
/// (with weights) and run it through `bezier_grid_to_patch`.
///
/// **Caveat (Phase 1 scope):** this works on the surface's source
/// control net. For surfaces whose interior knots already have full
/// multiplicity (single Bézier patch) the path is trivial; for true
/// piecewise NURBS (multiple knot spans) we need the insertion
/// mechanic. The implementation handles both.
pub fn nurbs_to_bezier_patches(s: &NurbsSurface) -> Vec<BezierPatch> {
    let pu = s.degree_u() as usize;
    let pv = s.degree_v() as usize;
    let nu_cp = s.count_u() as usize;
    let nv_cp = s.count_v() as usize;
    let knots_u: Vec<f64> = s.knots_u().to_vec();
    let knots_v: Vec<f64> = s.knots_v().to_vec();

    // Step 1: build a row-major control-point grid `cp[i][j]` with
    // the source weights folded in. We work entirely in homogeneous
    // (P*w, w) and convert back at the end.
    let mut cp: Vec<Vec<(Point3, f64)>> = (0..nu_cp)
        .map(|i| {
            (0..nv_cp)
                .map(|j| {
                    let idx = i * nv_cp + j;
                    (s.control_points()[idx], s.weights()[idx])
                })
                .collect()
        })
        .collect();
    let mut knots_u = knots_u;
    let mut knots_v = knots_v;

    // Step 2: insert all interior u-knots to full multiplicity p_u.
    insert_to_full_multiplicity_u(&mut cp, &mut knots_u, pu);
    // Step 3: insert all interior v-knots to full multiplicity p_v.
    insert_to_full_multiplicity_v(&mut cp, &mut knots_v, pv);

    // After insertion, knots_u looks like:
    //   [u_0]*(p_u+1) [u_1]*(p_u) [u_2]*(p_u) ... [u_M]*(p_u) [u_M+1]*(p_u+1)
    // The interior knot values (u_1 through u_M) define M+1 Bézier
    // spans in u: [u_0, u_1], [u_1, u_2], ..., [u_M, u_M+1].
    let unique_u = unique_knot_values(&knots_u);
    let unique_v = unique_knot_values(&knots_v);

    let mut patches = Vec::new();
    for span_u in 0..unique_u.len() - 1 {
        let u_lo = unique_u[span_u];
        let u_hi = unique_u[span_u + 1];
        if (u_hi - u_lo).abs() < 1e-15 {
            continue; // degenerate span (shouldn't happen after dedup)
        }
        let u_off = span_u * pu; // first row of this span's Bézier net

        for span_v in 0..unique_v.len() - 1 {
            let v_lo = unique_v[span_v];
            let v_hi = unique_v[span_v + 1];
            if (v_hi - v_lo).abs() < 1e-15 {
                continue;
            }
            let v_off = span_v * pv;

            // Extract the (p_u + 1) × (p_v + 1) sub-grid of cp.
            let mut sub: Vec<Vec<(Point3, f64)>> = Vec::with_capacity(pu + 1);
            for i in 0..=pu {
                let mut row = Vec::with_capacity(pv + 1);
                for j in 0..=pv {
                    row.push(cp[u_off + i][v_off + j]);
                }
                sub.push(row);
            }
            patches.push(bezier_grid_to_patch(
                pu as u32,
                pv as u32,
                &sub,
                (u_lo, u_hi),
                (v_lo, v_hi),
            ));
        }
    }

    patches
}

/// Strict-decreasing-multiplicity dedup: return the distinct knot
/// values in increasing order. After full-multiplicity insertion the
/// knot vector has each interior value repeated `p` times and the
/// boundary values repeated `p + 1` times; deduping recovers the
/// breakpoint sequence.
fn unique_knot_values(knots: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(knots.len());
    for &k in knots {
        if out.last().map_or(true, |&last: &f64| (last - k).abs() > 1e-12) {
            out.push(k);
        }
    }
    out
}

/// Insert each interior u-knot value until its multiplicity equals `p`.
/// Operates on the row-major control-point grid in place, mutating
/// `knots` to the new knot vector.
fn insert_to_full_multiplicity_u(
    cp: &mut Vec<Vec<(Point3, f64)>>,
    knots: &mut Vec<f64>,
    p: usize,
) {
    // Walk through the unique interior knot values; for each, count
    // current multiplicity and insert (p - mult) copies via Boehm.
    let mut i = p + 1; // first interior knot index (after p+1 boundary)
    while i < knots.len() - p - 1 {
        let val = knots[i];
        // Count multiplicity of `val` here.
        let mut mult = 0usize;
        let mut j = i;
        while j < knots.len() - p - 1 && (knots[j] - val).abs() < 1e-12 {
            mult += 1;
            j += 1;
        }
        // Insert (p - mult) copies of `val`.
        for _ in 0..(p.saturating_sub(mult)) {
            insert_u_knot(cp, knots, val, p);
        }
        i = j + (p - mult); // skip over the now-full-multiplicity block
    }
}

fn insert_to_full_multiplicity_v(
    cp: &mut Vec<Vec<(Point3, f64)>>,
    knots: &mut Vec<f64>,
    p: usize,
) {
    let mut i = p + 1;
    while i < knots.len() - p - 1 {
        let val = knots[i];
        let mut mult = 0usize;
        let mut j = i;
        while j < knots.len() - p - 1 && (knots[j] - val).abs() < 1e-12 {
            mult += 1;
            j += 1;
        }
        for _ in 0..(p.saturating_sub(mult)) {
            insert_v_knot(cp, knots, val, p);
        }
        i = j + (p - mult);
    }
}

/// Boehm knot insertion in u-direction. Inserts knot value `t` once,
/// updating the control-point grid (each row is one v-stripe). After
/// insertion the row count grows by 1; affected rows in the range
/// [k - p + 1, k] get replaced by p new rows.
fn insert_u_knot(
    cp: &mut Vec<Vec<(Point3, f64)>>,
    knots: &mut Vec<f64>,
    t: f64,
    p: usize,
) {
    // Find span k such that knots[k] <= t < knots[k+1].
    let k = find_span(knots, t, p);
    let n_v = cp[0].len();

    let mut new_cp = Vec::with_capacity(cp.len() + 1);
    // Rows 0..=k-p stay unchanged.
    for i in 0..=(k as isize - p as isize).max(0) as usize {
        if i < cp.len() {
            new_cp.push(cp[i].clone());
        }
    }
    // Compute the p new rows for indices [k - p + 1, k].
    for i in (k as isize - p as isize + 1).max(0) as usize..=k {
        if i >= cp.len() || i + 1 > knots.len() {
            break;
        }
        let denom = knots[i + p] - knots[i];
        if denom.abs() < 1e-15 {
            new_cp.push(cp[i].clone());
            continue;
        }
        let alpha = (t - knots[i]) / denom;
        let mut blended_row: Vec<(Point3, f64)> = Vec::with_capacity(n_v);
        for j in 0..n_v {
            // Blend in homogeneous coordinates: (P*w, w).
            let (p0, w0) = cp[i - 1][j];
            let (p1, w1) = cp[i][j];
            let hw0 = w0;
            let hw1 = w1;
            let hp0 = p0 * w0;
            let hp1 = p1 * w1;
            let hw = (1.0 - alpha) * hw0 + alpha * hw1;
            let hp_x = (1.0 - alpha) * hp0.x + alpha * hp1.x;
            let hp_y = (1.0 - alpha) * hp0.y + alpha * hp1.y;
            let hp_z = (1.0 - alpha) * hp0.z + alpha * hp1.z;
            blended_row.push((
                Point3::new(hp_x / hw, hp_y / hw, hp_z / hw),
                hw,
            ));
        }
        new_cp.push(blended_row);
    }
    // Rows k..=last stay unchanged.
    for i in k..cp.len() {
        new_cp.push(cp[i].clone());
    }

    *cp = new_cp;

    // Insert t into knots at position k+1.
    knots.insert(k + 1, t);
}

/// Boehm knot insertion in v-direction. Same algorithm, but each
/// column is the affected stripe.
fn insert_v_knot(
    cp: &mut Vec<Vec<(Point3, f64)>>,
    knots: &mut Vec<f64>,
    t: f64,
    p: usize,
) {
    let k = find_span(knots, t, p);
    let n_u = cp.len();
    if n_u == 0 { return; }
    let n_v = cp[0].len();

    let mut new_cp: Vec<Vec<(Point3, f64)>> = (0..n_u).map(|_| Vec::with_capacity(n_v + 1)).collect();

    for j in 0..=(k as isize - p as isize).max(0) as usize {
        if j < n_v {
            for i in 0..n_u {
                new_cp[i].push(cp[i][j]);
            }
        }
    }
    for j in (k as isize - p as isize + 1).max(0) as usize..=k {
        if j >= n_v || j + p >= knots.len() {
            break;
        }
        let denom = knots[j + p] - knots[j];
        let alpha = if denom.abs() < 1e-15 { 0.0 } else { (t - knots[j]) / denom };
        for i in 0..n_u {
            let (p0, w0) = cp[i][j - 1];
            let (p1, w1) = cp[i][j];
            let hw0 = w0;
            let hw1 = w1;
            let hp0 = p0 * w0;
            let hp1 = p1 * w1;
            let hw = (1.0 - alpha) * hw0 + alpha * hw1;
            let hp_x = (1.0 - alpha) * hp0.x + alpha * hp1.x;
            let hp_y = (1.0 - alpha) * hp0.y + alpha * hp1.y;
            let hp_z = (1.0 - alpha) * hp0.z + alpha * hp1.z;
            new_cp[i].push((Point3::new(hp_x / hw, hp_y / hw, hp_z / hw), hw));
        }
    }
    for j in k..n_v {
        for i in 0..n_u {
            new_cp[i].push(cp[i][j]);
        }
    }

    *cp = new_cp;
    knots.insert(k + 1, t);
}

/// Standard knot-span lookup. Returns `k` such that
/// `knots[k] <= t < knots[k + 1]`, with t clamped to the active
/// (non-boundary) region [knots[p], knots[len - p - 1]].
fn find_span(knots: &[f64], t: f64, p: usize) -> usize {
    let n = knots.len() - p - 2;
    if t >= knots[n + 1] {
        return n;
    }
    if t <= knots[p] {
        return p;
    }
    let mut low = p;
    let mut high = n + 1;
    let mut mid = (low + high) / 2;
    while t < knots[mid] || t >= knots[mid + 1] {
        if t < knots[mid] {
            high = mid;
        } else {
            low = mid;
        }
        mid = (low + high) / 2;
    }
    mid
}

/// Convert f64 to exact rational. Each finite f64 is a finite rational
/// (mantissa × 2^exponent) so this is loss-less.
fn rat(v: f64) -> Rational {
    Rational::try_from(v).unwrap_or(Rational::from(0))
}

// ─────────────────────────────────────────────────────────────────────
// Thread-local Bezier-patch cache.
//
// Decomposing a NURBS surface to Bézier patches involves Boehm knot
// insertion (homogeneous-coord blending) plus power-basis conversion
// to BiPoly with exact rationals — both expensive. In the boolean
// pipeline a single NURBS face is paired with many faces from the
// other solid, so the same surface is decomposed dozens of times.
//
// We cache by `*const NurbsSurface` (pointer identity, not content
// hash) for O(1) lookup. The boolean op clears the cache at entry and
// exit so the pointer keys are valid for the entire lifetime of any
// cache entry — there's no risk of address reuse pointing to
// different content.
//
// Thread-local storage scopes the cache per-thread automatically;
// future parallelization (rayon) gets independent caches per worker.
// ─────────────────────────────────────────────────────────────────────

thread_local! {
    static BEZIER_PATCH_CACHE:
        RefCell<HashMap<*const NurbsSurface, Arc<Vec<BezierPatch>>>> =
        RefCell::new(HashMap::new());
}

/// Cached form of `nurbs_to_bezier_patches`. Returns an `Arc` so the
/// cache and caller share ownership cheaply. First call for a given
/// NURBS pointer decomposes; subsequent calls for the same pointer
/// return the cached `Arc<Vec<BezierPatch>>`.
pub fn cached_nurbs_to_bezier_patches(nurbs: &NurbsSurface) -> Arc<Vec<BezierPatch>> {
    let key = nurbs as *const NurbsSurface;
    BEZIER_PATCH_CACHE.with(|cache| {
        if let Some(p) = cache.borrow().get(&key) {
            return p.clone();
        }
        let patches = Arc::new(nurbs_to_bezier_patches(nurbs));
        cache.borrow_mut().insert(key, patches.clone());
        patches
    })
}

/// Drop all entries from the thread-local Bezier-patch cache.
/// Called by the boolean op at entry and exit so the cache only
/// holds entries whose `*const NurbsSurface` keys are guaranteed
/// alive for the entire window.
pub fn clear_bezier_patch_cache() {
    BEZIER_PATCH_CACHE.with(|cache| cache.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;
    use knot_geom::Point3;

    fn approx_pt(a: Point3, b: Point3, eps: f64) -> bool {
        (a.x - b.x).abs() < eps && (a.y - b.y).abs() < eps && (a.z - b.z).abs() < eps
    }

    /// Bernstein basis sums to 1 at every parameter (partition of unity).
    /// In power basis, each B_i^n is a polynomial; we verify by summing
    /// rows of the matrix (sum should equal the polynomial 1).
    #[test]
    fn bernstein_partition_of_unity() {
        for n in [1u32, 2, 3, 4, 5] {
            let m = bernstein_power_matrix(n);
            for j in 0..=n as usize {
                let mut sum = Rational::from(0);
                for i in 0..=n as usize {
                    sum += &m[i][j];
                }
                let expected = if j == 0 { Rational::from(1) } else { Rational::from(0) };
                assert_eq!(sum, expected,
                    "row sum at u^{} for degree {} should be {} (sum of B_i^n is the polynomial 1, so coeff of u^j is δ_j0)",
                    j, n, expected);
            }
        }
    }

    /// Bernstein basis evaluated at t=0: B_0^n(0) = 1, all others = 0.
    #[test]
    fn bernstein_at_zero() {
        for n in [1u32, 3, 5] {
            let m = bernstein_power_matrix(n);
            for i in 0..=n as usize {
                // Evaluating a polynomial Σ m[i][k] t^k at t=0 gives m[i][0].
                let expected = if i == 0 { Rational::from(1) } else { Rational::from(0) };
                assert_eq!(m[i][0], expected, "B_{}^{}(0) coeff at t^0", i, n);
            }
        }
    }

    /// Single bilinear (degree 1×1) Bézier patch over a flat plate.
    /// Control points form a unit square in z=0; converted polynomial
    /// must reproduce the plate exactly.
    #[test]
    fn flat_plate_bilinear() {
        let cps = vec![
            vec![(Point3::new(0.0, 0.0, 0.0), 1.0), (Point3::new(0.0, 1.0, 0.0), 1.0)],
            vec![(Point3::new(1.0, 0.0, 0.0), 1.0), (Point3::new(1.0, 1.0, 0.0), 1.0)],
        ];
        let patch = bezier_grid_to_patch(1, 1, &cps, (0.0, 1.0), (0.0, 1.0));
        for &u in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            for &v in &[0.0, 0.3, 0.7, 1.0] {
                let p = patch.eval_f64(u, v);
                let expected = Point3::new(u, v, 0.0);
                assert!(approx_pt(p, expected, 1e-12),
                    "patch({u}, {v}) = ({}, {}, {}) != expected ({}, {}, 0)",
                    p.x, p.y, p.z, expected.x, expected.y);
            }
        }
        // W should be the constant 1 (all weights 1, partition of unity).
        assert_eq!(patch.w.eval_f64(0.5, 0.5), 1.0);
    }

    /// Bicubic Bézier patch with non-trivial geometry (a "hat" shape).
    /// Compare against direct Bernstein evaluation.
    #[test]
    fn bicubic_matches_direct_bernstein() {
        let cps: Vec<Vec<(Point3, f64)>> = (0..4)
            .map(|i| {
                (0..4)
                    .map(|j| {
                        let u = i as f64 / 3.0;
                        let v = j as f64 / 3.0;
                        let z = (4.0 * (u - 0.5).powi(2) + (v - 0.5).powi(2)).sin();
                        (Point3::new(u, v, z), 1.0)
                    })
                    .collect()
            })
            .collect();
        let patch = bezier_grid_to_patch(3, 3, &cps, (0.0, 1.0), (0.0, 1.0));

        // Direct Bernstein evaluation as oracle.
        let direct = |u: f64, v: f64| -> Point3 {
            let bu = |i: usize| -> f64 {
                let n = 3u32;
                let c = [1.0, 3.0, 3.0, 1.0][i];
                c * u.powi(i as i32) * (1.0 - u).powi((n as usize - i) as i32)
            };
            let bv = |j: usize| -> f64 {
                let c = [1.0, 3.0, 3.0, 1.0][j];
                c * v.powi(j as i32) * (1.0 - v).powi((3 - j) as i32)
            };
            let mut p = [0.0; 3];
            for i in 0..4 {
                for j in 0..4 {
                    let (cp, w) = cps[i][j];
                    let bw = bu(i) * bv(j) * w;
                    p[0] += bw * cp.x;
                    p[1] += bw * cp.y;
                    p[2] += bw * cp.z;
                }
            }
            Point3::new(p[0], p[1], p[2])
        };

        for &u in &[0.0, 0.1, 0.5, 0.9, 1.0] {
            for &v in &[0.0, 0.2, 0.6, 1.0] {
                let actual = patch.eval_f64(u, v);
                let expected = direct(u, v);
                assert!(approx_pt(actual, expected, 1e-9),
                    "bicubic({u}, {v}) = {actual:?} vs direct {expected:?}");
            }
        }
    }

    /// Rational quadratic Bézier representing a quarter-circle arc
    /// extruded as a degenerate surface (constant in v). Verify the
    /// homogeneous denominator W ≠ 1 and the projected point lies
    /// on the unit circle.
    #[test]
    fn rational_quarter_circle() {
        // Standard rational quadratic for 90° circular arc:
        //   P0 = (1, 0), w0 = 1
        //   P1 = (1, 1), w1 = sqrt(2)/2  (≈ 0.7071)
        //   P2 = (0, 1), w2 = 1
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let cps: Vec<Vec<(Point3, f64)>> = vec![
            vec![(Point3::new(1.0, 0.0, 0.0), 1.0), (Point3::new(1.0, 0.0, 1.0), 1.0)],
            vec![(Point3::new(1.0, 1.0, 0.0), inv_sqrt2), (Point3::new(1.0, 1.0, 1.0), inv_sqrt2)],
            vec![(Point3::new(0.0, 1.0, 0.0), 1.0), (Point3::new(0.0, 1.0, 1.0), 1.0)],
        ];
        let patch = bezier_grid_to_patch(2, 1, &cps, (0.0, 1.0), (0.0, 1.0));

        for u_step in 0..=10 {
            let u = u_step as f64 / 10.0;
            let p = patch.eval_f64(u, 0.5);
            let r = (p.x * p.x + p.y * p.y).sqrt();
            assert!((r - 1.0).abs() < 1e-12,
                "rational arc point at u={u} should have radius 1, got {r} (point {p:?})");
        }
    }

    /// Single-span (Bézier) NURBS surface: knot vector has full
    /// boundary multiplicity and no interior knots. Decomposition
    /// produces exactly one patch matching the source.
    #[test]
    fn single_span_nurbs_to_one_patch() {
        // Bicubic patch, 4×4 control net, simple plane.
        let cps: Vec<Point3> = (0..4)
            .flat_map(|i| {
                (0..4).map(move |j| Point3::new(i as f64 / 3.0, j as f64 / 3.0, 0.0))
            })
            .collect();
        let weights = vec![1.0; 16];
        let knots_u = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let knots_v = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let s = NurbsSurface::new(cps, weights, knots_u, knots_v, 3, 3, 4, 4).unwrap();

        let patches = nurbs_to_bezier_patches(&s);
        assert_eq!(patches.len(), 1, "single-span NURBS → exactly one Bézier patch");
        assert_eq!(patches[0].degree_u, 3);
        assert_eq!(patches[0].degree_v, 3);
        assert_eq!(patches[0].u_range, (0.0, 1.0));
        assert_eq!(patches[0].v_range, (0.0, 1.0));

        // Pointwise match.
        for &u in &[0.05, 0.5, 0.95] {
            for &v in &[0.1, 0.4, 0.9] {
                let p_nurbs = s.point_at(u, v);
                let p_patch = patches[0].eval_f64(u, v);
                assert!(approx_pt(p_nurbs, p_patch, 1e-10),
                    "NURBS({u}, {v}) = {p_nurbs:?} vs patch {p_patch:?}");
            }
        }
    }

    /// Rational NURBS surface with one interior knot AND non-uniform
    /// weights. Stresses the homogeneous-coordinate path through
    /// Boehm knot insertion: the inserted control points must be
    /// blended in (P*w, w) homogeneous form, not just (P, w)
    /// cartesian form, otherwise rational geometry drifts.
    #[test]
    fn rational_multi_span_preserves_geometry() {
        // Rational bicubic surface with weights varying across the
        // control net. The exact geometry isn't important — the test
        // is that BezierPatch evaluation reproduces source NURBS.
        let cps: Vec<Point3> = (0..5)
            .flat_map(|i| {
                (0..5).map(move |j| {
                    let u = i as f64 / 4.0;
                    let v = j as f64 / 4.0;
                    Point3::new(u, v, (u * v).sqrt())
                })
            })
            .collect();
        // Diverse positive weights (avoid 1.0 to actually exercise rational arithmetic).
        let weights: Vec<f64> = (0..25).map(|k| 0.5 + (k as f64) * 0.1).collect();
        let knots_u = vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0];
        let knots_v = vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0];
        let s = NurbsSurface::new(cps, weights, knots_u, knots_v, 3, 3, 5, 5).unwrap();

        let patches = nurbs_to_bezier_patches(&s);
        assert_eq!(patches.len(), 4);

        // Tolerance is looser than the non-rational case because Boehm
        // insertion in cartesian form (which we do for backward
        // compat with NurbsSurface's f64 control points) accumulates
        // a few ULPs per insertion. 1e-9 is well within geometric
        // tolerance for any practical CAD model.
        for patch in &patches {
            for &lu in &[0.05, 0.5, 0.95] {
                for &lv in &[0.05, 0.5, 0.95] {
                    let (gu, gv) = patch.local_to_global(lu, lv);
                    let p_nurbs = s.point_at(gu, gv);
                    let p_patch = patch.eval_f64(lu, lv);
                    assert!(approx_pt(p_nurbs, p_patch, 1e-9),
                        "rational NURBS({gu:.3}, {gv:.3}) = {p_nurbs:?} vs patch ({lu}, {lv}) = {p_patch:?}");
                }
            }
        }
    }

    /// Multi-span NURBS with one interior knot in u, one in v. The
    /// decomposition should produce 4 Bézier patches (2×2 grid of
    /// knot rectangles), and each patch should reproduce the source
    /// surface within its global parameter range.
    #[test]
    fn multi_span_nurbs_to_four_patches() {
        // Bicubic, with knot 0.5 inserted at multiplicity 1 in both
        // directions. Control net is 5×5.
        let cps: Vec<Point3> = (0..5)
            .flat_map(|i| {
                (0..5).map(move |j| {
                    let u = i as f64 / 4.0;
                    let v = j as f64 / 4.0;
                    Point3::new(u, v, (u - 0.5).powi(2) + (v - 0.5).powi(2))
                })
            })
            .collect();
        let weights = vec![1.0; 25];
        let knots_u = vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0];
        let knots_v = vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0];
        let s = NurbsSurface::new(cps, weights, knots_u, knots_v, 3, 3, 5, 5).unwrap();

        let patches = nurbs_to_bezier_patches(&s);
        assert_eq!(patches.len(), 4, "expected 4 patches (2 u-spans × 2 v-spans), got {}", patches.len());

        // Each patch should evaluate to the same point as the source
        // NURBS (after local→global parameter translation).
        for patch in &patches {
            for &lu in &[0.1, 0.5, 0.9] {
                for &lv in &[0.1, 0.5, 0.9] {
                    let (gu, gv) = patch.local_to_global(lu, lv);
                    let p_nurbs = s.point_at(gu, gv);
                    let p_patch = patch.eval_f64(lu, lv);
                    assert!(approx_pt(p_nurbs, p_patch, 1e-9),
                        "NURBS({gu}, {gv}) = {p_nurbs:?} vs patch local ({lu}, {lv}) = {p_patch:?}");
                }
            }
        }
    }
}
