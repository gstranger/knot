use knot_geom::curve::{offset::offset, CircularArc, Curve, CurveParam, LineSeg, NurbsCurve};
use knot_geom::{Point3, Vector3};
use std::f64::consts::{FRAC_PI_2, PI};

const EPS: f64 = 1e-9;

// ── Lines ────────────────────────────────────────────────────────

#[test]
fn line_offset_in_xy_plane_shifts_y() {
    // Line along +X. plane_normal = +Z. Cross(+Z, +X) = +Y, so positive
    // distance shifts in +Y.
    let line = Curve::Line(LineSeg::new(
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(2.0, 0.0, 0.0),
    ));
    let off = offset(&line, 0.5, Vector3::z()).unwrap();
    let p0 = off.point_at(CurveParam(0.0));
    let p1 = off.point_at(CurveParam(1.0));
    assert!((p0 - Point3::new(0.0, 0.5, 0.0)).norm() < EPS);
    assert!((p1 - Point3::new(2.0, 0.5, 0.0)).norm() < EPS);
}

#[test]
fn line_offset_negative_distance_flips_side() {
    let line = Curve::Line(LineSeg::new(
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(2.0, 0.0, 0.0),
    ));
    let off = offset(&line, -0.5, Vector3::z()).unwrap();
    let p0 = off.point_at(CurveParam(0.0));
    assert!((p0 - Point3::new(0.0, -0.5, 0.0)).norm() < EPS);
}

#[test]
fn line_offset_rejects_zero_normal() {
    let line = Curve::Line(LineSeg::new(
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(1.0, 0.0, 0.0),
    ));
    assert!(offset(&line, 1.0, Vector3::zeros()).is_err());
}

#[test]
fn line_offset_rejects_normal_parallel_to_line() {
    let line = Curve::Line(LineSeg::new(
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(1.0, 0.0, 0.0),
    ));
    // plane_normal along the line direction → cross product is zero.
    assert!(offset(&line, 1.0, Vector3::x()).is_err());
}

// ── Circular arcs ────────────────────────────────────────────────

fn xy_unit_arc(start: f64, end: f64) -> Curve {
    Curve::CircularArc(CircularArc {
        center: Point3::origin(),
        normal: Vector3::z(),
        radius: 1.0,
        ref_direction: Vector3::x(),
        start_angle: start,
        end_angle: end,
    })
}

#[test]
fn arc_offset_positive_distance_shrinks_radius() {
    // Quarter arc, +Z normal → positive offset shrinks (offset toward centre).
    let arc = xy_unit_arc(0.0, FRAC_PI_2);
    let off = offset(&arc, 0.25, Vector3::z()).unwrap();
    match &off {
        Curve::CircularArc(a) => {
            assert!((a.radius - 0.75).abs() < EPS);
            assert_eq!(a.start_angle, 0.0);
            assert_eq!(a.end_angle, FRAC_PI_2);
        }
        _ => panic!("expected CircularArc, got {:?}", off),
    }
}

#[test]
fn arc_offset_negative_distance_grows_radius() {
    let arc = xy_unit_arc(0.0, PI);
    let off = offset(&arc, -0.5, Vector3::z()).unwrap();
    match off {
        Curve::CircularArc(a) => assert!((a.radius - 1.5).abs() < EPS),
        _ => panic!("expected CircularArc"),
    }
}

#[test]
fn arc_offset_with_flipped_plane_normal_grows_radius_for_positive_distance() {
    // plane_normal opposite to arc.normal → sign flips, so positive
    // distance now grows the radius (offset to the outside).
    let arc = xy_unit_arc(0.0, FRAC_PI_2);
    let off = offset(&arc, 0.25, -Vector3::z()).unwrap();
    match off {
        Curve::CircularArc(a) => assert!((a.radius - 1.25).abs() < EPS),
        _ => panic!("expected CircularArc"),
    }
}

#[test]
fn arc_offset_rejects_radius_collapse() {
    let arc = xy_unit_arc(0.0, FRAC_PI_2);
    // distance >= radius should fail rather than producing a degenerate arc.
    assert!(offset(&arc, 1.0, Vector3::z()).is_err());
    assert!(offset(&arc, 1.5, Vector3::z()).is_err());
}

#[test]
fn arc_offset_rejects_non_planar_normal() {
    // arc plane is XY, but offset normal is X → not parallel → reject.
    let arc = xy_unit_arc(0.0, FRAC_PI_2);
    assert!(offset(&arc, 0.1, Vector3::x()).is_err());
}

// ── Unsupported curve types ──────────────────────────────────────

#[test]
fn nurbs_offset_unsupported() {
    // Trivial degree-1 NURBS that is geometrically a line — still rejected.
    let nurbs = NurbsCurve::new(
        vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)],
        vec![1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
        1,
    )
    .unwrap();
    let curve = Curve::Nurbs(nurbs);
    let err = offset(&curve, 0.1, Vector3::z()).unwrap_err();
    assert!(format!("{err}").contains("NURBS"));
}
