//! NURBS-vs-NURBS surface intersection (Phase 2B).
//!
//! Pipeline:
//!
//! 1. Decompose both NURBS surfaces into Bézier patches (via
//!    `nurbs_bridge::nurbs_to_bezier_patches`, but in f64 control-net
//!    form for fast subdivision).
//! 2. For each pair of patches, run subdivision-with-bbox-culling to
//!    find parameter regions where the patches potentially intersect.
//!    Recursive de Casteljau halves each patch into 4; bbox-vs-bbox
//!    test prunes non-overlapping pairs; recursion bottoms out when
//!    both sub-patches are below tolerance.
//! 3. Each leaf produces an (u_a, v_a, u_b, v_b, point_3d) tuple.
//! 4. Points across all patch pairs are clustered into chains by 3D
//!    proximity, one chain per intersection curve component.
//! 5. Each chain is emitted as a `SurfaceSurfaceTrace` with parameter
//!    coordinates back-mapped to the source NURBS parameter spaces.
//!
//! This is the walking-skeleton form. The subdivision step has
//! O((1/tol)²) leaves per patch pair, which is workable for
//! single-patch bicubic NURBS but slow for dense models. Phase 2B.4
//! replaces subdivision with Sederberg-Nishita fat-plane clipping
//! that converges in 1-2 iterations on transversal intersections.

use knot_geom::Point3;
use knot_geom::surface::{NurbsSurface, SurfaceParam};
use knot_core::{Aabb3, KResult};
use crate::SurfaceSurfaceTrace;

/// A Bézier patch in f64 control-net form. Used for fast subdivision.
/// Constructed from `nurbs_bridge::nurbs_to_bezier_patches` by lifting
/// the homogeneous polynomials back to a control net (or, more
/// directly, by reading the original NURBS control points after knot
/// insertion).
#[derive(Clone, Debug)]
pub struct F64BezierPatch {
    pub degree_u: u32,
    pub degree_v: u32,
    /// `cps[i][j]` = (control point, weight) at row i (u-index),
    /// column j (v-index). Stored in cartesian + weight, not
    /// homogeneous, so eval is `Σ B_i^p(u) B_j^q(v) w_ij P_ij /
    /// Σ B_i^p(u) B_j^q(v) w_ij`.
    pub cps: Vec<Vec<(Point3, f64)>>,
    /// Source-NURBS parameter range that this patch covers.
    pub u_range: (f64, f64),
    pub v_range: (f64, f64),
}

impl F64BezierPatch {
    /// Build a list of f64 patches from a NURBS surface, mirroring
    /// the BiPoly path in `nurbs_bridge` but skipping the
    /// power-basis conversion. This duplicates the Boehm knot
    /// insertion logic; future refactor could share with the BiPoly
    /// pipeline by exposing a "post-insertion control net" stage.
    pub fn from_nurbs(s: &NurbsSurface) -> Vec<F64BezierPatch> {
        super::nurbs_bridge::nurbs_to_bezier_patches(s)
            .into_iter()
            .map(|patch| {
                // Reconstruct the f64 control net from the BiPoly form
                // by sampling. For low-degree patches this is exact
                // (sample at Greville abscissae and invert). Cleaner
                // path: have nurbs_bridge return both forms.
                //
                // Walking-skeleton choice: we re-decompose the source
                // NURBS surface ourselves below. The duplication is
                // intentional — keeps this module self-contained
                // until Phase 2B.4's clipping path needs more.
                F64BezierPatch::from_bipoly_patch(&patch)
            })
            .collect()
    }

