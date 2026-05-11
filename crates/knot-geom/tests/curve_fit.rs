use knot_geom::Point3;
use knot_geom::curve::fit::{interpolate_curve, interpolate_curve_with_params, approximate_curve};

#[test]
fn interpolate_cubic_passes_through_points() {
    let points = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(1.0, 2.0, 0.0),
        Point3::new(3.0, 1.0, 0.0),
        Point3::new(4.0, 3.0, 0.0),
        Point3::new(5.0, 0.0, 0.0),
    ];
    let curve = interpolate_curve(&points, 3).unwrap();

    // Evaluate at the chord-length parameter values and check pass-through
    let n = points.len();
    let mut dists = vec![0.0; n];
    let mut total = 0.0;
    for i in 1..n {
        total += (points[i] - points[i - 1]).norm();
        dists[i] = total;
    }
    for i in 0..n {
        let t = if total > 0.0 { dists[i] / total } else { 0.0 };
        let domain = curve.domain();
        let t_mapped = domain.start + t * (domain.end - domain.start);
        let p = curve.point_at(t_mapped);
        let err = (p - points[i]).norm();
        assert!(err < 1e-8, "point {i}: error = {err}");
    }
}

#[test]
fn interpolate_quadratic_minimum_points() {
    // degree + 1 = 3 points → Bezier
    let points = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(0.5, 1.0, 0.0),
        Point3::new(1.0, 0.0, 0.0),
    ];
    let curve = interpolate_curve(&points, 2).unwrap();
    assert_eq!(curve.control_points().len(), 3);

    let start = curve.point_at(0.0);
    let end = curve.point_at(1.0);
    assert!((start - points[0]).norm() < 1e-12);
    assert!((end - points[2]).norm() < 1e-12);
}

#[test]
fn interpolate_linear() {
    let points = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(1.0, 1.0, 0.0),
        Point3::new(2.0, 0.0, 0.0),
    ];
    let curve = interpolate_curve(&points, 1).unwrap();

    // Midpoint of first segment
    let mid = curve.point_at(0.25);
    let expected = Point3::new(0.5, 0.5, 0.0);
    assert!((mid - expected).norm() < 1e-10);
}

#[test]
fn interpolate_3d_helix_points() {
    let n = 10;
    let points: Vec<Point3> = (0..n)
        .map(|i| {
            let t = i as f64 / (n - 1) as f64 * std::f64::consts::TAU;
            Point3::new(t.cos(), t.sin(), t / std::f64::consts::TAU)
        })
        .collect();
    let curve = interpolate_curve(&points, 3).unwrap();

    // Check endpoints
    let start = curve.point_at(curve.domain().start);
    let end = curve.point_at(curve.domain().end);
    assert!((start - points[0]).norm() < 1e-8);
    assert!((end - points[n - 1]).norm() < 1e-8);
}

#[test]
fn interpolate_with_custom_params() {
    let points = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(1.0, 1.0, 0.0),
        Point3::new(3.0, 0.0, 0.0),
        Point3::new(4.0, 1.0, 0.0),
    ];
    let params = vec![0.0, 0.25, 0.75, 1.0];
    let curve = interpolate_curve_with_params(&points, &params, 3).unwrap();

    // Should pass through all points at the given params
    for (i, &t) in params.iter().enumerate() {
        let p = curve.point_at(t);
        let err = (p - points[i]).norm();
        assert!(err < 1e-8, "point {i} at t={t}: error = {err}");
    }
}

#[test]
fn approximate_endpoints_exact() {
    let points: Vec<Point3> = (0..20)
        .map(|i| {
            let t = i as f64 / 19.0;
            Point3::new(t, (t * std::f64::consts::PI).sin(), 0.0)
        })
        .collect();
    let curve = approximate_curve(&points, 8, 3).unwrap();

    let start = curve.point_at(curve.domain().start);
    let end = curve.point_at(curve.domain().end);
    assert!((start - points[0]).norm() < 1e-10, "start mismatch");
    assert!((end - points[19]).norm() < 1e-10, "end mismatch");
}

#[test]
fn approximate_close_to_data() {
    let points: Vec<Point3> = (0..50)
        .map(|i| {
            let t = i as f64 / 49.0;
            Point3::new(t, (t * std::f64::consts::TAU).sin(), 0.0)
        })
        .collect();
    let curve = approximate_curve(&points, 12, 3).unwrap();

    // Max deviation should be small for a smooth sine curve
    let domain = curve.domain();
    let mut max_err = 0.0_f64;
    for i in 0..100 {
        let u = i as f64 / 99.0;
        let t = domain.start + u * (domain.end - domain.start);
        let p = curve.point_at(t);
        // Find closest data point (rough check)
        let min_dist = points.iter().map(|q| (p - q).norm()).fold(f64::MAX, f64::min);
        max_err = max_err.max(min_dist);
    }
    assert!(max_err < 0.1, "max deviation = {max_err}");
}

#[test]
fn approximate_equals_interpolate_when_same_count() {
    let points = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(1.0, 2.0, 0.0),
        Point3::new(3.0, 1.0, 0.0),
        Point3::new(4.0, 3.0, 0.0),
    ];
    let interp = interpolate_curve(&points, 3).unwrap();
    let approx = approximate_curve(&points, 4, 3).unwrap();

    // Both should produce the same curve
    for i in 0..=10 {
        let t = i as f64 / 10.0;
        let p1 = interp.point_at(t);
        let p2 = approx.point_at(t);
        assert!((p1 - p2).norm() < 1e-8, "divergence at t={t}");
    }
}

#[test]
fn interpolate_too_few_points() {
    let points = vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)];
    assert!(interpolate_curve(&points, 3).is_err());
}

#[test]
fn approximate_invalid_num_cp() {
    let points = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(1.0, 0.0, 0.0),
        Point3::new(2.0, 0.0, 0.0),
    ];
    // num_cp > points
    assert!(approximate_curve(&points, 5, 2).is_err());
    // num_cp < degree + 1
    assert!(approximate_curve(&points, 2, 3).is_err());
}
