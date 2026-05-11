pub mod nurbs;
pub mod line;
pub mod arc;
pub mod offset;
pub mod fit;

use knot_core::Aabb3;
use crate::point::{Point3, Vector3};

pub use nurbs::NurbsCurve;
pub use line::LineSeg;
pub use arc::{CircularArc, EllipticalArc};

/// Parameter value on a curve.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct CurveParam(pub f64);

/// Derivative information at a curve point.
#[derive(Clone, Debug)]
pub struct CurveDerivatives {
    pub point: Point3,
    pub d1: Vector3,
    pub d2: Option<Vector3>,
}

/// Closest-point query result.
#[derive(Clone, Debug)]
pub struct CurveClosestPoint {
    pub param: CurveParam,
    pub point: Point3,
    pub distance: f64,
}

/// Curve domain [start, end].
#[derive(Clone, Copy, Debug)]
pub struct CurveDomain {
    pub start: f64,
    pub end: f64,
}

/// All curve types the kernel can represent.
/// Enum dispatch for analytical fast-paths in intersection.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Curve {
    Nurbs(NurbsCurve),
    Line(LineSeg),
    CircularArc(CircularArc),
    EllipticalArc(EllipticalArc),
}

impl Curve {
    /// Evaluate a point on the curve at parameter t.
    pub fn point_at(&self, t: CurveParam) -> Point3 {
        match self {
            Curve::Nurbs(c) => c.point_at(t.0),
            Curve::Line(l) => l.point_at(t.0),
            Curve::CircularArc(a) => a.point_at(t.0),
            Curve::EllipticalArc(a) => a.point_at(t.0),
        }
    }

    /// Compute derivatives at parameter t.
    pub fn derivatives_at(&self, t: CurveParam) -> CurveDerivatives {
        match self {
            Curve::Nurbs(c) => c.derivatives_at(t.0),
            Curve::Line(l) => l.derivatives_at(t.0),
            Curve::CircularArc(a) => a.derivatives_at(t.0),
            Curve::EllipticalArc(a) => a.derivatives_at(t.0),
        }
    }

    /// Get the curve domain.
    pub fn domain(&self) -> CurveDomain {
        match self {
            Curve::Nurbs(c) => c.domain(),
            Curve::Line(_) => CurveDomain { start: 0.0, end: 1.0 },
            Curve::CircularArc(a) => CurveDomain { start: a.start_angle, end: a.end_angle },
            Curve::EllipticalArc(a) => CurveDomain { start: a.start_angle, end: a.end_angle },
        }
    }

    /// Closest point on the curve to a query point.
    pub fn closest_point(&self, query: &Point3) -> CurveClosestPoint {
        match self {
            Curve::Nurbs(c) => c.closest_point(query, 64),
            Curve::Line(l) => l.closest_point(query),
            Curve::CircularArc(a) => a.closest_point(query),
            Curve::EllipticalArc(_) => {
                // Fallback: sample-based for elliptical arcs
                nurbs_closest_point_sampled(self, query, 64)
            }
        }
    }

    /// Compute bounding box.
    pub fn bounding_box(&self) -> Aabb3 {
        match self {
            Curve::Nurbs(c) => c.bounding_box(),
            Curve::Line(l) => l.bounding_box(),
            Curve::CircularArc(a) => a.bounding_box(),
            Curve::EllipticalArc(a) => a.bounding_box(),
        }
    }

    /// Arc length. Closed-form for lines and circular arcs; adaptive
    /// integration for NURBS; chord-sum-with-refinement for elliptical
    /// arcs (whose arc length is an elliptic integral with no
    /// elementary form).
    pub fn length(&self, tolerance: f64) -> f64 {
        match self {
            Curve::Line(l) => l.length(),
            Curve::CircularArc(a) => a.length(),
            Curve::Nurbs(n) => n.length(tolerance),
            Curve::EllipticalArc(_) => length_by_sampling(self, tolerance),
        }
    }

