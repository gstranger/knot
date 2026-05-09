use knot_core::Aabb3;
use crate::point::{Point3, Vector3};
use super::{CurveDerivatives, CurveClosestPoint, CurveParam};

/// A circular arc in 3D space.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CircularArc {
    pub center: Point3,
    pub normal: Vector3,
    pub radius: f64,
    pub ref_direction: Vector3,
    pub start_angle: f64,
    pub end_angle: f64,
}

impl CircularArc {
    fn binormal(&self) -> Vector3 {
        self.normal.cross(&self.ref_direction)
    }

    /// Evaluate at parameter t (angle in radians).
    pub fn point_at(&self, t: f64) -> Point3 {
        let b = self.binormal();
        let cos_t = t.cos();
        let sin_t = t.sin();
        Point3::new(
            self.center.x + self.radius * (cos_t * self.ref_direction.x + sin_t * b.x),
            self.center.y + self.radius * (cos_t * self.ref_direction.y + sin_t * b.y),
            self.center.z + self.radius * (cos_t * self.ref_direction.z + sin_t * b.z),
        )
    }

    pub fn derivatives_at(&self, t: f64) -> CurveDerivatives {
        let b = self.binormal();
        let cos_t = t.cos();
        let sin_t = t.sin();
        let point = self.point_at(t);
        // d/dt = radius * (-sin(t) * ref + cos(t) * binormal)
        let d1 = Vector3::new(
            self.radius * (-sin_t * self.ref_direction.x + cos_t * b.x),
            self.radius * (-sin_t * self.ref_direction.y + cos_t * b.y),
            self.radius * (-sin_t * self.ref_direction.z + cos_t * b.z),
        );
        // d2/dt2 = radius * (-cos(t) * ref - sin(t) * binormal)
        let d2 = Vector3::new(
            self.radius * (-cos_t * self.ref_direction.x - sin_t * b.x),
            self.radius * (-cos_t * self.ref_direction.y - sin_t * b.y),
            self.radius * (-cos_t * self.ref_direction.z - sin_t * b.z),
        );
        CurveDerivatives { point, d1, d2: Some(d2) }
    }

    pub fn closest_point(&self, query: &Point3) -> CurveClosestPoint {
        // Project query onto the arc's plane, find angle, clamp to domain
        let v = query - self.center;
        let u_comp = v.dot(&self.ref_direction);
        let v_comp = v.dot(&self.binormal());
        let angle = v_comp.atan2(u_comp);
        // Normalize angle to be within [start_angle, end_angle]
        let t = clamp_angle(angle, self.start_angle, self.end_angle);
        let point = self.point_at(t);
        CurveClosestPoint {
            param: CurveParam(t),
            point,
            distance: (query - point).norm(),
        }
    }

    pub fn length(&self) -> f64 {
        self.radius * (self.end_angle - self.start_angle).abs()
    }

    pub fn split_at(&self, t: f64) -> (CircularArc, CircularArc) {
        let left = CircularArc {
            center: self.center,
            normal: self.normal,
            radius: self.radius,
            ref_direction: self.ref_direction,
            start_angle: self.start_angle,
            end_angle: t,
        };
        let right = CircularArc {
            center: self.center,
            normal: self.normal,
            radius: self.radius,
            ref_direction: self.ref_direction,
            start_angle: t,
            end_angle: self.end_angle,
        };
        (left, right)
    }

    pub fn bounding_box(&self) -> Aabb3 {
        let n = 16;
        let dt = (self.end_angle - self.start_angle) / n as f64;
        let pts: Vec<Point3> = (0..=n)
            .map(|i| self.point_at(self.start_angle + i as f64 * dt))
            .collect();
        Aabb3::from_points(&pts).unwrap()
    }
}

/// An elliptical arc in 3D space.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EllipticalArc {
    pub center: Point3,
    pub normal: Vector3,
    pub major_axis: Vector3,
    pub major_radius: f64,
    pub minor_radius: f64,
    pub start_angle: f64,
    pub end_angle: f64,
}

impl EllipticalArc {
    fn minor_axis(&self) -> Vector3 {
        self.normal.cross(&self.major_axis)
    }

    /// Evaluate at parameter t (angle in radians).
    pub fn point_at(&self, t: f64) -> Point3 {
        let minor = self.minor_axis();
        let cos_t = t.cos();
        let sin_t = t.sin();
        Point3::new(
            self.center.x + self.major_radius * cos_t * self.major_axis.x
                + self.minor_radius * sin_t * minor.x,
            self.center.y + self.major_radius * cos_t * self.major_axis.y
                + self.minor_radius * sin_t * minor.y,
            self.center.z + self.major_radius * cos_t * self.major_axis.z
                + self.minor_radius * sin_t * minor.z,
        )
    }

    pub fn derivatives_at(&self, t: f64) -> CurveDerivatives {
        let minor = self.minor_axis();
        let cos_t = t.cos();
        let sin_t = t.sin();
        let point = self.point_at(t);
        let d1 = Vector3::new(
            -self.major_radius * sin_t * self.major_axis.x + self.minor_radius * cos_t * minor.x,
            -self.major_radius * sin_t * self.major_axis.y + self.minor_radius * cos_t * minor.y,
            -self.major_radius * sin_t * self.major_axis.z + self.minor_radius * cos_t * minor.z,
        );
        let d2 = Vector3::new(
            -self.major_radius * cos_t * self.major_axis.x - self.minor_radius * sin_t * minor.x,
            -self.major_radius * cos_t * self.major_axis.y - self.minor_radius * sin_t * minor.y,
            -self.major_radius * cos_t * self.major_axis.z - self.minor_radius * sin_t * minor.z,
        );
        CurveDerivatives { point, d1, d2: Some(d2) }
    }

    pub fn bounding_box(&self) -> Aabb3 {
        let n = 16;
        let dt = (self.end_angle - self.start_angle) / n as f64;
        let pts: Vec<Point3> = (0..=n)
            .map(|i| self.point_at(self.start_angle + i as f64 * dt))
            .collect();
        Aabb3::from_points(&pts).unwrap()
    }
}

/// Clamp an angle to the range [start, end], handling wrapping.
fn clamp_angle(mut angle: f64, start: f64, end: f64) -> f64 {
    use std::f64::consts::TAU;
    // Normalize angle relative to start
    while angle < start {
        angle += TAU;
    }
    while angle > start + TAU {
        angle -= TAU;
    }
    angle.clamp(start, end)
}