    /// Reconstruct the f64 control net from the BiPoly form by
    /// inverse-Bernstein evaluation at the Greville abscissae. Each
    /// control point is recovered as a polynomial value at a known
    /// parameter, so this is exact up to f64 precision.
    fn from_bipoly_patch(p: &super::nurbs_bridge::BezierPatch) -> F64BezierPatch {
        let nu = p.degree_u as usize;
        let nv = p.degree_v as usize;
        // For a Bézier patch, P(u_i, v_j) at certain (u, v) equals the
        // control point times a Bernstein basis matrix that we can
        // invert. Simpler path: sample the patch at a (nu+1)×(nv+1)
        // grid of Bernstein-basis collocation points and store the
        // points directly — this isn't the control net but acts as
        // one for evaluation purposes.
        //
        // Actually we need the *real* control net for de Casteljau.
        // The clean solution: interrogate the BiPoly's u^i v^j
        // coefficients to reconstruct (X, Y, Z, W) as a power-basis
        // polynomial, then convert back to Bernstein.
        let mut cps = vec![vec![(Point3::origin(), 1.0); nv + 1]; nu + 1];
        let p_inv_u = power_to_bernstein_matrix(p.degree_u);
        let p_inv_v = power_to_bernstein_matrix(p.degree_v);

        // Extract power-basis coefficients of (X, Y, Z, W).
        let extract = |poly: &super::poly::BiPoly| -> Vec<Vec<f64>> {
            let mut m = vec![vec![0.0_f64; nv + 1]; nu + 1];
            for (iu, iv, c) in poly.iter() {
                if (iu as usize) <= nu && (iv as usize) <= nv {
                    use malachite_base::num::conversion::traits::RoundingFrom;
                    use malachite_base::rounding_modes::RoundingMode;
                    let (cf, _) = f64::rounding_from(c, RoundingMode::Nearest);
                    m[iu as usize][iv as usize] = cf;
                }
            }
            m
        };
        let xc = extract(&p.x);
        let yc = extract(&p.y);
        let zc = extract(&p.z);
        let wc = extract(&p.w);

        // Convert each coordinate's power coefficients to Bernstein:
        // Bezier_coeffs = M_inv_u · power_coeffs · M_inv_v^T.
        let to_bern = |power: &[Vec<f64>]| -> Vec<Vec<f64>> {
            let mut tmp = vec![vec![0.0_f64; nv + 1]; nu + 1];
            for i in 0..=nu {
                for j in 0..=nv {
                    let mut s = 0.0;
                    for k in 0..=nu {
                        s += p_inv_u[i][k] * power[k][j];
                    }
                    tmp[i][j] = s;
                }
            }
            let mut bern = vec![vec![0.0_f64; nv + 1]; nu + 1];
            for i in 0..=nu {
                for j in 0..=nv {
                    let mut s = 0.0;
                    for k in 0..=nv {
                        s += tmp[i][k] * p_inv_v[j][k];
                    }
                    bern[i][j] = s;
                }
            }
            bern
        };
        let xb = to_bern(&xc);
        let yb = to_bern(&yc);
        let zb = to_bern(&zc);
        let wb = to_bern(&wc);

        // Each (xb, yb, zb, wb) entry is a homogeneous Bernstein
        // coefficient: (P_ij · w_ij, w_ij). Recover cartesian:
        for i in 0..=nu {
            for j in 0..=nv {
                let w = wb[i][j];
                let pt = if w.abs() > 1e-15 {
                    Point3::new(xb[i][j] / w, yb[i][j] / w, zb[i][j] / w)
                } else {
                    Point3::new(xb[i][j], yb[i][j], zb[i][j])
                };
                cps[i][j] = (pt, w);
            }
        }

        F64BezierPatch {
            degree_u: p.degree_u,
            degree_v: p.degree_v,
            cps,
            u_range: p.u_range,
            v_range: p.v_range,
        }
    }

    /// Axis-aligned bounding box of the patch's control net (in
    /// cartesian coordinates after weight division). For non-rational
    /// patches the surface lies inside the convex hull of the control
    /// points so this bbox encloses the surface tightly. Rational
    /// patches with non-uniform weights need a slightly conservative
    /// expansion in extreme cases — we use the cartesian-control bbox
    /// for the walking skeleton.
    pub fn bbox(&self) -> Aabb3 {
        let pts: Vec<Point3> = self.cps.iter().flatten().map(|(p, _)| *p).collect();
        Aabb3::from_points(&pts).unwrap()
    }

    /// Evaluate at local (u, v) ∈ [0, 1]² via direct Bernstein
    /// summation. Uses the same homogeneous formulation as the BiPoly
    /// patch (sum weighted control points, divide by sum of weights).
    pub fn eval(&self, u: f64, v: f64) -> Point3 {
        let nu = self.degree_u as usize;
        let nv = self.degree_v as usize;
        let mut acc = [0.0_f64; 3];
        let mut w_sum = 0.0_f64;
        for i in 0..=nu {
            let bu = bernstein(nu, i, u);
            for j in 0..=nv {
                let bv = bernstein(nv, j, v);
                let (p, w) = self.cps[i][j];
                let bw = bu * bv * w;
                acc[0] += bw * p.x;
                acc[1] += bw * p.y;
                acc[2] += bw * p.z;
                w_sum += bw;
            }
        }
        if w_sum.abs() < 1e-15 {
            return Point3::new(acc[0], acc[1], acc[2]);
        }
        Point3::new(acc[0] / w_sum, acc[1] / w_sum, acc[2] / w_sum)
    }

