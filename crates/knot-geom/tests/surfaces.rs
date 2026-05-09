use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::surface::*;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

const EPS: f64 = 1e-10;

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < EPS
}

// ── Plane ──

#[test]
fn plane_origin_at_zero_params() {
    let p = Plane::new(Point3::new(1.0, 2.0, 3.0), Vector3::z());
    let pt = p.point_at(0.0, 0.0);
    assert!(approx_eq(pt.x, 1.0));
    assert!(approx_eq(pt.y, 2.0));
    assert!(approx_eq(pt.z, 3.0));
}

#[test]
fn plane_signed_distance() {
    let p = Plane::new(Point3::origin(), Vector3::z());
    assert!(approx_eq(p.signed_distance(&Point3::new(0.0, 0.0, 5.0)), 5.0));
    assert!(approx_eq(p.signed_distance(&Point3::new(0.0, 0.0, -3.0)), -3.0));
    assert!(approx_eq(p.signed_distance(&Point3::new(7.0, 8.0, 0.0)), 0.0));
}

#[test]
fn plane_orthonormal_frame() {
    let p = Plane::new(Point3::origin(), Vector3::new(1.0, 1.0, 1.0));
    let dot_un = p.u_axis.dot(&p.normal);
    let dot_vn = p.v_axis.dot(&p.normal);
    let dot_uv = p.u_axis.dot(&p.v_axis);
    assert!(dot_un.abs() < EPS, "u_axis not perpendicular to normal");
    assert!(dot_vn.abs() < EPS, "v_axis not perpendicular to normal");
    assert!(dot_uv.abs() < EPS, "u_axis not perpendicular to v_axis");
}

// ── Sphere ──

#[test]
fn sphere_poles() {
    let s = Sphere::new(Point3::origin(), 2.0);
    let north = s.point_at(0.0, FRAC_PI_2);
    let south = s.point_at(0.0, -FRAC_PI_2);
    assert!(approx_eq(north.z, 2.0));
    assert!(approx_eq(south.z, -2.0));
}

#[test]
fn sphere_equator_point() {
    let s = Sphere::new(Point3::origin(), 1.0);
    let p = s.point_at(0.0, 0.0);
    assert!(approx_eq(p.x, 1.0));
    assert!(approx_eq(p.y, 0.0));
    assert!(approx_eq(p.z, 0.0));
}

#[test]
fn sphere_normal_is_unit_radial() {
    let s = Sphere::new(Point3::origin(), 3.0);
    let n = s.normal_at(FRAC_PI_4, FRAC_PI_4);
    assert!((n.norm() - 1.0).abs() < EPS);
    // Normal should point in the same direction as the point (for centered sphere)
    let p = s.point_at(FRAC_PI_4, FRAC_PI_4);
    let p_dir = p.coords.normalize();
    assert!((n - p_dir).norm() < EPS);
}

// ── Cylinder ──

#[test]
fn cylinder_base_circle() {
    let c = Cylinder {
        origin: Point3::origin(),
        axis: Vector3::z(),
        radius: 1.0,
        ref_direction: Vector3::x(),
        v_min: 0.0,
        v_max: 10.0,
    };
    let p = c.point_at(0.0, 0.0);
    assert!(approx_eq(p.x, 1.0));
    assert!(approx_eq(p.y, 0.0));
    assert!(approx_eq(p.z, 0.0));

    let p2 = c.point_at(FRAC_PI_2, 0.0);
    assert!(approx_eq(p2.x, 0.0));
    assert!(approx_eq(p2.y, 1.0));
}

#[test]
fn cylinder_height() {
    let c = Cylinder {
        origin: Point3::origin(),
        axis: Vector3::z(),
        radius: 1.0,
        ref_direction: Vector3::x(),
        v_min: 0.0,
        v_max: 10.0,
    };
    let p = c.point_at(0.0, 5.0);
    assert!(approx_eq(p.z, 5.0));
    assert!(approx_eq((p.x * p.x + p.y * p.y).sqrt(), 1.0));
}

