use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::curve::{LineSeg, CircularArc, EllipticalArc, Curve, CurveParam};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

const EPS: f64 = 1e-10;

// ── LineSeg ──

#[test]
fn line_endpoints() {
    let l = LineSeg::new(Point3::new(0.0, 0.0, 0.0), Point3::new(3.0, 4.0, 0.0));
    let start = l.point_at(0.0);
    let end = l.point_at(1.0);
    assert!((start - Point3::origin()).norm() < EPS);
    assert!((end - Point3::new(3.0, 4.0, 0.0)).norm() < EPS);
}

#[test]
fn line_midpoint() {
    let l = LineSeg::new(Point3::new(0.0, 0.0, 0.0), Point3::new(2.0, 0.0, 0.0));
    let mid = l.point_at(0.5);
    assert!((mid.x - 1.0).abs() < EPS);
}

#[test]
fn line_length() {
    let l = LineSeg::new(Point3::new(0.0, 0.0, 0.0), Point3::new(3.0, 4.0, 0.0));
    assert!((l.length() - 5.0).abs() < EPS);
}

// ── CircularArc ──

#[test]
fn circular_arc_quarter() {
    let arc = CircularArc {
        center: Point3::origin(),
        normal: Vector3::z(),
        radius: 1.0,
        ref_direction: Vector3::x(),
        start_angle: 0.0,
        end_angle: FRAC_PI_2,
    };

    let start = arc.point_at(0.0);
    assert!((start.x - 1.0).abs() < EPS);
    assert!(start.y.abs() < EPS);

    let end = arc.point_at(FRAC_PI_2);
    assert!(end.x.abs() < EPS);
    assert!((end.y - 1.0).abs() < EPS);
}

#[test]
fn circular_arc_on_unit_circle() {
    let arc = CircularArc {
        center: Point3::origin(),
        normal: Vector3::z(),
        radius: 1.0,
        ref_direction: Vector3::x(),
        start_angle: 0.0,
        end_angle: TAU,
    };

    for i in 0..20 {
        let t = i as f64 / 20.0 * TAU;
        let p = arc.point_at(t);
        let r = (p.x * p.x + p.y * p.y).sqrt();
        assert!((r - 1.0).abs() < EPS, "off circle at t={t}, r={r}");
    }
}

// ── EllipticalArc ──

#[test]
fn elliptical_arc_axes() {
    let arc = EllipticalArc {
        center: Point3::origin(),
        normal: Vector3::z(),
        major_axis: Vector3::x(),
        major_radius: 3.0,
        minor_radius: 1.0,
        start_angle: 0.0,
        end_angle: TAU,
    };

    let p0 = arc.point_at(0.0);
    assert!((p0.x - 3.0).abs() < EPS); // major axis
    assert!(p0.y.abs() < EPS);

    let p90 = arc.point_at(FRAC_PI_2);
    assert!(p90.x.abs() < EPS);
    assert!((p90.y - 1.0).abs() < EPS); // minor axis
}

// ── Curve enum dispatch ──

#[test]
fn curve_enum_line_dispatch() {
    let l = LineSeg::new(Point3::origin(), Point3::new(1.0, 0.0, 0.0));
    let curve = Curve::Line(l);
    let mid = curve.point_at(CurveParam(0.5));
    assert!((mid.x - 0.5).abs() < EPS);
}

#[test]
fn curve_enum_domain() {
    let l = LineSeg::new(Point3::origin(), Point3::new(1.0, 0.0, 0.0));
    let curve = Curve::Line(l);
    let d = curve.domain();
    assert_eq!(d.start, 0.0);
    assert_eq!(d.end, 1.0);
}
