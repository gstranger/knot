use crate::point::{Isometry3, Point3, Vector3};

/// Create a translation transform.
pub fn translation(v: Vector3) -> Isometry3 {
    Isometry3::translation(v.x, v.y, v.z)
}

/// Create a rotation around an axis through the origin.
pub fn rotation(axis: Vector3, angle: f64) -> Isometry3 {
    let axis_angle = nalgebra::Vector3::new(
        axis.x * angle,
        axis.y * angle,
        axis.z * angle,
    );
    Isometry3::new(nalgebra::Vector3::zeros(), axis_angle)
}

/// Transform a point.
pub fn transform_point(iso: &Isometry3, p: &Point3) -> Point3 {
    iso.transform_point(p)
}

/// Transform a direction vector.
pub fn transform_vector(iso: &Isometry3, v: &Vector3) -> Vector3 {
    iso.transform_vector(v)
}
