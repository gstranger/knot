use knot_core::exact::*;

#[test]
fn exact_rational_roundtrip() {
    let r = ExactRational::from_f64(3.14);
    let v = r.to_f64();
    assert!((v - 3.14).abs() < 1e-15);
}

#[test]
fn exact_rational_sign() {
    use std::cmp::Ordering;
    assert_eq!(ExactRational::from_f64(1.0).sign(), Ordering::Greater);
    assert_eq!(ExactRational::from_f64(-1.0).sign(), Ordering::Less);
    assert_eq!(ExactRational::from_f64(0.0).sign(), Ordering::Equal);
}

#[test]
fn orient3d_positive_side() {
    // Triangle: (0,0,0), (1,0,0), (0,1,0)
    // det(a-d, b-d, c-d) is negative when d is on the +z side of this triangle.
    // Standard convention: orient3d > 0 when d is below the plane (a,b,c).
    let a = ExactPoint3::from_f64(0.0, 0.0, 0.0);
    let b = ExactPoint3::from_f64(1.0, 0.0, 0.0);
    let c = ExactPoint3::from_f64(0.0, 1.0, 0.0);

    // d below plane → positive determinant
    let d = ExactPoint3::from_f64(0.0, 0.0, -1.0);
    assert_eq!(orient3d(&a, &b, &c, &d), Orientation::Positive);
}

#[test]
fn orient3d_negative_side() {
    let a = ExactPoint3::from_f64(0.0, 0.0, 0.0);
    let b = ExactPoint3::from_f64(1.0, 0.0, 0.0);
    let c = ExactPoint3::from_f64(0.0, 1.0, 0.0);

    // d above plane → negative determinant
    let d = ExactPoint3::from_f64(0.0, 0.0, 1.0);
    assert_eq!(orient3d(&a, &b, &c, &d), Orientation::Negative);
}

#[test]
fn orient3d_coplanar() {
    let a = ExactPoint3::from_f64(0.0, 0.0, 0.0);
    let b = ExactPoint3::from_f64(1.0, 0.0, 0.0);
    let c = ExactPoint3::from_f64(0.0, 1.0, 0.0);
    let d = ExactPoint3::from_f64(0.5, 0.5, 0.0);
    assert_eq!(orient3d(&a, &b, &c, &d), Orientation::Zero);
}

#[test]
fn orient2d_ccw() {
    let a = [ExactRational::from_f64(0.0), ExactRational::from_f64(0.0)];
    let b = [ExactRational::from_f64(1.0), ExactRational::from_f64(0.0)];
    let c = [ExactRational::from_f64(0.0), ExactRational::from_f64(1.0)];
    assert_eq!(orient2d(&a, &b, &c), Orientation::Positive);
}

#[test]
fn orient2d_cw() {
    let a = [ExactRational::from_f64(0.0), ExactRational::from_f64(0.0)];
    let b = [ExactRational::from_f64(0.0), ExactRational::from_f64(1.0)];
    let c = [ExactRational::from_f64(1.0), ExactRational::from_f64(0.0)];
    assert_eq!(orient2d(&a, &b, &c), Orientation::Negative);
}

#[test]
fn orient2d_collinear() {
    let a = [ExactRational::from_f64(0.0), ExactRational::from_f64(0.0)];
    let b = [ExactRational::from_f64(1.0), ExactRational::from_f64(1.0)];
    let c = [ExactRational::from_f64(2.0), ExactRational::from_f64(2.0)];
    assert_eq!(orient2d(&a, &b, &c), Orientation::Zero);
}
