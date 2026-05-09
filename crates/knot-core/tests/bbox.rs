use knot_core::Aabb3;
use nalgebra::Point3;

#[test]
fn from_points() {
    let pts = vec![
        Point3::new(1.0, 2.0, 3.0),
        Point3::new(-1.0, 5.0, 0.0),
        Point3::new(3.0, 0.0, 1.0),
    ];
    let bb = Aabb3::from_points(&pts).unwrap();
    assert_eq!(bb.min, Point3::new(-1.0, 0.0, 0.0));
    assert_eq!(bb.max, Point3::new(3.0, 5.0, 3.0));
}

#[test]
fn from_points_empty() {
    assert!(Aabb3::from_points(&[]).is_none());
}

#[test]
fn intersects() {
    let a = Aabb3::new(Point3::new(0.0, 0.0, 0.0), Point3::new(2.0, 2.0, 2.0));
    let b = Aabb3::new(Point3::new(1.0, 1.0, 1.0), Point3::new(3.0, 3.0, 3.0));
    let c = Aabb3::new(Point3::new(5.0, 5.0, 5.0), Point3::new(6.0, 6.0, 6.0));
    assert!(a.intersects(&b));
    assert!(!a.intersects(&c));
}

#[test]
fn union() {
    let a = Aabb3::new(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 1.0, 1.0));
    let b = Aabb3::new(Point3::new(2.0, 2.0, 2.0), Point3::new(3.0, 3.0, 3.0));
    let u = a.union(&b);
    assert_eq!(u.min, Point3::new(0.0, 0.0, 0.0));
    assert_eq!(u.max, Point3::new(3.0, 3.0, 3.0));
}

#[test]
fn expand() {
    let bb = Aabb3::new(Point3::new(1.0, 1.0, 1.0), Point3::new(2.0, 2.0, 2.0));
    let expanded = bb.expand(0.5);
    assert_eq!(expanded.min, Point3::new(0.5, 0.5, 0.5));
    assert_eq!(expanded.max, Point3::new(2.5, 2.5, 2.5));
}

#[test]
fn center_and_diagonal() {
    let bb = Aabb3::new(Point3::new(0.0, 0.0, 0.0), Point3::new(4.0, 6.0, 8.0));
    assert_eq!(bb.center(), Point3::new(2.0, 3.0, 4.0));
    assert_eq!(bb.diagonal(), nalgebra::Vector3::new(4.0, 6.0, 8.0));
    assert!((bb.diagonal_length() - (4.0_f64.powi(2) + 6.0_f64.powi(2) + 8.0_f64.powi(2)).sqrt()).abs() < 1e-12);
}
