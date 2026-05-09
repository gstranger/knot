use crate::point::{Point3, Vector3};

/// A sphere centered at a point with a given radius.
/// Parameterized as u = longitude [0, 2pi], v = latitude [-pi/2, pi/2].
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Sphere {
    pub center: Point3,
    pub radius: f64,
}

impl Sphere {
    pub fn new(center: Point3, radius: f64) -> Self {
        Self { center, radius }
    }

    /// Evaluate at (u=longitude, v=latitude).
    pub fn point_at(&self, u: f64, v: f64) -> Point3 {
        let cos_v = v.cos();
        Point3::new(
            self.center.x + self.radius * cos_v * u.cos(),
            self.center.y + self.radius * cos_v * u.sin(),
            self.center.z + self.radius * v.sin(),
        )
    }

    pub fn normal_at(&self, u: f64, v: f64) -> Vector3 {
        let cos_v = v.cos();
        Vector3::new(cos_v * u.cos(), cos_v * u.sin(), v.sin())
    }
}