    /// Split the curve at parameter `t` into two sub-curves. Errors if
    /// `t` is outside `(domain.start, domain.end)`. Variant-on-input
    /// = variant-on-output for all cases.
    pub fn split_at(&self, t: CurveParam) -> Result<(Curve, Curve), &'static str> {
        let domain = self.domain();
        if t.0 <= domain.start || t.0 >= domain.end {
            return Err("split_at: parameter outside (start, end)");
        }
        match self {
            Curve::Line(l) => {
                let (a, b) = l.split_at(t.0);
                Ok((Curve::Line(a), Curve::Line(b)))
            }
            Curve::CircularArc(a) => {
                let (l, r) = a.split_at(t.0);
                Ok((Curve::CircularArc(l), Curve::CircularArc(r)))
            }
            Curve::EllipticalArc(a) => {
                let mut left = a.clone();
                let mut right = a.clone();
                left.end_angle = t.0;
                right.start_angle = t.0;
                Ok((Curve::EllipticalArc(left), Curve::EllipticalArc(right)))
            }
            Curve::Nurbs(n) => {
                let (l, r) = n.split_at(t.0);
                Ok((Curve::Nurbs(l), Curve::Nurbs(r)))
            }
        }
    }

    /// Reversed-orientation copy. The 3D point set is identical;
    /// only the parameterization runs the other way.
    pub fn reverse(&self) -> Curve {
        match self {
            Curve::Line(l) => Curve::Line(l.reverse()),
            Curve::CircularArc(a) => {
                let mut r = a.clone();
                r.start_angle = a.end_angle;
                r.end_angle = a.start_angle;
                Curve::CircularArc(r)
            }
            Curve::EllipticalArc(a) => {
                let mut r = a.clone();
                r.start_angle = a.end_angle;
                r.end_angle = a.start_angle;
                Curve::EllipticalArc(r)
            }
            Curve::Nurbs(n) => Curve::Nurbs(n.reverse()),
        }
    }

    /// Return `n + 1` parameter values evenly spaced by arc length
    /// (including both endpoints). For non-linearly-parameterized
    /// curves this gives a very different distribution than dividing
    /// the parameter domain evenly.
    pub fn divide_by_length(&self, n: u32, _tolerance: f64) -> Vec<CurveParam> {
        if n == 0 {
            return vec![CurveParam(self.domain().start)];
        }
        let domain = self.domain();
        // Sample density: enough for sub-tolerance interpolation on
        // typical CAD curves. Could be made adaptive on `_tolerance`
        // later if drift shows up on very curvy NURBS.
        let samples = (256 * n as usize).max(256);
        let dt = (domain.end - domain.start) / samples as f64;
        let mut params = Vec::with_capacity(samples + 1);
        let mut lengths = Vec::with_capacity(samples + 1);
        params.push(domain.start);
        lengths.push(0.0_f64);
        let mut prev = self.point_at(CurveParam(domain.start));
        for i in 1..=samples {
            let t = domain.start + dt * i as f64;
            let p = self.point_at(CurveParam(t));
            let cum = lengths[i - 1] + (p - prev).norm();
            params.push(t);
            lengths.push(cum);
            prev = p;
        }
        let total = *lengths.last().unwrap();
        let mut out = Vec::with_capacity(n as usize + 1);
        for i in 0..=n {
            let target = total * i as f64 / n as f64;
            let mut lo = 0usize;
            let mut hi = lengths.len() - 1;
            while hi - lo > 1 {
                let mid = (lo + hi) / 2;
                if lengths[mid] <= target {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let span = lengths[hi] - lengths[lo];
            let frac = if span == 0.0 { 0.0 } else { (target - lengths[lo]) / span };
            let t = params[lo] + frac * (params[hi] - params[lo]);
            out.push(CurveParam(t));
        }
        out
    }
}

fn length_by_sampling(curve: &Curve, tolerance: f64) -> f64 {
    let mut n = 32usize;
    let mut prev = chord_sum(curve, n);
    for _ in 0..16 {
        n *= 2;
        let est = chord_sum(curve, n);
        if (est - prev).abs() <= tolerance * est.abs().max(1.0) {
            return est;
        }
        prev = est;
    }
    prev
}

fn chord_sum(curve: &Curve, n: usize) -> f64 {
    let domain = curve.domain();
    let dt = (domain.end - domain.start) / n as f64;
    let mut prev = curve.point_at(CurveParam(domain.start));
    let mut s = 0.0_f64;
    for i in 1..=n {
        let t = domain.start + dt * i as f64;
        let p = curve.point_at(CurveParam(t));
        s += (p - prev).norm();
        prev = p;
    }
    s
}

/// Fallback closest-point using sampling (for curve types without analytical solution).
fn nurbs_closest_point_sampled(curve: &Curve, query: &Point3, n: usize) -> CurveClosestPoint {
    let domain = curve.domain();
    let dt = (domain.end - domain.start) / n as f64;
    let mut best_t = domain.start;
    let mut best_dist_sq = f64::MAX;

    for i in 0..=n {
        let t = domain.start + dt * i as f64;
        let p = curve.point_at(CurveParam(t));
        let d_sq = (p - query).norm_squared();
        if d_sq < best_dist_sq {
            best_dist_sq = d_sq;
            best_t = t;
        }
    }

    let point = curve.point_at(CurveParam(best_t));
    CurveClosestPoint {
        param: CurveParam(best_t),
        point,
        distance: best_dist_sq.sqrt(),
    }
}