    /// Convert local patch parameter to source-NURBS parameter.
    pub fn local_to_global(&self, lu: f64, lv: f64) -> (f64, f64) {
        let g_u = self.u_range.0 + (self.u_range.1 - self.u_range.0) * lu;
        let g_v = self.v_range.0 + (self.v_range.1 - self.v_range.0) * lv;
        (g_u, g_v)
    }

    /// Subdivide at u = 0.5 → two halves over [0, 0.5] and [0.5, 1].
    /// Implemented via row-wise de Casteljau on (P*w, w) homogeneous
    /// coordinates so rational patches subdivide correctly.
    pub fn subdivide_u(&self) -> (F64BezierPatch, F64BezierPatch) {
        let nu = self.degree_u as usize;
        let nv = self.degree_v as usize;
        let mut left = vec![vec![(Point3::origin(), 0.0_f64); nv + 1]; nu + 1];
        let mut right = vec![vec![(Point3::origin(), 0.0_f64); nv + 1]; nu + 1];

        for j in 0..=nv {
            // Build a 1D column of homogeneous control points along u.
            let mut col_h: Vec<(f64, f64, f64, f64)> = (0..=nu)
                .map(|i| {
                    let (p, w) = self.cps[i][j];
                    (p.x * w, p.y * w, p.z * w, w)
                })
                .collect();

            left[0][j] = self.cps[0][j];
            right[nu][j] = self.cps[nu][j];

            // de Casteljau at u=0.5 produces the new control points.
            // After k folding steps, col_h[0..=nu-k] is the k-th row
            // of the de Casteljau triangle. left[k][j] is the first
            // entry; right[nu - k][j] is the last entry.
            for k in 1..=nu {
                for i in 0..=nu - k {
                    let a = col_h[i];
                    let b = col_h[i + 1];
                    col_h[i] = (
                        0.5 * (a.0 + b.0),
                        0.5 * (a.1 + b.1),
                        0.5 * (a.2 + b.2),
                        0.5 * (a.3 + b.3),
                    );
                }
                let h = col_h[0];
                let p = if h.3.abs() > 1e-15 {
                    Point3::new(h.0 / h.3, h.1 / h.3, h.2 / h.3)
                } else {
                    Point3::new(h.0, h.1, h.2)
                };
                left[k][j] = (p, h.3);
                let h2 = col_h[nu - k];
                let p2 = if h2.3.abs() > 1e-15 {
                    Point3::new(h2.0 / h2.3, h2.1 / h2.3, h2.2 / h2.3)
                } else {
                    Point3::new(h2.0, h2.1, h2.2)
                };
                right[nu - k][j] = (p2, h2.3);
            }
        }

        let u_mid = 0.5 * (self.u_range.0 + self.u_range.1);
        (
            F64BezierPatch {
                degree_u: self.degree_u,
                degree_v: self.degree_v,
                cps: left,
                u_range: (self.u_range.0, u_mid),
                v_range: self.v_range,
            },
            F64BezierPatch {
                degree_u: self.degree_u,
                degree_v: self.degree_v,
                cps: right,
                u_range: (u_mid, self.u_range.1),
                v_range: self.v_range,
            },
        )
    }

