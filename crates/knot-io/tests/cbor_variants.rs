//! Per-variant CBOR roundtrip coverage.
//!
//! `cbor_roundtrip.rs` covers BReps built from box/sphere/cylinder
//! primitives, which only exercises Plane, Sphere, Cylinder, and Line
//! variants. This file pins down the remaining surface/curve variants
//! (Cone, Torus, NurbsSurface; CircularArc, EllipticalArc, NurbsCurve)
//! by round-tripping each at the geometry level via ciborium.
//!
//! The check is **byte-stable round-trip**: encode → decode → re-encode,
//! assert the two byte sequences match. Stronger than value-equality
//! because it catches silent default-fallback skips and any drift in
//! the encoding itself.
//!
//! A second sample-point assertion confirms the *semantic* roundtrip
//! is good even if a future serde refactor changes the encoding format
//! (which would break byte-equality but not correctness).

use knot_geom::curve::{CircularArc, Curve, EllipticalArc, LineSeg, NurbsCurve};
use knot_geom::surface::{
    Cone, Cylinder, NurbsSurface, Plane, Sphere, Surface, SurfaceParam, Torus,
};
use knot_geom::{Point3, Vector3};

fn cbor_encode<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut out = Vec::new();
    ciborium::into_writer(v, &mut out).expect("encode");
    out
}

fn cbor_decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> T {
    ciborium::from_reader(bytes).expect("decode")
}

/// Encode → decode → re-encode, assert the two byte payloads match.
/// `label` is for diagnostics on failure.
fn assert_byte_stable<T>(label: &str, value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let bytes1 = cbor_encode(value);
    let restored: T = cbor_decode(&bytes1);
    let bytes2 = cbor_encode(&restored);
    assert_eq!(
        bytes1, bytes2,
        "{}: CBOR roundtrip is not byte-stable",
        label,
    );
}

// ── Curves ──────────────────────────────────────────────────────────

#[test]
fn roundtrip_curve_line() {
    let line = Curve::Line(LineSeg::new(
        Point3::new(1.0, 2.0, 3.0),
        Point3::new(4.0, 5.0, 6.0),
    ));
    assert_byte_stable("Curve::Line", &line);

    let bytes = cbor_encode(&line);
    let restored: Curve = cbor_decode(&bytes);
    let p1 = line.point_at(knot_geom::curve::CurveParam(0.7));
    let p2 = restored.point_at(knot_geom::curve::CurveParam(0.7));
    assert!((p1 - p2).norm() < 1e-12);
}

#[test]
fn roundtrip_curve_circular_arc() {
    let arc = Curve::CircularArc(CircularArc {
        center: Point3::new(0.0, 0.0, 0.0),
        normal: Vector3::z(),
        radius: 2.5,
        ref_direction: Vector3::x(),
        start_angle: 0.0,
        end_angle: std::f64::consts::FRAC_PI_2,
    });
    assert_byte_stable("Curve::CircularArc", &arc);

    let restored: Curve = cbor_decode(&cbor_encode(&arc));
    let p1 = arc.point_at(knot_geom::curve::CurveParam(0.5));
    let p2 = restored.point_at(knot_geom::curve::CurveParam(0.5));
    assert!((p1 - p2).norm() < 1e-12);
}

#[test]
fn roundtrip_curve_elliptical_arc() {
    let arc = Curve::EllipticalArc(EllipticalArc {
        center: Point3::new(1.0, 1.0, 0.0),
        normal: Vector3::z(),
        major_axis: Vector3::x(),
        major_radius: 3.0,
        minor_radius: 2.0,
        start_angle: 0.0,
        end_angle: std::f64::consts::TAU,
    });
    assert_byte_stable("Curve::EllipticalArc", &arc);

    let restored: Curve = cbor_decode(&cbor_encode(&arc));
    let p1 = arc.point_at(knot_geom::curve::CurveParam(1.234));
    let p2 = restored.point_at(knot_geom::curve::CurveParam(1.234));
    assert!((p1 - p2).norm() < 1e-12);
}

#[test]
fn roundtrip_curve_nurbs() {
    // Rational quadratic Bezier (3 control points, weights [1, 2, 1]).
    let nurbs = NurbsCurve::new(
        vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
            Point3::new(3.0, 0.0, 0.0),
        ],
        vec![1.0, 2.0, 1.0],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        2,
    )
    .unwrap();
    let curve = Curve::Nurbs(nurbs);
    assert_byte_stable("Curve::Nurbs", &curve);

    let restored: Curve = cbor_decode(&cbor_encode(&curve));
    for &t in &[0.0_f64, 0.25, 0.5, 0.75, 1.0] {
        let p1 = curve.point_at(knot_geom::curve::CurveParam(t));
        let p2 = restored.point_at(knot_geom::curve::CurveParam(t));
        assert!((p1 - p2).norm() < 1e-12, "mismatch at t={}", t);
    }
}

