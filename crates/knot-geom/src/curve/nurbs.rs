use std::sync::Arc;
use knot_core::{Aabb3, ErrorCode, KResult, KernelError};
use crate::point::{Point3, Vector3};
use super::{CurveClosestPoint, CurveDerivatives, CurveDomain, CurveParam};

/// A NURBS curve. Immutable after construction.
/// Uses Arc for structural sharing of control data.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NurbsCurve {
    #[serde(with = "knot_core::serde_arc::arc_slice")]
    control_points: Arc<[Point3]>,
    #[serde(with = "knot_core::serde_arc::arc_slice")]
    weights: Arc<[f64]>,
    #[serde(with = "knot_core::serde_arc::arc_slice")]
    knots: Arc<[f64]>,
    degree: u32,
}

impl NurbsCurve {
    /// Validated constructor.
    pub fn new(
        control_points: Vec<Point3>,
        weights: Vec<f64>,
        knots: Vec<f64>,
        degree: u32,
    ) -> KResult<Self> {
        let n = control_points.len();
        let p = degree as usize;

        if n < p + 1 {
            return Err(KernelError::InvalidGeometry {
                code: ErrorCode::InsufficientControlPoints,
                detail: format!("need at least {} control points for degree {}, got {}", p + 1, degree, n),
            });
        }

        if weights.len() != n {
            return Err(KernelError::InvalidGeometry {
                code: ErrorCode::InsufficientControlPoints,
                detail: format!("weights count ({}) != control points count ({})", weights.len(), n),
            });
        }

        if knots.len() != n + p + 1 {
            return Err(KernelError::InvalidGeometry {
                code: ErrorCode::InvalidKnotVector,
                detail: format!("knots count ({}) != n + p + 1 ({})", knots.len(), n + p + 1),
            });
        }

        if weights.iter().any(|&w| w <= 0.0) {
            return Err(KernelError::InvalidGeometry {
                code: ErrorCode::NegativeWeight,
                detail: "all weights must be positive".into(),
            });
        }

        // Verify knot vector is non-decreasing
        for i in 1..knots.len() {
            if knots[i] < knots[i - 1] {
                return Err(KernelError::InvalidGeometry {
                    code: ErrorCode::InvalidKnotVector,
                    detail: format!("knot vector not non-decreasing at index {}", i),
                });
            }
        }

        Ok(Self {
            control_points: control_points.into(),
            weights: weights.into(),
            knots: knots.into(),
            degree,
        })
    }

    pub fn degree(&self) -> u32 {
        self.degree
    }

    pub fn control_points(&self) -> &[Point3] {
        &self.control_points
    }

    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    pub fn domain(&self) -> CurveDomain {
        let p = self.degree as usize;
        CurveDomain {
            start: self.knots[p],
            end: self.knots[self.control_points.len()],
        }
    }

    /// Evaluate a point on the curve at parameter t using de Boor's algorithm.
    pub fn point_at(&self, t: f64) -> Point3 {
        let p = self.degree as usize;
        let span = self.find_span(t);
        let basis = self.basis_functions(span, t);

        let mut point = [0.0; 3];
        let mut w_sum = 0.0;

        for i in 0..=p {
            let idx = span - p + i;
            let w = self.weights[idx];
            let bw = basis[i] * w;
            point[0] += bw * self.control_points[idx].x;
            point[1] += bw * self.control_points[idx].y;
            point[2] += bw * self.control_points[idx].z;
            w_sum += bw;
        }

        Point3::new(point[0] / w_sum, point[1] / w_sum, point[2] / w_sum)
    }