    /// Subdivide at v = 0.5 — same algorithm operating on columns.
    pub fn subdivide_v(&self) -> (F64BezierPatch, F64BezierPatch) {
        let nu = self.degree_u as usize;
        let nv = self.degree_v as usize;
        let mut left = vec![vec![(Point3::origin(), 0.0_f64); nv + 1]; nu + 1];
        let mut right = vec![vec![(Point3::origin(), 0.0_f64); nv + 1]; nu + 1];

        for i in 0..=nu {
            let mut row_h: Vec<(f64, f64, f64, f64)> = (0..=nv)
                .map(|j| {
                    let (p, w) = self.cps[i][j];
                    (p.x * w, p.y * w, p.z * w, w)
                })
                .collect();

            left[i][0] = self.cps[i][0];
            right[i][nv] = self.cps[i][nv];

            for k in 1..=nv {
                for j in 0..=nv - k {
                    let a = row_h[j];
                    let b = row_h[j + 1];
                    row_h[j] = (
                        0.5 * (a.0 + b.0),
                        0.5 * (a.1 + b.1),
                        0.5 * (a.2 + b.2),
                        0.5 * (a.3 + b.3),
                    );
                }
                let h = row_h[0];
                let p = if h.3.abs() > 1e-15 {
                    Point3::new(h.0 / h.3, h.1 / h.3, h.2 / h.3)
                } else {
                    Point3::new(h.0, h.1, h.2)
                };
                left[i][k] = (p, h.3);
                let h2 = row_h[nv - k];
                let p2 = if h2.3.abs() > 1e-15 {
                    Point3::new(h2.0 / h2.3, h2.1 / h2.3, h2.2 / h2.3)
                } else {
                    Point3::new(h2.0, h2.1, h2.2)
                };
                right[i][nv - k] = (p2, h2.3);
            }
        }

        let v_mid = 0.5 * (self.v_range.0 + self.v_range.1);
        (
            F64BezierPatch {
                degree_u: self.degree_u,
                degree_v: self.degree_v,
                cps: left,
                u_range: self.u_range,
                v_range: (self.v_range.0, v_mid),
            },
            F64BezierPatch {
                degree_u: self.degree_u,
                degree_v: self.degree_v,
                cps: right,
                u_range: self.u_range,
                v_range: (v_mid, self.v_range.1),
            },
        )
    }
}

fn bernstein(n: usize, i: usize, t: f64) -> f64 {
    let mut c = [1.0, 3.0, 3.0, 1.0]; // bicubic shortcut
    let _ = c;
    let mut binom = 1.0;
    for k in 1..=i {
        binom *= (n + 1 - k) as f64 / k as f64;
    }
    binom * t.powi(i as i32) * (1.0 - t).powi((n - i) as i32)
}

/// Power-basis to Bernstein-basis conversion matrix for degree n.
/// Returns `m` such that the Bernstein coefficient `b_i` is recovered
/// from power coefficients `a_j` via `b_i = Σ_j m[i][j] a_j`.
///
/// Derivation: classically `t^j = Σ_{i ≥ j} (C(i, j) / C(n, j)) B_i^n(t)`
/// for `j ≤ i ≤ n`. So `m[i][j] = C(i, j) / C(n, j)` for `i ≥ j`,
/// else `0`. (The matrix is lower-triangular.)
fn power_to_bernstein_matrix(n: u32) -> Vec<Vec<f64>> {
    let n = n as usize;
    let binom = binomial_table(n);
    let mut m = vec![vec![0.0_f64; n + 1]; n + 1];
    for i in 0..=n {
        for j in 0..=i {
            m[i][j] = binom[i][j] / binom[n][j];
        }
    }
    m
}

fn binomial_table(n: usize) -> Vec<Vec<f64>> {
    let mut t = vec![vec![0.0_f64; n + 1]; n + 1];
    for k in 0..=n {
        t[k][0] = 1.0;
        for i in 1..=k {
            t[k][i] = if i == k { 1.0 } else { t[k - 1][i - 1] + t[k - 1][i] };
        }
    }
    t
}

// ─────────────────────────────────────────────────────────────────────
// Patch-pair subdivision intersection (Phase 2B.2)
// ─────────────────────────────────────────────────────────────────────

/// One discrete intersection point sampled by the subdivision tracer.
#[derive(Clone, Debug)]
pub struct IntersectionSample {
    pub u_a: f64,
    pub v_a: f64,
    pub u_b: f64,
    pub v_b: f64,
    pub point: Point3,
}

