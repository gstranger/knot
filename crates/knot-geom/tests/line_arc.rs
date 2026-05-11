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

// ── Track A: Curve enum dispatch ─────────────────────────────────

#[test]
fn curve_enum_length_line() {
    let c = Curve::Line(LineSeg::new(Point3::origin(), Point3::new(3.0, 4.0, 0.0)));
    assert!((c.length(1e-9) - 5.0).abs() < EPS);
}

#[test]
fn curve_enum_length_arc() {
    // Quarter circle of radius 2: length = π
    let c = Curve::CircularArc(CircularArc {
        center: Point3::origin(),
        normal: Vector3::z(),
        radius: 2.0,
        ref_direction: Vector3::x(),
        start_angle: 0.0,
        end_angle: FRAC_PI_2,
    });
    assert!((c.length(1e-9) - PI).abs() < EPS);
}

#[test]
fn curve_enum_length_ellipse_circle_case() {
    // Elliptical arc with equal radii degenerates to a circle —
    // quarter perimeter at r=2 is π. Confirms the sampling fallback
    // converges within tolerance.
    let c = Curve::EllipticalArc(EllipticalArc {
        center: Point3::origin(),
        normal: Vector3::z(),
        major_axis: Vector3::x(),
        major_radius: 2.0,
        minor_radius: 2.0,
        start_angle: 0.0,
        end_angle: FRAC_PI_2,
    });
    let len = c.length(1e-7);
    assert!((len - PI).abs() < 1e-5, "got {}", len);
}

#[test]
fn curve_enum_split_line() {
    let c = Curve::Line(LineSeg::new(Point3::origin(), Point3::new(10.0, 0.0, 0.0)));
    let (a, b) = c.split_at(CurveParam(0.5)).unwrap();
    // Left half ends at (5, 0, 0); right half starts there.
    let a_end = a.point_at(CurveParam(a.domain().end));
    let b_start = b.point_at(CurveParam(b.domain().start));
    assert!((a_end - Point3::new(5.0, 0.0, 0.0)).norm() < EPS);
    assert!((b_start - Point3::new(5.0, 0.0, 0.0)).norm() < EPS);
}

#[test]
fn curve_enum_split_rejects_endpoint() {
    let c = Curve::Line(LineSeg::new(Point3::origin(), Point3::new(1.0, 0.0, 0.0)));
    assert!(c.split_at(CurveParam(0.0)).is_err());
    assert!(c.split_at(CurveParam(1.0)).is_err());
    assert!(c.split_at(CurveParam(2.0)).is_err());
}

#[test]
fn curve_enum_reverse_line() {
    let c = Curve::Line(LineSeg::new(Point3::origin(), Point3::new(1.0, 2.0, 3.0)));
    let r = c.reverse();
    // start of reversed = end of original
    assert!((r.point_at(CurveParam(0.0)) - Point3::new(1.0, 2.0, 3.0)).norm() < EPS);
    assert!((r.point_at(CurveParam(1.0)) - Point3::origin()).norm() < EPS);
}

#[test]
fn curve_enum_reverse_arc_traces_same_geometry() {
    let c = Curve::CircularArc(CircularArc {
        center: Point3::origin(),
        normal: Vector3::z(),
        radius: 1.0,
        ref_direction: Vector3::x(),
        start_angle: 0.0,
        end_angle: FRAC_PI_2,
    });
    let r = c.reverse();
    // Reversed arc at its domain start should land on original's domain end.
    let orig_end = c.point_at(CurveParam(c.domain().end));
    let rev_start = r.point_at(CurveParam(r.domain().start));
    assert!((orig_end - rev_start).norm() < EPS);
}

#[test]
fn curve_enum_divide_by_length_line_is_uniform() {
    let c = Curve::Line(LineSeg::new(Point3::origin(), Point3::new(10.0, 0.0, 0.0)));
    let params = c.divide_by_length(4, 1e-9);
    assert_eq!(params.len(), 5);
    // Line is parameterized on [0, 1]; 4 equal-arc-length segments
    // = parameters at 0, 0.25, 0.5, 0.75, 1.
    let expected = [0.0, 0.25, 0.5, 0.75, 1.0];
    for (p, e) in params.iter().zip(expected.iter()) {
        assert!((p.0 - e).abs() < 1e-6, "got {} expected {}", p.0, e);
    }
}

#[test]
fn curve_enum_divide_by_length_full_circle() {
    // Full circle: 8 equal-length segments → angles at multiples of π/4.
    let c = Curve::CircularArc(CircularArc {
        center: Point3::origin(),
        normal: Vector3::z(),
        radius: 1.0,
        ref_direction: Vector3::x(),
        start_angle: 0.0,
        end_angle: TAU,
    });
    let params = c.divide_by_length(8, 1e-9);
    assert_eq!(params.len(), 9);
    for i in 0..=8 {
        let expected = i as f64 * TAU / 8.0;
        assert!(
            (params[i].0 - expected).abs() < 1e-4,
            "i={} got {} expected {}",
            i,
            params[i].0,
            expected
        );
    }
}
