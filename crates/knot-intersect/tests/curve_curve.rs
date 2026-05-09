use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::curve::*;
use knot_intersect::curve_curve::intersect_curves;

const TOL: f64 = 1e-6;

#[test]
fn line_line_crossing() {
    let a = Curve::Line(LineSeg::new(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0)));
    let b = Curve::Line(LineSeg::new(Point3::new(0.0, 1.0, 0.0), Point3::new(1.0, 0.0, 0.0)));
    let hits = intersect_curves(&a, &b, TOL).unwrap();
    assert_eq!(hits.len(), 1);
    assert!((hits[0].point.x - 0.5).abs() < TOL);
    assert!((hits[0].point.y - 0.5).abs() < TOL);
}

#[test]
fn line_line_parallel_no_intersection() {
    let a = Curve::Line(LineSeg::new(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)));
    let b = Curve::Line(LineSeg::new(Point3::new(0.0, 1.0, 0.0), Point3::new(1.0, 1.0, 0.0)));
    let hits = intersect_curves(&a, &b, TOL).unwrap();
    assert!(hits.is_empty());
}

#[test]
fn line_line_no_overlap() {
    // Two lines that would intersect if extended, but the segments don't reach
    let a = Curve::Line(LineSeg::new(Point3::new(0.0, 0.0, 0.0), Point3::new(0.4, 0.4, 0.0)));
    let b = Curve::Line(LineSeg::new(Point3::new(0.6, 0.6, 0.0), Point3::new(1.0, 0.0, 0.0)));
    let hits = intersect_curves(&a, &b, TOL).unwrap();
    assert!(hits.is_empty());
}

#[test]
fn line_line_t_intersection() {
    let a = Curve::Line(LineSeg::new(Point3::new(0.0, 0.5, 0.0), Point3::new(1.0, 0.5, 0.0)));
    let b = Curve::Line(LineSeg::new(Point3::new(0.5, 0.0, 0.0), Point3::new(0.5, 1.0, 0.0)));
    let hits = intersect_curves(&a, &b, TOL).unwrap();
    assert_eq!(hits.len(), 1);
    assert!((hits[0].point.x - 0.5).abs() < TOL);
    assert!((hits[0].point.y - 0.5).abs() < TOL);
}

#[test]
fn line_nurbs_intersection() {
    // Line crossing a quadratic Bezier curve
    let line = Curve::Line(LineSeg::new(Point3::new(0.0, 0.3, 0.0), Point3::new(1.0, 0.3, 0.0)));

    let pts = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(0.5, 1.0, 0.0),
        Point3::new(1.0, 0.0, 0.0),
    ];
    let nurbs = Curve::Nurbs(NurbsCurve::new(pts, vec![1.0; 3], vec![0.0,0.0,0.0, 1.0,1.0,1.0], 2).unwrap());

    let hits = intersect_curves(&line, &nurbs, TOL).unwrap();
    // A horizontal line at y=0.3 should cross a symmetric parabolic Bezier twice
    assert_eq!(hits.len(), 2, "expected 2 intersections, got {}", hits.len());

    // Both hits should have y ~ 0.3
    for hit in &hits {
        assert!((hit.point.y - 0.3).abs() < 0.01, "hit y = {}", hit.point.y);
    }
}

#[test]
fn nurbs_nurbs_tangency() {
    // Two symmetric parabolas that meet at a tangent point at t=0.5
    let a = Curve::Nurbs(NurbsCurve::new(
        vec![Point3::new(0.0, 0.0, 0.0), Point3::new(0.5, 1.0, 0.0), Point3::new(1.0, 0.0, 0.0)],
        vec![1.0; 3],
        vec![0.0,0.0,0.0, 1.0,1.0,1.0],
        2,
    ).unwrap());

    let b = Curve::Nurbs(NurbsCurve::new(
        vec![Point3::new(0.0, 1.0, 0.0), Point3::new(0.5, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0)],
        vec![1.0; 3],
        vec![0.0,0.0,0.0, 1.0,1.0,1.0],
        2,
    ).unwrap());

    let hits = intersect_curves(&a, &b, TOL).unwrap();
    // These parabolas are tangent at x=0.5, y=0.5 — single intersection
    assert_eq!(hits.len(), 1, "expected 1 tangent intersection, got {}", hits.len());
    assert!((hits[0].point.x - 0.5).abs() < 0.01);
    assert!((hits[0].point.y - 0.5).abs() < 0.01);
}

#[test]
fn nurbs_nurbs_two_crossings() {
    // Parabola vs. a line-like curve that crosses it twice
    let a = Curve::Nurbs(NurbsCurve::new(
        vec![Point3::new(0.0, 0.0, 0.0), Point3::new(0.5, 1.5, 0.0), Point3::new(1.0, 0.0, 0.0)],
        vec![1.0; 3],
        vec![0.0,0.0,0.0, 1.0,1.0,1.0],
        2,
    ).unwrap());

    // A straight line at y=0.5 as a degree-1 NURBS
    let b = Curve::Nurbs(NurbsCurve::new(
        vec![Point3::new(0.0, 0.5, 0.0), Point3::new(1.0, 0.5, 0.0)],
        vec![1.0; 2],
        vec![0.0, 0.0, 1.0, 1.0],
        1,
    ).unwrap());

    let hits = intersect_curves(&a, &b, TOL).unwrap();
    assert_eq!(hits.len(), 2, "expected 2 crossings, got {}", hits.len());
    for hit in &hits {
        assert!((hit.point.y - 0.5).abs() < 0.01, "hit y = {}", hit.point.y);
    }
}