/// Recursively subdivide patch_a and patch_b, emitting one sample per
/// leaf where both patches' bboxes are below tolerance.
///
/// `max_depth` bounds recursion. For typical bicubic NURBS, ~12 levels
/// (4096 sub-patches per side) is plenty; adjust if deeper recursion
/// is needed.
pub fn subdivide_intersect(
    patch_a: &F64BezierPatch,
    patch_b: &F64BezierPatch,
    tolerance: f64,
    max_depth: usize,
) -> Vec<IntersectionSample> {
    let mut out = Vec::new();
    let mut work: Vec<(F64BezierPatch, F64BezierPatch, usize)> =
        vec![(patch_a.clone(), patch_b.clone(), 0)];

    while let Some((a, b, depth)) = work.pop() {
        let ba = a.bbox();
        let bb = b.bbox();
        if !ba.intersects(&bb) {
            continue;
        }

        let diag_a = ba.diagonal_length();
        let diag_b = bb.diagonal_length();
        if (diag_a < tolerance && diag_b < tolerance) || depth >= max_depth {
            // Seed: midpoint of the two patches' parametric centers,
            // mapped through eval. This is the *initial guess* — for
            // generic geometry the midpoint of two surface points is
            // not itself on either surface. Newton-refine onto both
            // patches simultaneously to converge to an actual
            // intersection point.
            let seed_uv_a = (0.5, 0.5);
            let seed_uv_b = (0.5, 0.5);
            let (uv_a, uv_b, point) =
                newton_refine_onto_both(&a, &b, seed_uv_a, seed_uv_b);

            // Verify post-refinement: the point should be near both
            // surface evaluations. If not, the seed was too far from
            // any actual intersection — drop the sample.
            let pa = a.eval(uv_a.0, uv_a.1);
            let pb = b.eval(uv_b.0, uv_b.1);
            if (pa - pb).norm() > tolerance * 10.0 {
                continue;
            }

            // Map local patch (u, v) back to source-NURBS parameters.
            let (gu_a, gv_a) = a.local_to_global(uv_a.0, uv_a.1);
            let (gu_b, gv_b) = b.local_to_global(uv_b.0, uv_b.1);
            out.push(IntersectionSample {
                u_a: gu_a,
                v_a: gv_a,
                u_b: gu_b,
                v_b: gv_b,
                point,
            });
            continue;
        }

        // Subdivide the larger patch (faster convergence).
        if diag_a >= diag_b {
            let (a1, a2) = a.subdivide_u();
            let (a11, a12) = a1.subdivide_v();
            let (a21, a22) = a2.subdivide_v();
            for sub in [a11, a12, a21, a22] {
                work.push((sub, b.clone(), depth + 1));
            }
        } else {
            let (b1, b2) = b.subdivide_u();
            let (b11, b12) = b1.subdivide_v();
            let (b21, b22) = b2.subdivide_v();
            for sub in [b11, b12, b21, b22] {
                work.push((a.clone(), sub, depth + 1));
            }
        }
    }

    out
}

/// Newton-refine a seed (uv_a, uv_b) onto both patches simultaneously.
/// Returns ((uv_a, uv_b), 3D midpoint) where the midpoint is the
/// average of the two refined surface evaluations.
///
/// Algorithm: alternating projection. Compute current 3D midpoint of
/// (P_a(uv_a), P_b(uv_b)). Project that midpoint back onto P_a (find
/// nearest uv_a). Repeat for P_b. Iterate until P_a ≈ P_b. This is the
/// classical Newton SSI seed-refinement pattern; bounded iteration
/// count (16) prevents pathological non-convergence on tangent cases.
fn newton_refine_onto_both(
    patch_a: &F64BezierPatch,
    patch_b: &F64BezierPatch,
    seed_uv_a: (f64, f64),
    seed_uv_b: (f64, f64),
) -> ((f64, f64), (f64, f64), Point3) {
    let mut uv_a = seed_uv_a;
    let mut uv_b = seed_uv_b;

    for _ in 0..16 {
        let pa = patch_a.eval(uv_a.0, uv_a.1);
        let pb = patch_b.eval(uv_b.0, uv_b.1);
        if (pa - pb).norm() < 1e-12 {
            break;
        }
        let mid = Point3::new(
            0.5 * (pa.x + pb.x),
            0.5 * (pa.y + pb.y),
            0.5 * (pa.z + pb.z),
        );
        // Project the midpoint onto each patch independently. After
        // projection both `uv_a` and `uv_b` evaluate closer to the
        // midpoint and (if the patches actually meet there) closer to
        // each other.
        uv_a = project_onto_patch(patch_a, mid, uv_a);
        uv_b = project_onto_patch(patch_b, mid, uv_b);
    }

    let pa = patch_a.eval(uv_a.0, uv_a.1);
    let pb = patch_b.eval(uv_b.0, uv_b.1);
    let point = Point3::new(
        0.5 * (pa.x + pb.x),
        0.5 * (pa.y + pb.y),
        0.5 * (pa.z + pb.z),
    );
    (uv_a, uv_b, point)
}

