//! Adversarial SSI tests: edge cases that break naive marching algorithms.

use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::surface::*;
use knot_intersect::surface_surface::intersect_surfaces;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

const TOL: f64 = 1e-6;

// ═══════════════════════════════════════════════════════════════════
// Tangent intersections: surfaces touch but don't cross
// ═══════════════════════════════════════════════════════════════════

#[test]
fn sphere_sphere_tangent_external() {
    // Two spheres touching at exactly one point (external tangency)
    let a = Surface::Sphere(Sphere::new(Point3::origin(), 1.0));
    let b = Surface::Sphere(Sphere::new(Point3::new(2.0, 0.0, 0.0), 1.0));
    let traces = intersect_surfaces(&a, &b, TOL).unwrap();
    assert_eq!(traces.len(), 1, "tangent spheres should produce 1 trace");
    assert_eq!(traces[0].points.len(), 1, "tangent touch should be a single point");
    let pt = &traces[0].points[0];
    assert!((pt.x - 1.0).abs() < TOL * 10.0, "tangent point x={}", pt.x);
}

#[test]
fn sphere_sphere_tangent_internal() {
    // Sphere inside another, touching at one point (internal tangency)
    let a = Surface::Sphere(Sphere::new(Point3::origin(), 3.0));
    let b = Surface::Sphere(Sphere::new(Point3::new(1.0, 0.0, 0.0), 2.0));
    let traces = intersect_surfaces(&a, &b, TOL).unwrap();
    assert_eq!(traces.len(), 1, "internal tangent should produce 1 trace");
    assert_eq!(traces[0].points.len(), 1, "should be a single tangent point");
}

#[test]
fn plane_sphere_tangent() {
    // Plane just touching the top of a sphere
    let plane = Surface::Plane(Plane::new(Point3::new(0.0, 0.0, 1.0), Vector3::z()));
    let sphere = Surface::Sphere(Sphere::new(Point3::origin(), 1.0));
    let traces = intersect_surfaces(&plane, &sphere, TOL).unwrap();
    assert_eq!(traces.len(), 1);
    assert_eq!(traces[0].points.len(), 1, "tangent plane-sphere should be a single point");
}

// ═══════════════════════════════════════════════════════════════════
// Closed loops: intersection curves that close on themselves
// ═══════════════════════════════════════════════════════════════════

#[test]
fn sphere_sphere_circle_is_closed() {
    // Two overlapping spheres → intersection is a circle
    let a = Surface::Sphere(Sphere::new(Point3::origin(), 2.0));
    let b = Surface::Sphere(Sphere::new(Point3::new(1.0, 0.0, 0.0), 2.0));
    let traces = intersect_surfaces(&a, &b, TOL).unwrap();
    assert_eq!(traces.len(), 1);
    let trace = &traces[0];
    assert!(trace.points.len() >= 10, "circle should have many points");
    // Check it's actually closed: first ≈ last
    let first = trace.points.first().unwrap();
    let last = trace.points.last().unwrap();
    assert!((first - last).norm() < TOL * 100.0,
        "circle should be closed, gap = {}", (first - last).norm());
}

#[test]
fn plane_sphere_circle_is_closed() {
    // Plane through sphere center → equator circle
    let plane = Surface::Plane(Plane::new(Point3::origin(), Vector3::z()));
    let sphere = Surface::Sphere(Sphere::new(Point3::origin(), 1.5));
    let traces = intersect_surfaces(&plane, &sphere, TOL).unwrap();
    assert_eq!(traces.len(), 1);
    let trace = &traces[0];
    let first = trace.points.first().unwrap();
    let last = trace.points.last().unwrap();
    assert!((first - last).norm() < TOL * 100.0, "equator should close");
}

// ═══════════════════════════════════════════════════════════════════
// Multi-component: surfaces intersect in multiple disjoint curves
// ═══════════════════════════════════════════════════════════════════

#[test]
fn plane_cylinder_two_lines() {
    // Plane parallel to cylinder axis, cutting through → 2 lines
    let cyl = Surface::Cylinder(Cylinder {
        origin: Point3::origin(),
        axis: Vector3::z(),
        radius: 1.0,
        ref_direction: Vector3::x(),
        v_min: -2.0,
        v_max: 2.0,
    });
    let plane = Surface::Plane(Plane::new(Point3::new(0.5, 0.0, 0.0), Vector3::x()));
    let traces = intersect_surfaces(&plane, &cyl, TOL).unwrap();
    assert_eq!(traces.len(), 2, "plane parallel to axis through cylinder → 2 lines, got {}", traces.len());
}

// ═══════════════════════════════════════════════════════════════════
// Near-miss: surfaces very close but not intersecting
// ═══════════════════════════════════════════════════════════════════

#[test]
fn sphere_sphere_near_miss() {
    // Two spheres with a tiny gap — should return empty
    let a = Surface::Sphere(Sphere::new(Point3::origin(), 1.0));
    let b = Surface::Sphere(Sphere::new(Point3::new(2.001, 0.0, 0.0), 1.0));
    let traces = intersect_surfaces(&a, &b, TOL).unwrap();
    assert!(traces.is_empty(), "near-miss should produce no intersection");
}