// ── Surfaces ────────────────────────────────────────────────────────

#[test]
fn roundtrip_surface_plane() {
    let plane = Surface::Plane(Plane::new(Point3::new(1.0, 2.0, 3.0), Vector3::z()));
    assert_byte_stable("Surface::Plane", &plane);
}

#[test]
fn roundtrip_surface_sphere() {
    let sphere = Surface::Sphere(Sphere::new(Point3::new(1.0, 1.0, 1.0), 2.5));
    assert_byte_stable("Surface::Sphere", &sphere);
}

#[test]
fn roundtrip_surface_cylinder() {
    let cyl = Surface::Cylinder(Cylinder {
        origin: Point3::new(0.0, 0.0, 0.0),
        axis: Vector3::z(),
        radius: 1.5,
        ref_direction: Vector3::x(),
        v_min: -1.0,
        v_max: 4.0,
    });
    assert_byte_stable("Surface::Cylinder", &cyl);

    let restored: Surface = cbor_decode(&cbor_encode(&cyl));
    let uv = SurfaceParam { u: 0.6, v: 1.3 };
    assert!((cyl.point_at(uv) - restored.point_at(uv)).norm() < 1e-12);
}

#[test]
fn roundtrip_surface_cone() {
    let cone = Surface::Cone(Cone {
        apex: Point3::new(0.0, 0.0, 0.0),
        axis: Vector3::z(),
        half_angle: std::f64::consts::FRAC_PI_6,
        ref_direction: Vector3::x(),
        v_min: 1.0,
        v_max: 3.0,
    });
    assert_byte_stable("Surface::Cone", &cone);

    let restored: Surface = cbor_decode(&cbor_encode(&cone));
    let uv = SurfaceParam { u: 0.9, v: 2.0 };
    assert!((cone.point_at(uv) - restored.point_at(uv)).norm() < 1e-12);
}

#[test]
fn roundtrip_surface_torus() {
    let torus = Surface::Torus(Torus {
        center: Point3::new(0.0, 0.0, 0.0),
        axis: Vector3::z(),
        major_radius: 3.0,
        minor_radius: 1.0,
        ref_direction: Vector3::x(),
    });
    assert_byte_stable("Surface::Torus", &torus);

    let restored: Surface = cbor_decode(&cbor_encode(&torus));
    let uv = SurfaceParam { u: 1.1, v: 0.4 };
    assert!((torus.point_at(uv) - restored.point_at(uv)).norm() < 1e-12);
}

#[test]
fn roundtrip_surface_nurbs() {
    // Rational bilinear patch (2×2 grid, all weights 1 except corner).
    let pts = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(1.0, 0.0, 0.5),
        Point3::new(0.0, 1.0, 0.5),
        Point3::new(1.0, 1.0, 0.0),
    ];
    let weights = vec![1.0, 1.0, 1.0, 2.0];
    let knots_u = vec![0.0, 0.0, 1.0, 1.0];
    let knots_v = vec![0.0, 0.0, 1.0, 1.0];
    let nurbs = NurbsSurface::new(
        pts, weights, knots_u, knots_v, 1, 1, 2, 2,
    )
    .unwrap();
    let surface = Surface::Nurbs(nurbs);
    assert_byte_stable("Surface::Nurbs", &surface);

    let restored: Surface = cbor_decode(&cbor_encode(&surface));
    for &(u, v) in &[(0.0, 0.0), (0.5, 0.5), (1.0, 1.0), (0.3, 0.7)] {
        let p1 = surface.point_at(SurfaceParam { u, v });
        let p2 = restored.point_at(SurfaceParam { u, v });
        assert!(
            (p1 - p2).norm() < 1e-12,
            "NURBS surface mismatch at (u={}, v={}): {:?} vs {:?}",
            u, v, p1, p2,
        );
    }
}

// ── Reject corrupt / version-bumped payloads ─────────────────────────

#[test]
fn corrupt_payload_does_not_panic() {
    // Half-truncated CBOR of a Plane: should produce a decode error,
    // not a panic.
    let plane = Surface::Plane(Plane::new(Point3::origin(), Vector3::z()));
    let mut bytes = cbor_encode(&plane);
    bytes.truncate(bytes.len() / 2);
    let result: Result<Surface, _> = ciborium::from_reader(bytes.as_slice());
    assert!(result.is_err(), "expected decode error on truncated payload");
}