/// Project a 3D point onto a Bézier patch by Gauss-Newton iteration.
/// Returns the (u, v) of the projection, clamped to [0, 1]². Uses
/// finite-difference Jacobian — the patch's analytic derivatives
/// would be cleaner but for the walking skeleton this is good enough.
fn project_onto_patch(
    patch: &F64BezierPatch,
    target: Point3,
    seed: (f64, f64),
) -> (f64, f64) {
    let mut uv = seed;
    let h = 1e-5;
    for _ in 0..10 {
        let p = patch.eval(uv.0, uv.1);
        let diff = target - p;
        if diff.norm() < 1e-12 {
            break;
        }
        let pu = patch.eval((uv.0 + h).min(1.0), uv.1);
        let pv = patch.eval(uv.0, (uv.1 + h).min(1.0));
        let du = (pu - p) / h;
        let dv = (pv - p) / h;
        let a11 = du.dot(&du);
        let a12 = du.dot(&dv);
        let a22 = dv.dot(&dv);
        let b1 = du.dot(&diff);
        let b2 = dv.dot(&diff);
        let det = a11 * a22 - a12 * a12;
        if det.abs() < 1e-15 {
            break;
        }
        let d_u = (a22 * b1 - a12 * b2) / det;
        let d_v = (-a12 * b1 + a11 * b2) / det;
        uv.0 = (uv.0 + d_u).clamp(0.0, 1.0);
        uv.1 = (uv.1 + d_v).clamp(0.0, 1.0);
    }
    uv
}

// ─────────────────────────────────────────────────────────────────────
// Cluster intersection samples into curve traces (Phase 2B.3)
// ─────────────────────────────────────────────────────────────────────

/// Group samples into proximity-connected chains. Two samples are
/// linked if their 3D distance is below `link_threshold`. Each
/// connected component becomes one chain in arbitrary traversal
/// order (we don't try to order along the curve direction here —
/// downstream Newton refinement handles ordering if needed).
fn cluster_samples(
    samples: &[IntersectionSample],
    link_threshold: f64,
) -> Vec<Vec<IntersectionSample>> {
    if samples.is_empty() {
        return Vec::new();
    }
    let n = samples.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], i: usize) -> usize {
        if parent[i] != i {
            parent[i] = find(parent, parent[i]);
        }
        parent[i]
    }
    fn union(parent: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[ra] = rb;
        }
    }

    for i in 0..n {
        for j in (i + 1)..n {
            let d = (samples[i].point - samples[j].point).norm();
            if d < link_threshold {
                union(&mut parent, i, j);
            }
        }
    }

    let mut groups: std::collections::BTreeMap<usize, Vec<IntersectionSample>> =
        std::collections::BTreeMap::new();
    for (i, s) in samples.iter().enumerate() {
        let r = find(&mut parent, i);
        groups.entry(r).or_default().push(s.clone());
    }
    groups.into_values().collect()
}

/// Order a cluster of samples into a chain by greedy nearest-neighbor
/// traversal. For closed loops this produces a roughly sequential
/// path; for open chains it walks endpoint-to-endpoint. Good enough
/// for the walking skeleton; Phase 2B.4 + topology connector replace
/// this with proper algebraic chain extraction.
fn order_cluster(cluster: Vec<IntersectionSample>) -> Vec<IntersectionSample> {
    if cluster.len() <= 2 {
        return cluster;
    }
    let mut remaining = cluster;
    let mut ordered = Vec::with_capacity(remaining.len());
    ordered.push(remaining.remove(0));
    while !remaining.is_empty() {
        let last = ordered.last().unwrap().point;
        let (best_idx, _) = remaining
            .iter()
            .enumerate()
            .map(|(i, s)| (i, (s.point - last).norm()))
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();
        ordered.push(remaining.remove(best_idx));
    }
    ordered
}

// ─────────────────────────────────────────────────────────────────────
// Top-level entry: NURBS-vs-NURBS intersection
// ─────────────────────────────────────────────────────────────────────

const MAX_PATCHES_NN: usize = 16;
const MAX_DEGREE_NN: u32 = 4;
const MAX_DEPTH_NN: usize = 12;