    /// Evaluate the curve and its derivatives up to the given order in homogeneous space,
    /// then convert to Cartesian. This is the proper NURBS derivative formula.
    pub fn derivatives_at(&self, t: f64) -> CurveDerivatives {
        let p = self.degree as usize;
        let span = self.find_span(t);
        let basis = self.basis_functions(span, t);
        let dbasis = self.basis_function_derivatives(span, t, 2);

        // Evaluate weighted point and derivatives: A(t) = sum(N_i * w_i * P_i), w(t) = sum(N_i * w_i)
        let mut a = [[0.0; 3]; 3]; // A, dA, d2A
        let mut w = [0.0; 3]; // w, dw, d2w

        for k in 0..3usize.min(dbasis.len()) {
            for i in 0..=p {
                let idx = span - p + i;
                let wi = self.weights[idx];
                let bw = dbasis[k][i] * wi;
                a[k][0] += bw * self.control_points[idx].x;
                a[k][1] += bw * self.control_points[idx].y;
                a[k][2] += bw * self.control_points[idx].z;
                w[k] += bw;
            }
        }

        // C(t) = A(t) / w(t)
        let point = Point3::new(a[0][0] / w[0], a[0][1] / w[0], a[0][2] / w[0]);

        // C'(t) = (A' - w' * C) / w
        let d1 = Vector3::new(
            (a[1][0] - w[1] * point.x) / w[0],
            (a[1][1] - w[1] * point.y) / w[0],
            (a[1][2] - w[1] * point.z) / w[0],
        );

        // C''(t) = (A'' - 2*w'*C' - w''*C) / w
        let d2 = Vector3::new(
            (a[2][0] - 2.0 * w[1] * d1.x - w[2] * point.x) / w[0],
            (a[2][1] - 2.0 * w[1] * d1.y - w[2] * point.y) / w[0],
            (a[2][2] - 2.0 * w[1] * d1.z - w[2] * point.z) / w[0],
        );

        CurveDerivatives { point, d1, d2: Some(d2) }
    }

    /// Closest point on the curve to a query point.
    /// Uses Newton iteration with sampled starting points.
    pub fn closest_point(&self, query: &Point3, num_samples: usize) -> CurveClosestPoint {
        let domain = self.domain();
        let dt = (domain.end - domain.start) / num_samples as f64;

        // Sample to find best initial guess
        let mut best_t = domain.start;
        let mut best_dist_sq = f64::MAX;

        for i in 0..=num_samples {
            let t = domain.start + dt * i as f64;
            let p = self.point_at(t);
            let d_sq = (p - query).norm_squared();
            if d_sq < best_dist_sq {
                best_dist_sq = d_sq;
                best_t = t;
            }
        }

        // Newton iteration to refine
        for _ in 0..20 {
            let derivs = self.derivatives_at(best_t);
            let diff = derivs.point - query;
            // f(t) = (C(t) - Q) . C'(t) = 0
            let f = diff.dot(&derivs.d1);
            // f'(t) = C'(t).C'(t) + (C(t)-Q).C''(t)
            let d2 = derivs.d2.unwrap_or(Vector3::zeros());
            let fp = derivs.d1.norm_squared() + diff.dot(&d2);
            if fp.abs() < 1e-30 {
                break;
            }
            let dt_step = f / fp;
            best_t -= dt_step;
            best_t = best_t.clamp(domain.start, domain.end);
            if dt_step.abs() < 1e-14 {
                break;
            }
        }

        let point = self.point_at(best_t);
        CurveClosestPoint {
            param: CurveParam(best_t),
            point,
            distance: (point - query).norm(),
        }
    }

    /// Split the curve at parameter t into two curves.
    pub fn split_at(&self, t: f64) -> (NurbsCurve, NurbsCurve) {
        let p = self.degree as usize;

        // Insert knot t until multiplicity equals degree
        let mut working = self.clone();
        let mut mult = 0;
        for &k in working.knots.iter() {
            if (k - t).abs() < 1e-15 {
                mult += 1;
            }
        }
        for _ in mult..p {
            working = working.insert_knot(t);
        }

        // Find the split index
        let split_idx = working.find_span(t);

        // Left curve: control points 0..=split_idx, knots 0..=split_idx+1 with clamped end
        let left_pts: Vec<Point3> = working.control_points[..=split_idx].to_vec();
        let left_w: Vec<f64> = working.weights[..=split_idx].to_vec();
        let n_left = left_pts.len();
        let mut left_knots: Vec<f64> = working.knots[..=split_idx].to_vec();
        // Add p+1 copies of t at the end to clamp
        for _ in 0..=p {
            left_knots.push(t);
        }
        // Trim to correct count
        left_knots.truncate(n_left + p + 1);

        // Right curve: control points split_idx-p+1.., knots from split_idx-p+1..
        let right_start = split_idx - p + 1;
        let right_pts: Vec<Point3> = working.control_points[right_start..].to_vec();
        let right_w: Vec<f64> = working.weights[right_start..].to_vec();
        let n_right = right_pts.len();
        let mut right_knots = vec![t; p + 1];
        right_knots.extend_from_slice(&working.knots[split_idx + 1..]);
        right_knots.truncate(n_right + p + 1);

        (
            NurbsCurve {
                control_points: left_pts.into(),
                weights: left_w.into(),
                knots: left_knots.into(),
                degree: self.degree,
            },
            NurbsCurve {
                control_points: right_pts.into(),
                weights: right_w.into(),
                knots: right_knots.into(),
                degree: self.degree,
            },
        )
    }

