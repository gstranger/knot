use crate::point::{Point3, Vector3};

/// A cylinder along an axis with given radius.
/// u = angle [0, 2pi], v = height along axis.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Cylinder {
    pub origin: Point3,
    pub axis: Vector3,
    pub radius: f64,
    pub ref_direction: Vector3,
    pub v_min: f64,
    pub v_max: f64,
}

impl Cylinder {
    pub fn point_at(&self, u: f64, v: f64) -> Point3 {
        let binormal = self.axis.cross(&self.ref_direction);
        let cos_u = u.cos();
        let sin_u = u.sin();
        Point3::new(
            self.origin.x + self.radius * (cos_u * self.ref_direction.x + sin_u * binormal.x) + v * self.axis.x,
            self.origin.y + self.radius * (cos_u * self.ref_direction.y + sin_u * binormal.y) + v * self.axis.y,
            self.origin.z + self.radius * (cos_u * self.ref_direction.z + sin_u * binormal.z) + v * self.axis.z,
        )
    }

    pub fn normal_at(&self, u: f64, _v: f64) -> Vector3 {
        let binormal = self.axis.cross(&self.ref_direction);
        let cos_u = u.cos();
        let sin_u = u.sin();
        Vector3::new(
            cos_u * self.ref_direction.x + sin_u * binormal.x,
            cos_u * self.ref_direction.y + sin_u * binormal.y,
            cos_u * self.ref_direction.z + sin_u * binormal.z,
        )
    }
}
