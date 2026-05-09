use knot_core::Aabb3;
use crate::point::{Point3, Vector3};
use super::{CurveClosestPoint, CurveDerivatives, CurveParam};

/// A line segment defined by start and end points.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LineSeg {
    pub start: Point3,
    pub end: Point3,
}

impl LineSeg {
    pub fn new(start: Point3, end: Point3) -> Self {
        Self { start, end }
    }

    /// Evaluate at parameter t in [0, 1].
    pub fn point_at(&self, t: f64) -> Point3 {
        Point3::new(
            self.start.x + t * (self.end.x - self.start.x),
            self.start.y + t * (self.end.y - self.start.y),
            self.start.z + t * (self.end.z - self.start.z),
        )
    }

    pub fn direction(&self) -> Vector3 {
        self.end - self.start
    }

    pub fn length(&self) -> f64 {
        self.direction().norm()
    }

    pub fn derivatives_at(&self, t: f64) -> CurveDerivatives {
        let d = self.direction();
        CurveDerivatives {
            point: self.point_at(t),
            d1: d,
            d2: Some(Vector3::zeros()),
        }
    }

    pub fn closest_point(&self, query: &Point3) -> CurveClosestPoint {
        let d = self.direction();
        let len_sq = d.norm_squared();
        let t = if len_sq < 1e-30 {
            0.0
        } else {
            ((query - self.start).dot(&d) / len_sq).clamp(0.0, 1.0)
        };
        let point = self.point_at(t);
        CurveClosestPoint {
            param: CurveParam(t),
            point,
            distance: (query - point).norm(),
        }
    }

    pub fn split_at(&self, t: f64) -> (LineSeg, LineSeg) {
        let mid = self.point_at(t);
        (LineSeg::new(self.start, mid), LineSeg::new(mid, self.end))
    }

    pub fn reverse(&self) -> LineSeg {
        LineSeg::new(self.end, self.start)
    }

    pub fn bounding_box(&self) -> Aabb3 {
        Aabb3::from_points(&[self.start, self.end]).unwrap()
    }
}
