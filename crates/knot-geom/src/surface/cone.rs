use crate::point::{Point3, Vector3};

/// A cone with apex, axis direction, and half-angle.
/// u = angle [0, 2pi], v = distance along axis from apex.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Cone {
    pub apex: Point3,
    pub axis: Vector3,
    pub half_angle: f64,
    pub ref_direction: Vector3,
    pub v_min: f64,
    pub v_max: f64,
}

impl Cone {
    pub fn point_at(&self, u: f64, v: f64) -> Point3 {
        let binormal = self.axis.cross(&self.ref_direction);
        let r = v * self.half_angle.tan();
        let cos_u = u.cos();
        let sin_u = u.sin();
        Point3::new(
            self.apex.x + v * self.axis.x + r * (cos_u * self.ref_direction.x + sin_u * binormal.x),
            self.apex.y + v * self.axis.y + r * (cos_u * self.ref_direction.y + sin_u * binormal.y),
            self.apex.z + v * self.axis.z + r * (cos_u * self.ref_direction.z + sin_u * binormal.z),
        )
    }

    pub fn normal_at(&self, u: f64, _v: f64) -> Vector3 {
        let binormal = self.axis.cross(&self.ref_direction);
        let cos_u = u.cos();
        let sin_u = u.sin();
        let cos_ha = self.half_angle.cos();
        let sin_ha = self.half_angle.sin();
        // Normal is perpendicular to surface: radial component * cos(half_angle) - axis * sin(half_angle)
        let radial = Vector3::new(
            cos_u * self.ref_direction.x + sin_u * binormal.x,
            cos_u * self.ref_direction.y + sin_u * binormal.y,
            cos_u * self.ref_direction.z + sin_u * binormal.z,
        );
        (radial * cos_ha - self.axis * sin_ha).normalize()
    }
}