// ── Cone ──

#[test]
fn cone_apex() {
    let c = Cone {
        apex: Point3::origin(),
        axis: Vector3::z(),
        half_angle: FRAC_PI_4,
        ref_direction: Vector3::x(),
        v_min: 0.0,
        v_max: 5.0,
    };
    let p = c.point_at(0.0, 0.0);
    assert!(approx_eq(p.x, 0.0));
    assert!(approx_eq(p.y, 0.0));
    assert!(approx_eq(p.z, 0.0));
}

#[test]
fn cone_radius_grows_with_height() {
    let c = Cone {
        apex: Point3::origin(),
        axis: Vector3::z(),
        half_angle: FRAC_PI_4, // 45 degrees — radius = height
        ref_direction: Vector3::x(),
        v_min: 0.0,
        v_max: 5.0,
    };
    let p = c.point_at(0.0, 3.0);
    assert!(approx_eq(p.z, 3.0));
    assert!(approx_eq(p.x, 3.0)); // r = v * tan(pi/4) = v
}

// ── Torus ──

#[test]
fn torus_outer_point() {
    let t = Torus {
        center: Point3::origin(),
        axis: Vector3::z(),
        major_radius: 3.0,
        minor_radius: 1.0,
        ref_direction: Vector3::x(),
    };
    // u=0, v=0 → outermost point on x-axis
    let p = t.point_at(0.0, 0.0);
    assert!(approx_eq(p.x, 4.0)); // 3 + 1
    assert!(approx_eq(p.y, 0.0));
    assert!(approx_eq(p.z, 0.0));
}

#[test]
fn torus_inner_point() {
    let t = Torus {
        center: Point3::origin(),
        axis: Vector3::z(),
        major_radius: 3.0,
        minor_radius: 1.0,
        ref_direction: Vector3::x(),
    };
    // u=0, v=PI → innermost point
    let p = t.point_at(0.0, PI);
    assert!(approx_eq(p.x, 2.0)); // 3 - 1
    assert!(approx_eq(p.y, 0.0));
}

// ── Surface enum dispatch ──

#[test]
fn surface_enum_plane_dispatch() {
    let plane = Plane::new(Point3::origin(), Vector3::z());
    let surface = Surface::Plane(plane);
    let pt = surface.point_at(SurfaceParam { u: 1.0, v: 0.0 });
    assert!(approx_eq(pt.z, 0.0));
}

// ── NURBS Surface ──

#[test]
fn nurbs_surface_bilinear_patch() {
    // Degree 1x1 bilinear patch
    let pts = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(1.0, 0.0, 0.0),
        Point3::new(0.0, 1.0, 0.0),
        Point3::new(1.0, 1.0, 0.0),
    ];
    let weights = vec![1.0; 4];
    let knots_u = vec![0.0, 0.0, 1.0, 1.0];
    let knots_v = vec![0.0, 0.0, 1.0, 1.0];

    let s = NurbsSurface::new(pts, weights, knots_u, knots_v, 1, 1, 2, 2).unwrap();

    let center = s.point_at(0.5, 0.5);
    assert!(approx_eq(center.x, 0.5));
    assert!(approx_eq(center.y, 0.5));
    assert!(approx_eq(center.z, 0.0));

    let corner = s.point_at(0.0, 0.0);
    assert!(approx_eq(corner.x, 0.0));
    assert!(approx_eq(corner.y, 0.0));
}

#[test]
fn nurbs_surface_validation_bad_count() {
    let pts = vec![Point3::origin()]; // too few
    let weights = vec![1.0];
    let knots_u = vec![0.0, 0.0, 1.0, 1.0];
    let knots_v = vec![0.0, 0.0, 1.0, 1.0];
    assert!(NurbsSurface::new(pts, weights, knots_u, knots_v, 1, 1, 2, 2).is_err());
}
