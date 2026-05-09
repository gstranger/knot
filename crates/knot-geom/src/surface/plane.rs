use crate::point::{Point3, Vector3};

/// An infinite plane defined by origin and orthonormal frame.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Plane {
    pub origin: Point3,
    pub normal: Vector3,
    pub u_axis: Vector3,
    pub v_axis: Vector3,
}

impl Plane {
    pub fn new(origin: Point3, normal: Vector3) -> Self {
        let normal = normal.normalize();
        // Compute a stable orthonormal frame
        let u_axis = if normal.x.abs() < 0.9 {
            Vector3::x().cross(&normal).normalize()
        } else {
            Vector3::y().cross(&normal).normalize()
        };
        let v_axis = normal.cross(&u_axis);
        Self { origin, normal, u_axis, v_axis }
    }

    /// Evaluate at parameters (u, v) -> origin + u * u_axis + v * v_axis.
    pub fn point_at(&self, u: f64, v: f64) -> Point3 {
        Point3::new(
            self.origin.x + u * self.u_axis.x + v * self.v_axis.x,
            self.origin.y + u * self.u_axis.y + v * self.v_axis.y,
            self.origin.z + u * self.u_axis.z + v * self.v_axis.z,
        )
    }

    /// Signed distance from a point to the plane.
    pub fn signed_distance(&self, p: &Point3) -> f64 {
        (p - self.origin).dot(&self.normal)
    }
}