#[test]
fn plane_sphere_near_miss() {
    // Plane just above the sphere
    let plane = Surface::Plane(Plane::new(Point3::new(0.0, 0.0, 1.001), Vector3::z()));
    let sphere = Surface::Sphere(Sphere::new(Point3::origin(), 1.0));
    let traces = intersect_surfaces(&plane, &sphere, TOL).unwrap();
    assert!(traces.is_empty(), "plane above sphere should have no intersection");
}

// ═══════════════════════════════════════════════════════════════════
// Small intersections: tiny circles that uniform grids could miss
// ═══════════════════════════════════════════════════════════════════

#[test]
fn sphere_sphere_small_circle() {
    // Two spheres barely overlapping → small intersection circle
    let a = Surface::Sphere(Sphere::new(Point3::origin(), 1.0));
    let b = Surface::Sphere(Sphere::new(Point3::new(1.99, 0.0, 0.0), 1.0));
    let traces = intersect_surfaces(&a, &b, TOL).unwrap();
    assert_eq!(traces.len(), 1, "barely overlapping spheres should have 1 intersection");
    // The circle radius should be small
    let trace = &traces[0];
    if trace.points.len() > 1 {
        let center_x: f64 = trace.points.iter().map(|p| p.x).sum::<f64>() / trace.points.len() as f64;
        assert!((center_x - 0.995).abs() < 0.05, "circle center x={center_x}");
    }
}

// ═══════════════════════════════════════════════════════════════════
// Symmetry: result should not depend on argument order
// ═══════════════════════════════════════════════════════════════════

#[test]
fn sphere_sphere_order_independent() {
    let s1 = Surface::Sphere(Sphere::new(Point3::origin(), 2.0));
    let s2 = Surface::Sphere(Sphere::new(Point3::new(1.5, 0.0, 0.0), 2.0));

    let traces_ab = intersect_surfaces(&s1, &s2, TOL).unwrap();
    let traces_ba = intersect_surfaces(&s2, &s1, TOL).unwrap();

    assert_eq!(traces_ab.len(), traces_ba.len());
    assert_eq!(traces_ab[0].points.len(), traces_ba[0].points.len());
}

// ═══════════════════════════════════════════════════════════════════
// Accuracy: intersection points should actually lie on both surfaces
// ═══════════════════════════════════════════════════════════════════

#[test]
fn intersection_points_on_both_surfaces() {
    let a = Surface::Sphere(Sphere::new(Point3::origin(), 2.0));
    let b = Surface::Sphere(Sphere::new(Point3::new(1.0, 0.0, 0.0), 1.5));
    let traces = intersect_surfaces(&a, &b, TOL).unwrap();
    assert!(!traces.is_empty());

    for trace in &traces {
        for (i, pt) in trace.points.iter().enumerate() {
            // Check point is on sphere a
            let dist_a = ((pt - Point3::origin()).norm() - 2.0).abs();
            assert!(dist_a < TOL * 100.0,
                "point {} not on sphere a: dist={}", i, dist_a);

            // Check point is on sphere b
            let dist_b = ((pt - Point3::new(1.0, 0.0, 0.0)).norm() - 1.5).abs();
            assert!(dist_b < TOL * 100.0,
                "point {} not on sphere b: dist={}", i, dist_b);
        }
    }
}

#[test]
fn plane_sphere_points_on_both() {
    let plane = Surface::Plane(Plane::new(Point3::new(0.0, 0.0, 0.5), Vector3::z()));
    let sphere = Surface::Sphere(Sphere::new(Point3::origin(), 2.0));
    let traces = intersect_surfaces(&plane, &sphere, TOL).unwrap();
    assert!(!traces.is_empty());

    for trace in &traces {
        for pt in &trace.points {
            assert!((pt.z - 0.5).abs() < TOL * 10.0, "point not on plane: z={}", pt.z);
            let r = (pt - Point3::origin()).norm();
            assert!((r - 2.0).abs() < TOL * 10.0, "point not on sphere: r={}", r);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Concentric / degenerate configurations
// ═══════════════════════════════════════════════════════════════════

#[test]
fn sphere_sphere_concentric() {
    // Same center, different radii — no intersection
    let a = Surface::Sphere(Sphere::new(Point3::origin(), 1.0));
    let b = Surface::Sphere(Sphere::new(Point3::origin(), 2.0));
    let traces = intersect_surfaces(&a, &b, TOL).unwrap();
    assert!(traces.is_empty(), "concentric spheres should not intersect");
}

#[test]
fn sphere_sphere_identical() {
    // Identical spheres — coincident surface, not a curve intersection
    let a = Surface::Sphere(Sphere::new(Point3::origin(), 1.0));
    let b = Surface::Sphere(Sphere::new(Point3::origin(), 1.0));
    let traces = intersect_surfaces(&a, &b, TOL).unwrap();
    // Coincident surfaces: could be empty or could report something,
    // but should not crash or produce garbage
    // (coincidence detection is a separate feature — for now, empty is acceptable)
}

#[test]
fn plane_plane_coincident() {
    // Two identical planes — should return empty (coincident, not a curve)
    let a = Surface::Plane(Plane::new(Point3::origin(), Vector3::z()));
    let b = Surface::Plane(Plane::new(Point3::origin(), Vector3::z()));
    let traces = intersect_surfaces(&a, &b, TOL).unwrap();
    assert!(traces.is_empty(), "coincident planes should produce no intersection curve");
}
