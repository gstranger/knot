use std::sync::Arc;
use knot_core::{Aabb3, ErrorCode, KResult, KernelError};
use crate::point::{Point3, Vector3};
use super::SurfaceDomain;

/// A NURBS surface. Immutable after construction.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NurbsSurface {
    #[serde(with = "knot_core::serde_arc::arc_slice")]
    control_points: Arc<[Point3]>,
    #[serde(with = "knot_core::serde_arc::arc_slice")]
    weights: Arc<[f64]>,
    #[serde(with = "knot_core::serde_arc::arc_slice")]
    knots_u: Arc<[f64]>,
    #[serde(with = "knot_core::serde_arc::arc_slice")]
    knots_v: Arc<[f64]>,
    degree_u: u32,
    degree_v: u32,
    count_u: u32,
    count_v: u32,
}

impl NurbsSurface {
    /// Validated constructor.
    pub fn new(
        control_points: Vec<Point3>,
        weights: Vec<f64>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        degree_u: u32,
        degree_v: u32,
        count_u: u32,
        count_v: u32,
    ) -> KResult<Self> {
        let n = (count_u * count_v) as usize;

        if control_points.len() != n {
            return Err(KernelError::InvalidGeometry {
                code: ErrorCode::InsufficientControlPoints,
                detail: format!(
                    "expected {} control points ({}x{}), got {}",
                    n, count_u, count_v, control_points.len()
                ),
            });
        }

        if weights.len() != n {
            return Err(KernelError::InvalidGeometry {
                code: ErrorCode::InsufficientControlPoints,
                detail: format!("weights count ({}) != control points count ({})", weights.len(), n),
            });
        }

        if knots_u.len() != (count_u + degree_u + 1) as usize {
            return Err(KernelError::InvalidGeometry {
                code: ErrorCode::InvalidKnotVector,
                detail: format!(
                    "knots_u count ({}) != count_u + degree_u + 1 ({})",
                    knots_u.len(),
                    count_u + degree_u + 1
                ),
            });
        }

        if knots_v.len() != (count_v + degree_v + 1) as usize {
            return Err(KernelError::InvalidGeometry {
                code: ErrorCode::InvalidKnotVector,
                detail: format!(
                    "knots_v count ({}) != count_v + degree_v + 1 ({})",
                    knots_v.len(),
                    count_v + degree_v + 1
                ),
            });
        }

        if weights.iter().any(|&w| w <= 0.0) {
            return Err(KernelError::InvalidGeometry {
                code: ErrorCode::NegativeWeight,
                detail: "all weights must be positive".into(),
            });
        }

        Ok(Self {
            control_points: control_points.into(),
            weights: weights.into(),
            knots_u: knots_u.into(),
            knots_v: knots_v.into(),
            degree_u,
            degree_v,
            count_u,
            count_v,
        })
    }

    pub fn degree_u(&self) -> u32 {
        self.degree_u
    }

    pub fn degree_v(&self) -> u32 {
        self.degree_v
    }

    pub fn control_points(&self) -> &[Point3] {
        &self.control_points
    }

    pub fn count_u(&self) -> u32 {
        self.count_u
    }

    pub fn count_v(&self) -> u32 {
        self.count_v
    }

    pub fn knots_u(&self) -> &[f64] {
        &self.knots_u
    }

    pub fn knots_v(&self) -> &[f64] {
        &self.knots_v
    }

    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    pub fn domain(&self) -> SurfaceDomain {
        let pu = self.degree_u as usize;
        let pv = self.degree_v as usize;
        SurfaceDomain {
            u_start: self.knots_u[pu],
            u_end: self.knots_u[self.count_u as usize],
            v_start: self.knots_v[pv],
            v_end: self.knots_v[self.count_v as usize],
        }
    }

    /// Evaluate a point on the surface at parameters (u, v).
    pub fn point_at(&self, u: f64, v: f64) -> Point3 {
        let span_u = self.find_span(&self.knots_u, self.count_u as usize - 1, self.degree_u as usize, u);
        let span_v = self.find_span(&self.knots_v, self.count_v as usize - 1, self.degree_v as usize, v);
        let basis_u = self.basis_functions(&self.knots_u, span_u, self.degree_u as usize, u);
        let basis_v = self.basis_functions(&self.knots_v, span_v, self.degree_v as usize, v);

        let mut point = [0.0; 3];
        let mut w_sum = 0.0;

        for i in 0..=self.degree_u as usize {
            for j in 0..=self.degree_v as usize {
                let row = span_u - self.degree_u as usize + i;
                let col = span_v - self.degree_v as usize + j;
                let idx = row * self.count_v as usize + col;

                let w = self.weights[idx];
                let bw = basis_u[i] * basis_v[j] * w;

                point[0] += bw * self.control_points[idx].x;
                point[1] += bw * self.control_points[idx].y;
                point[2] += bw * self.control_points[idx].z;
                w_sum += bw;
            }
        }

        Point3::new(point[0] / w_sum, point[1] / w_sum, point[2] / w_sum)
    }

    /// Compute normal at (u, v) via finite differences.
    pub fn normal_at(&self, u: f64, v: f64) -> Vector3 {
        let h = 1e-7;
        let domain = self.domain();
        let du = (self.point_at((u + h).min(domain.u_end), v) - self.point_at((u - h).max(domain.u_start), v)).normalize();
        let dv = (self.point_at(u, (v + h).min(domain.v_end)) - self.point_at(u, (v - h).max(domain.v_start))).normalize();
        du.cross(&dv).normalize()
    }

    /// Bounding box from control points.
    pub fn bounding_box(&self) -> Aabb3 {
        Aabb3::from_points(&self.control_points).unwrap()
    }

    fn find_span(&self, knots: &[f64], n: usize, p: usize, t: f64) -> usize {
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

    fn basis_functions(&self, knots: &[f64], span: usize, degree: usize, t: f64) -> Vec<f64> {
        let p = degree;
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