    /// Approximate arc length using Gaussian quadrature over sampled intervals.
    pub fn length(&self, tolerance: f64) -> f64 {
        let domain = self.domain();
        let n = ((domain.end - domain.start) / tolerance).ceil().max(8.0) as usize;
        let n = n.min(1000);
        let dt = (domain.end - domain.start) / n as f64;
        let mut total = 0.0;
        for i in 0..n {
            let t0 = domain.start + i as f64 * dt;
            let t1 = t0 + dt;
            // Simpson's rule
            let p0 = self.point_at(t0);
            let pm = self.point_at(0.5 * (t0 + t1));
            let p1 = self.point_at(t1);
            let l01 = (pm - p0).norm() + (p1 - pm).norm();
            total += l01;
        }
        total
    }

    /// Reverse the curve parameterization direction.
    pub fn reverse(&self) -> NurbsCurve {
        let mut pts: Vec<Point3> = self.control_points.to_vec();
        pts.reverse();
        let mut wts: Vec<f64> = self.weights.to_vec();
        wts.reverse();
        let domain_end = *self.knots.last().unwrap();
        let domain_start = self.knots[0];
        let knots: Vec<f64> = self.knots.iter().rev()
            .map(|&k| domain_start + domain_end - k)
            .collect();
        NurbsCurve {
            control_points: pts.into(),
            weights: wts.into(),
            knots: knots.into(),
            degree: self.degree,
        }
    }

    /// Compute bounding box from control points (conservative).
    pub fn bounding_box(&self) -> Aabb3 {
        Aabb3::from_points(&self.control_points).unwrap()
    }

    /// Insert a knot, returning a new curve.
    pub fn insert_knot(&self, t: f64) -> NurbsCurve {
        let p = self.degree as usize;
        let span = self.find_span(t);
        let n = self.control_points.len();

        let mut new_knots = Vec::with_capacity(self.knots.len() + 1);
        new_knots.extend_from_slice(&self.knots[..=span]);
        new_knots.push(t);
        new_knots.extend_from_slice(&self.knots[span + 1..]);

        let mut new_pts = Vec::with_capacity(n + 1);
        let mut new_weights = Vec::with_capacity(n + 1);

        for i in 0..=n {
            if i <= span - p {
                new_pts.push(self.control_points[i]);
                new_weights.push(self.weights[i]);
            } else if i > span {
                new_pts.push(self.control_points[i - 1]);
                new_weights.push(self.weights[i - 1]);
            } else {
                let alpha = (t - self.knots[i]) / (self.knots[i + p] - self.knots[i]);
                let w0 = self.weights[i - 1];
                let w1 = self.weights[i];
                let new_w = (1.0 - alpha) * w0 + alpha * w1;
                let pt = Point3::new(
                    ((1.0 - alpha) * w0 * self.control_points[i - 1].x
                        + alpha * w1 * self.control_points[i].x)
                        / new_w,
                    ((1.0 - alpha) * w0 * self.control_points[i - 1].y
                        + alpha * w1 * self.control_points[i].y)
                        / new_w,
                    ((1.0 - alpha) * w0 * self.control_points[i - 1].z
                        + alpha * w1 * self.control_points[i].z)
                        / new_w,
                );
                new_pts.push(pt);
                new_weights.push(new_w);
            }
        }

        NurbsCurve {
            control_points: new_pts.into(),
            weights: new_weights.into(),
            knots: new_knots.into(),
            degree: self.degree,
        }
    }

