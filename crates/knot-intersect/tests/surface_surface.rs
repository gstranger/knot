use knot_geom::Point3;
use knot_geom::Vector3;
use knot_geom::surface::{Surface, Plane, Sphere, Cylinder};
use knot_intersect::surface_surface::intersect_surfaces;

const TOL: f64 = 1e-6;

#[test]
fn plane_plane_perpendicular() {
    let a = Surface::Plane(Plane::new(Point3::origin(), Vector3::z()));
    let b = Surface::Plane(Plane::new(Point3::origin(), Vector3::x()));
    let traces = intersect_surfaces(&a, &b, TOL).unwrap();
    assert_eq!(traces.len(), 1);
    // Intersection should be along the y-axis (perpendicular to both normals)
    for pt in &traces[0].points {
        assert!(pt.x.abs() < TOL, "x should be ~0, got {}", pt.x);
        assert!(pt.z.abs() < TOL, "z should be ~0, got {}", pt.z);
    }
}

#[test]
fn plane_plane_parallel() {
    let a = Surface::Plane(Plane::new(Point3::origin(), Vector3::z()));
    let b = Surface::Plane(Plane::new(Point3::new(0.0, 0.0, 1.0), Vector3::z()));
    let traces = intersect_surfaces(&a, &b, TOL).unwrap();
    assert!(traces.is_empty());
}

#[test]
fn plane_sphere_equator() {
    let plane = Surface::Plane(Plane::new(Point3::origin(), Vector3::z()));
    let sphere = Surface::Sphere(Sphere::new(Point3::origin(), 2.0));
    let traces = intersect_surfaces(&plane, &sphere, TOL).unwrap();
    assert_eq!(traces.len(), 1);
    // Should be a circle of radius 2 in the z=0 plane
    for pt in &traces[0].points {
        assert!(pt.z.abs() < TOL);
        let r = (pt.x * pt.x + pt.y * pt.y).sqrt();
        assert!((r - 2.0).abs() < 0.01, "radius should be ~2, got {}", r);
    }
}

#[test]
fn plane_sphere_offset() {
    // Plane at z=1, sphere radius 2 centered at origin
    // Intersection circle radius = sqrt(4-1) = sqrt(3)
    let plane = Surface::Plane(Plane::new(Point3::new(0.0, 0.0, 1.0), Vector3::z()));
    let sphere = Surface::Sphere(Sphere::new(Point3::origin(), 2.0));
    let traces = intersect_surfaces(&plane, &sphere, TOL).unwrap();
    assert_eq!(traces.len(), 1);
    let expected_r = (4.0 - 1.0_f64).sqrt();
    for pt in &traces[0].points {
        assert!((pt.z - 1.0).abs() < TOL);
        let r = (pt.x * pt.x + pt.y * pt.y).sqrt();
        assert!((r - expected_r).abs() < 0.01, "radius should be ~{expected_r}, got {r}");
    }
}

#[test]
fn plane_sphere_miss() {
    let plane = Surface::Plane(Plane::new(Point3::new(0.0, 0.0, 5.0), Vector3::z()));
    let sphere = Surface::Sphere(Sphere::new(Point3::origin(), 1.0));
    let traces = intersect_surfaces(&plane, &sphere, TOL).unwrap();
    assert!(traces.is_empty());
}

#[test]
fn sphere_sphere_intersecting() {
    let a = Surface::Sphere(Sphere::new(Point3::origin(), 2.0));
    let b = Surface::Sphere(Sphere::new(Point3::new(2.0, 0.0, 0.0), 2.0));
    let traces = intersect_surfaces(&a, &b, TOL).unwrap();
    assert_eq!(traces.len(), 1);
    // Intersection is a circle in the plane x=1
    for pt in &traces[0].points {
        assert!((pt.x - 1.0).abs() < 0.05, "x should be ~1, got {}", pt.x);
    }
}

#[test]
fn sphere_sphere_disjoint() {
    let a = Surface::Sphere(Sphere::new(Point3::origin(), 1.0));
    let b = Surface::Sphere(Sphere::new(Point3::new(5.0, 0.0, 0.0), 1.0));
    let traces = intersect_surfaces(&a, &b, TOL).unwrap();
    assert!(traces.is_empty());
}

#[test]
fn plane_cylinder_parallel_to_axis() {
    // Plane parallel to cylinder axis, cutting through the cylinder
    let cyl = Surface::Cylinder(Cylinder {
        origin: Point3::origin(),
        axis: Vector3::z(),
        radius: 1.0,
        ref_direction: Vector3::x(),
        v_min: -2.0,
        v_max: 2.0,
    });
    // Plane at x=0 with normal in x-direction — should give 2 lines on the cylinder
    let plane = Surface::Plane(Plane::new(Point3::origin(), Vector3::x()));
    let traces = intersect_surfaces(&plane, &cyl, TOL).unwrap();
    // Two lines at y=+1 and y=-1 (where x=0 intersects the unit circle)
    assert_eq!(traces.len(), 2, "expected 2 line traces, got {}", traces.len());
}
