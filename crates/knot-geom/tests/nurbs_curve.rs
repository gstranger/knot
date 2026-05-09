use knot_geom::Point3;
use knot_geom::curve::NurbsCurve;

fn make_quadratic_bezier() -> NurbsCurve {
    let pts = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(0.5, 1.0, 0.0),
        Point3::new(1.0, 0.0, 0.0),
    ];
    let weights = vec![1.0, 1.0, 1.0];
    let knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    NurbsCurve::new(pts, weights, knots, 2).unwrap()
}

fn make_rational_arc() -> NurbsCurve {
    // Quarter circle approximation using a rational quadratic Bezier
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let pts = vec![
        Point3::new(1.0, 0.0, 0.0),
        Point3::new(1.0, 1.0, 0.0),
        Point3::new(0.0, 1.0, 0.0),
    ];
    let weights = vec![1.0, w, 1.0];
    let knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    NurbsCurve::new(pts, weights, knots, 2).unwrap()
}

#[test]
fn endpoints() {
    let c = make_quadratic_bezier();
    let start = c.point_at(0.0);
    let end = c.point_at(1.0);
    assert!((start.x - 0.0).abs() < 1e-12);
    assert!((start.y - 0.0).abs() < 1e-12);
    assert!((end.x - 1.0).abs() < 1e-12);
    assert!((end.y - 0.0).abs() < 1e-12);
}

#[test]
fn midpoint_of_symmetric_bezier() {
    let c = make_quadratic_bezier();
    let mid = c.point_at(0.5);
    assert!((mid.x - 0.5).abs() < 1e-12);
    assert!((mid.y - 0.5).abs() < 1e-12); // quadratic Bezier midpoint
}

#[test]
fn rational_arc_midpoint_on_circle() {
    let c = make_rational_arc();
    let mid = c.point_at(0.5);
    let r = (mid.x * mid.x + mid.y * mid.y).sqrt();
    assert!((r - 1.0).abs() < 1e-12, "midpoint should be on unit circle, r = {r}");
}

#[test]
fn domain() {
    let c = make_quadratic_bezier();
    let d = c.domain();
    assert_eq!(d.start, 0.0);
    assert_eq!(d.end, 1.0);
}

#[test]
fn bounding_box_contains_endpoints() {
    let c = make_quadratic_bezier();
    let bb = c.bounding_box();
    assert!(bb.min.x <= 0.0 && bb.max.x >= 1.0);
    assert!(bb.min.y <= 0.0 && bb.max.y >= 1.0);
}

#[test]
fn knot_insertion_preserves_shape() {
    let c = make_quadratic_bezier();
    let c2 = c.insert_knot(0.5);

    // Evaluate at several parameters — the curve shape should be identical
    for i in 0..=10 {
        let t = i as f64 / 10.0;
        let p1 = c.point_at(t);
        let p2 = c2.point_at(t);
        assert!((p1.x - p2.x).abs() < 1e-10, "x mismatch at t={t}");
        assert!((p1.y - p2.y).abs() < 1e-10, "y mismatch at t={t}");
        assert!((p1.z - p2.z).abs() < 1e-10, "z mismatch at t={t}");
    }
}

#[test]
fn knot_insertion_adds_control_point() {
    let c = make_quadratic_bezier();
    let c2 = c.insert_knot(0.5);
    assert_eq!(c2.control_points().len(), c.control_points().len() + 1);
}

#[test]
fn invalid_too_few_control_points() {
    let pts = vec![Point3::new(0.0, 0.0, 0.0)];
    let weights = vec![1.0];
    let knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    assert!(NurbsCurve::new(pts, weights, knots, 2).is_err());
}

#[test]
fn invalid_negative_weight() {
    let pts = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(0.5, 1.0, 0.0),
        Point3::new(1.0, 0.0, 0.0),
    ];
    let weights = vec![1.0, -1.0, 1.0];
    let knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    assert!(NurbsCurve::new(pts, weights, knots, 2).is_err());
}

#[test]
fn invalid_knot_count() {
    let pts = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(0.5, 1.0, 0.0),
        Point3::new(1.0, 0.0, 0.0),
    ];
    let weights = vec![1.0, 1.0, 1.0];
    let knots = vec![0.0, 0.0, 1.0, 1.0]; // wrong count
    assert!(NurbsCurve::new(pts, weights, knots, 2).is_err());
}
