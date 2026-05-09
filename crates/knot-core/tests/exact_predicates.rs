use knot_core::exact::*;

#[test]
fn point_side_of_plane_above() {
    let origin = ExactPoint3::from_f64(0.0, 0.0, 0.0);
    let normal = ExactPoint3::from_f64(0.0, 0.0, 1.0);
    let query = ExactPoint3::from_f64(1.0, 2.0, 5.0);
    assert_eq!(point_side_of_plane(&origin, &normal, &query), Orientation::Positive);
}

#[test]
fn point_side_of_plane_below() {
    let origin = ExactPoint3::from_f64(0.0, 0.0, 0.0);
    let normal = ExactPoint3::from_f64(0.0, 0.0, 1.0);
    let query = ExactPoint3::from_f64(1.0, 2.0, -3.0);
    assert_eq!(point_side_of_plane(&origin, &normal, &query), Orientation::Negative);
}

#[test]
fn point_side_of_plane_on() {
    let origin = ExactPoint3::from_f64(0.0, 0.0, 0.0);
    let normal = ExactPoint3::from_f64(0.0, 0.0, 1.0);
    let query = ExactPoint3::from_f64(7.0, 3.0, 0.0);
    assert_eq!(point_side_of_plane(&origin, &normal, &query), Orientation::Zero);
}

#[test]
fn point_side_of_tilted_plane() {
    let origin = ExactPoint3::from_f64(1.0, 1.0, 1.0);
    let normal = ExactPoint3::from_f64(1.0, 1.0, 1.0); // not unit — exact doesn't need it
    let query = ExactPoint3::from_f64(2.0, 2.0, 2.0); // on the positive side
    assert_eq!(point_side_of_plane(&origin, &normal, &query), Orientation::Positive);
}

#[test]
fn exact_cross_product() {
    let a = ExactPoint3::from_f64(1.0, 0.0, 0.0);
    let b = ExactPoint3::from_f64(0.0, 1.0, 0.0);
    let c = a.cross(&b);
    assert_eq!(c.x.to_f64(), 0.0);
    assert_eq!(c.y.to_f64(), 0.0);
    assert_eq!(c.z.to_f64(), 1.0);
}

#[test]
fn exact_dot_product() {
    let a = ExactPoint3::from_f64(1.0, 2.0, 3.0);
    let b = ExactPoint3::from_f64(4.0, 5.0, 6.0);
    let d = a.dot(&b);
    assert_eq!(d.to_f64(), 32.0); // 1*4 + 2*5 + 3*6
}

#[test]
fn exact_arithmetic_no_cancellation() {
    // This tests the key advantage of exact arithmetic:
    // near-cancellation that would lose precision in f64
    let a = ExactRational::from_f64(1.0);
    let b = ExactRational::from_f64(1e-15);
    let c = &a + &b;
    let d = &c - &a;
    // In f64 this would lose precision. In exact arithmetic, d == b exactly.
    assert_eq!(d, b);
}
