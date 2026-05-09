use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::curve::{Curve, LineSeg, NurbsCurve};
use knot_geom::surface::{Surface, Plane, Sphere, Cylinder};
use knot_intersect::curve_surface::intersect_curve_surface;

const TOL: f64 = 1e-6;

#[test]
fn line_plane_perpendicular() {
    let line = Curve::Line(LineSeg::new(
        Point3::new(0.5, 0.5, -1.0),
        Point3::new(0.5, 0.5, 1.0),
    ));
    let plane = Surface::Plane(Plane::new(Point3::origin(), Vector3::z()));
    let hits = intersect_curve_surface(&line, &plane, TOL).unwrap();
    assert_eq!(hits.len(), 1);
    assert!((hits[0].point.z).abs() < TOL);
    assert!((hits[0].curve_param.0 - 0.5).abs() < TOL);
}

#[test]
fn line_plane_parallel() {
    let line = Curve::Line(LineSeg::new(
        Point3::new(0.0, 0.0, 1.0),
        Point3::new(1.0, 0.0, 1.0),
    ));
    let plane = Surface::Plane(Plane::new(Point3::origin(), Vector3::z()));
    let hits = intersect_curve_surface(&line, &plane, TOL).unwrap();
    assert!(hits.is_empty());
}

#[test]
fn line_sphere_through_center() {
    let line = Curve::Line(LineSeg::new(
        Point3::new(-2.0, 0.0, 0.0),
        Point3::new(2.0, 0.0, 0.0),
    ));
    let sphere = Surface::Sphere(Sphere::new(Point3::origin(), 1.0));
    let hits = intersect_curve_surface(&line, &sphere, TOL).unwrap();
    assert_eq!(hits.len(), 2);
    // Entry and exit at x = -1 and x = 1
    let xs: Vec<f64> = hits.iter().map(|h| h.point.x).collect();
    assert!(xs.iter().any(|x| (x + 1.0).abs() < TOL));
    assert!(xs.iter().any(|x| (x - 1.0).abs() < TOL));
}

#[test]
fn line_sphere_miss() {
    let line = Curve::Line(LineSeg::new(
        Point3::new(-2.0, 2.0, 0.0),
        Point3::new(2.0, 2.0, 0.0),
    ));
    let sphere = Surface::Sphere(Sphere::new(Point3::origin(), 1.0));
    let hits = intersect_curve_surface(&line, &sphere, TOL).unwrap();
    assert!(hits.is_empty());
}

#[test]
fn line_sphere_tangent() {
    let line = Curve::Line(LineSeg::new(
        Point3::new(-2.0, 1.0, 0.0),
        Point3::new(2.0, 1.0, 0.0),
    ));
    let sphere = Surface::Sphere(Sphere::new(Point3::origin(), 1.0));
    let hits = intersect_curve_surface(&line, &sphere, TOL).unwrap();
    assert_eq!(hits.len(), 1);
    assert!((hits[0].point.y - 1.0).abs() < TOL);
}

#[test]
fn line_cylinder_two_hits() {
    let cyl = Surface::Cylinder(Cylinder {
        origin: Point3::origin(),
        axis: Vector3::z(),
        radius: 1.0,
        ref_direction: Vector3::x(),
        v_min: -5.0,
        v_max: 5.0,
    });
    let line = Curve::Line(LineSeg::new(
        Point3::new(-2.0, 0.0, 0.0),
        Point3::new(2.0, 0.0, 0.0),
    ));
    let hits = intersect_curve_surface(&line, &cyl, TOL).unwrap();
    assert_eq!(hits.len(), 2);
}
