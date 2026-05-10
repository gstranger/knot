pub mod nurbs;
pub mod line;
pub mod arc;
pub mod offset;

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