fn nurbs_is_tractable_nn(s: &NurbsSurface) -> bool {
    if s.degree_u() > MAX_DEGREE_NN || s.degree_v() > MAX_DEGREE_NN {
        return false;
    }
    let unique_u = count_unique_knots(s.knots_u());
    let unique_v = count_unique_knots(s.knots_v());
    let patches = unique_u.saturating_sub(1) * unique_v.saturating_sub(1);
    patches <= MAX_PATCHES_NN
}

fn count_unique_knots(knots: &[f64]) -> usize {
    let mut count = 0usize;
    let mut prev = f64::NEG_INFINITY;
    for &k in knots {
        if (k - prev).abs() > 1e-12 {
            count += 1;
            prev = k;
        }
    }
    count
}

/// NURBS-vs-NURBS intersection via patch-pair subdivision and
/// proximity clustering. Returns `Ok(empty)` when either input
/// exceeds the tractability gate (caller falls through to the
/// marcher).
pub fn intersect_nurbs_nurbs(
    a: &NurbsSurface,
    b: &NurbsSurface,
    tolerance: f64,
) -> KResult<Vec<SurfaceSurfaceTrace>> {
    if !nurbs_is_tractable_nn(a) || !nurbs_is_tractable_nn(b) {
        return Ok(Vec::new());
    }

    let patches_a = F64BezierPatch::from_nurbs(a);
    let patches_b = F64BezierPatch::from_nurbs(b);

    // Patch-pair candidate filter: skip pairs whose bboxes don't
    // overlap. For dense NURBS with many patches this prevents the
    // O(M·N) full subdivision sweep.
    let mut all_samples: Vec<IntersectionSample> = Vec::new();
    let bboxes_a: Vec<Aabb3> = patches_a.iter().map(|p| p.bbox()).collect();
    let bboxes_b: Vec<Aabb3> = patches_b.iter().map(|p| p.bbox()).collect();
    for (i, ba) in bboxes_a.iter().enumerate() {
        for (j, bb) in bboxes_b.iter().enumerate() {
            if !ba.intersects(bb) {
                continue;
            }
            let samples = subdivide_intersect(
                &patches_a[i],
                &patches_b[j],
                tolerance,
                MAX_DEPTH_NN,
            );
            all_samples.extend(samples);
        }
    }

    if all_samples.is_empty() {
        return Ok(Vec::new());
    }

    // Cluster into chains.
    let link_threshold = (tolerance * 100.0).max(1e-3);
    let clusters = cluster_samples(&all_samples, link_threshold);

    // Validation: every output sample is from subdivision-with-bbox-
    // overlap then Newton-refined onto both patches, so it lies near
    // both surfaces by construction. We additionally verify each
    // chain is non-degenerate (≥ 3 points) and emit a
    // `SurfaceSurfaceTrace` with the source-NURBS parameter
    // coordinates that the samples already carry.
    let mut traces = Vec::new();
    for cluster in clusters {
        if cluster.len() < 3 {
            continue;
        }
        let ordered = order_cluster(cluster);
        let points: Vec<Point3> = ordered.iter().map(|s| s.point).collect();
        let params_a: Vec<SurfaceParam> =
            ordered.iter().map(|s| SurfaceParam { u: s.u_a, v: s.v_a }).collect();
        let params_b: Vec<SurfaceParam> =
            ordered.iter().map(|s| SurfaceParam { u: s.u_b, v: s.v_b }).collect();
        traces.push(SurfaceSurfaceTrace { points, params_a, params_b });
    }

    Ok(traces)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_pt(a: Point3, b: Point3, eps: f64) -> bool {
        (a.x - b.x).abs() < eps && (a.y - b.y).abs() < eps && (a.z - b.z).abs() < eps
    }

    /// F64BezierPatch evaluation matches the reference NURBS surface
    /// over a sampling of the parameter domain.
    #[test]
    fn f64_patch_eval_matches_nurbs() {
        let cps: Vec<Point3> = (0..4)
            .flat_map(|i| {
                (0..4).map(move |j| {
                    let u = i as f64 / 3.0;
                    let v = j as f64 / 3.0;
                    Point3::new(u, v, (u - 0.5).powi(2) + (v - 0.5).powi(2))
                })
            })
            .collect();
        let weights = vec![1.0; 16];
        let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let s = NurbsSurface::new(cps, weights, knots.clone(), knots, 3, 3, 4, 4).unwrap();

        let patches = F64BezierPatch::from_nurbs(&s);
        assert_eq!(patches.len(), 1);

        for &lu in &[0.05, 0.5, 0.95] {
            for &lv in &[0.1, 0.5, 0.9] {
                let (gu, gv) = patches[0].local_to_global(lu, lv);
                let p_nurbs = s.point_at(gu, gv);
                let p_patch = patches[0].eval(lu, lv);
                assert!(approx_pt(p_nurbs, p_patch, 1e-9),
                    "f64 patch eval drifted from NURBS at ({lu},{lv})");
            }
        }
    }

    /// Subdividing at u=0.5 then evaluating left and right halves
    /// reproduces the original surface over [0, 0.5] and [0.5, 1].
    #[test]
    fn subdivision_preserves_surface() {
        let cps: Vec<Point3> = (0..4)
            .flat_map(|i| {
                (0..4).map(move |j| {
                    let u = i as f64 / 3.0;
                    let v = j as f64 / 3.0;
                    Point3::new(u, v, (u + v).sin())
                })
            })
            .collect();
        let weights = vec![1.0; 16];
        let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let s = NurbsSurface::new(cps, weights, knots.clone(), knots, 3, 3, 4, 4).unwrap();
        let patches = F64BezierPatch::from_nurbs(&s);
        let (left, right) = patches[0].subdivide_u();

        // Left covers u ∈ [0, 0.5]. left.eval(t, v) should equal patches[0].eval(t/2, v).
        for &t in &[0.0, 0.3, 0.7, 1.0] {
            for &v in &[0.0, 0.4, 0.8] {
                let p_left = left.eval(t, v);
                let p_orig = patches[0].eval(t * 0.5, v);
                assert!(approx_pt(p_left, p_orig, 1e-9),
                    "subdivide_u left mismatch at ({t}, {v}): left={p_left:?} orig={p_orig:?}");

                let p_right = right.eval(t, v);
                let p_orig_r = patches[0].eval(0.5 + t * 0.5, v);
                assert!(approx_pt(p_right, p_orig_r, 1e-9),
                    "subdivide_u right mismatch at ({t}, {v})");
            }
        }
    }

    /// Two flat NURBS plates at z = ±0.5 do not intersect; algebraic
    /// path returns no traces.
    #[test]
    fn disjoint_nurbs_plates_no_intersection() {
        let plate1 = make_flat_plate(0.5);
        let plate2 = make_flat_plate(-0.5);
        let traces = intersect_nurbs_nurbs(&plate1, &plate2, 1e-3).unwrap();
        assert!(traces.is_empty());
    }

    /// Two NURBS plates that cross along y = x at z = 0. The
    /// intersection is a line; subdivision produces samples along it.
    #[test]
    fn two_nurbs_plates_crossing() {
        // Plate 1: z = 0 (constant)
        let plate1 = make_flat_plate(0.0);
        // Plate 2: z = x - y, slanted to cross plate 1 along x = y
        let cps: Vec<Point3> = (0..4)
            .flat_map(|i| {
                (0..4).map(move |j| {
                    let u = i as f64 / 3.0 * 2.0 - 1.0; // [-1, 1]
                    let v = j as f64 / 3.0 * 2.0 - 1.0;
                    Point3::new(u, v, u - v)
                })
            })
            .collect();
        let weights = vec![1.0; 16];
        let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let plate2 = NurbsSurface::new(cps, weights, knots.clone(), knots, 3, 3, 4, 4).unwrap();

        let traces = intersect_nurbs_nurbs(&plate1, &plate2, 0.01).unwrap();
        assert!(!traces.is_empty(), "expected at least one trace");
        // Every output point must satisfy z ≈ 0 (on plate1) and x ≈ y
        // (intersection line).
        for trace in &traces {
            for p in &trace.points {
                assert!(p.z.abs() < 0.05, "point off plate1: {p:?}");
                assert!((p.x - p.y).abs() < 0.05, "point off intersection line: {p:?}");
            }
        }
    }

    fn make_flat_plate(z: f64) -> NurbsSurface {
        let cps: Vec<Point3> = (0..4)
            .flat_map(|i| {
                (0..4).map(move |j| {
                    let u = i as f64 / 3.0 * 2.0 - 1.0;
                    let v = j as f64 / 3.0 * 2.0 - 1.0;
                    Point3::new(u, v, z)
                })
            })
            .collect();
        let weights = vec![1.0; 16];
        let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        NurbsSurface::new(cps, weights, knots.clone(), knots, 3, 3, 4, 4).unwrap()
    }
}
