use crate::point::{Point3, Vector3};

/// A torus with major and minor radii.
/// u = angle around major circle [0, 2pi], v = angle around tube [0, 2pi].
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Torus {
    pub center: Point3,
    pub axis: Vector3,
    pub major_radius: f64,
    pub minor_radius: f64,
    pub ref_direction: Vector3,
}

impl Torus {
    pub fn point_at(&self, u: f64, v: f64) -> Point3 {
        let binormal = self.axis.cross(&self.ref_direction);
        let cos_u = u.cos();
        let sin_u = u.sin();
        let cos_v = v.cos();
        let sin_v = v.sin();

        let r = self.major_radius + self.minor_radius * cos_v;
        let radial_x = cos_u * self.ref_direction.x + sin_u * binormal.x;
        let radial_y = cos_u * self.ref_direction.y + sin_u * binormal.y;
        let radial_z = cos_u * self.ref_direction.z + sin_u * binormal.z;

        Point3::new(
            self.center.x + r * radial_x + self.minor_radius * sin_v * self.axis.x,
            self.center.y + r * radial_y + self.minor_radius * sin_v * self.axis.y,
            self.center.z + r * radial_z + self.minor_radius * sin_v * self.axis.z,
        )
    }

    pub fn normal_at(&self, u: f64, v: f64) -> Vector3 {
        let binormal = self.axis.cross(&self.ref_direction);
        let cos_u = u.cos();
        let sin_u = u.sin();
        let cos_v = v.cos();
        let sin_v = v.sin();

        let radial = Vector3::new(
            cos_u * self.ref_direction.x + sin_u * binormal.x,
            cos_u * self.ref_direction.y + sin_u * binormal.y,
            cos_u * self.ref_direction.z + sin_u * binormal.z,
        );

        (radial * cos_v + self.axis * sin_v).normalize()
    }
}