    fn find_span(&self, t: f64) -> usize {
        let n = self.control_points.len() - 1;
        let p = self.degree as usize;

        if t >= self.knots[n + 1] {
            return n;
        }
        if t <= self.knots[p] {
            return p;
        }

        let mut low = p;
        let mut high = n + 1;
        let mut mid = (low + high) / 2;

        while t < self.knots[mid] || t >= self.knots[mid + 1] {
            if t < self.knots[mid] {
                high = mid;
            } else {
                low = mid;
            }
            mid = (low + high) / 2;
        }

        mid
    }

    /// Compute basis function derivatives up to order `order`.
    /// Returns a Vec of Vec: result[k][i] = k-th derivative of basis function i.
    fn basis_function_derivatives(&self, span: usize, t: f64, order: usize) -> Vec<Vec<f64>> {
        let p = self.degree as usize;
        let knots = &self.knots;

        // ndu[j][i] stores basis function values and knot differences
        let mut ndu = vec![vec![0.0; p + 1]; p + 1];
        ndu[0][0] = 1.0;

        let mut left = vec![0.0; p + 1];
        let mut right = vec![0.0; p + 1];

        for j in 1..=p {
            left[j] = t - knots[span + 1 - j];
            right[j] = knots[span + j] - t;
            let mut saved = 0.0;
            for r in 0..j {
                // Lower triangle
                ndu[j][r] = right[r + 1] + left[j - r];
                let temp = ndu[r][j - 1] / ndu[j][r];
                // Upper triangle
                ndu[r][j] = saved + right[r + 1] * temp;
                saved = left[j - r] * temp;
            }
            ndu[j][j] = saved;
        }

        // Load basis functions
        let mut ders = vec![vec![0.0; p + 1]; order + 1];
        for i in 0..=p {
            ders[0][i] = ndu[i][p];
        }

        // Compute derivatives
        let mut a = vec![vec![0.0; p + 1]; 2];
        for r in 0..=p {
            let mut s1 = 0usize;
            let mut s2 = 1usize;
            a[0][0] = 1.0;

            for k in 1..=order.min(p) {
                let mut d = 0.0;
                let rk = r as isize - k as isize;
                let pk = (p as isize - k as isize) as usize;

                if rk >= 0 {
                    a[s2][0] = a[s1][0] / ndu[pk + 1][rk as usize];
                    d = a[s2][0] * ndu[rk as usize][pk];
                }

                let j1 = if rk >= -1 { 1 } else { (-rk) as usize };
                let j2 = if (r as isize - 1) <= pk as isize {
                    k - 1
                } else {
                    p - r
                };

                for j in j1..=j2 {
                    a[s2][j] = (a[s1][j] - a[s1][j - 1]) / ndu[pk + 1][(rk + j as isize) as usize];
                    d += a[s2][j] * ndu[(rk + j as isize) as usize][pk];
                }

                if r <= pk {
                    a[s2][k] = -a[s1][k - 1] / ndu[pk + 1][r];
                    d += a[s2][k] * ndu[r][pk];
                }

                ders[k][r] = d;
                std::mem::swap(&mut s1, &mut s2);
            }
        }

        // Multiply through by the correct factors
        let mut fac = p as f64;
        for k in 1..=order.min(p) {
            for i in 0..=p {
                ders[k][i] *= fac;
            }
            fac *= (p - k) as f64;
        }

        ders
    }

    fn basis_functions(&self, span: usize, t: f64) -> Vec<f64> {
        let p = self.degree as usize;
        let knots = &self.knots;
        let mut n = vec![0.0; p + 1];
        let mut left = vec![0.0; p + 1];
        let mut right = vec![0.0; p + 1];

        n[0] = 1.0;

        for j in 1..=p {
            left[j] = t - knots[span + 1 - j];
            right[j] = knots[span + j] - t;
            let mut saved = 0.0;

            for r in 0..j {
                let temp = n[r] / (right[r + 1] + left[j - r]);
                n[r] = saved + right[r + 1] * temp;
                saved = left[j - r] * temp;
            }

            n[j] = saved;
        }

        n
    }
}
